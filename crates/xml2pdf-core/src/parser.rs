//! UBL 2.1 parser for SUNAT invoices, receipts, credit notes and debit notes.
//!
//! Elements are matched by local name, so the parser does not depend on the
//! namespace prefixes chosen by the issuing system.

use std::borrow::Cow;
use std::str::FromStr;

use roxmltree::Node;
use rust_decimal::Decimal;

use crate::model::{
    Address, AffectedDocument, Detraction, Document, DocumentKind, Installment, Legend, Line,
    Party, Payment, PaymentForm, RelatedDocument, TaxSubtotal, Totals,
};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("the file is not valid XML: {0}")]
    Xml(String),
    #[error("unsupported document `{0}`: expected Invoice, CreditNote or DebitNote")]
    UnsupportedDocument(String),
    #[error("required field `{0}` is missing")]
    MissingField(&'static str),
}

/// Parses a document from raw bytes, decoding UTF-8 or ISO-8859-1 input.
pub fn parse_bytes(bytes: &[u8]) -> Result<Document, ParseError> {
    parse_str(&decode(bytes))
}

/// Parses a document from XML text.
pub fn parse_str(xml: &str) -> Result<Document, ParseError> {
    let tree = roxmltree::Document::parse(xml).map_err(|err| ParseError::Xml(err.to_string()))?;
    let root = tree.root_element();

    let (kind, type_code, line_tag, quantity_tag) = match root.tag_name().name() {
        "Invoice" => {
            let code = text(root, "InvoiceTypeCode").unwrap_or_else(|| "01".to_owned());
            let kind = if code == "03" {
                DocumentKind::Receipt
            } else {
                DocumentKind::Invoice
            };
            (kind, code, "InvoiceLine", "InvoicedQuantity")
        }
        "CreditNote" => (
            DocumentKind::CreditNote,
            "07".to_owned(),
            "CreditNoteLine",
            "CreditedQuantity",
        ),
        "DebitNote" => (
            DocumentKind::DebitNote,
            "08".to_owned(),
            "DebitNoteLine",
            "DebitedQuantity",
        ),
        other => return Err(ParseError::UnsupportedDocument(other.to_owned())),
    };

    let id = text(root, "ID").ok_or(ParseError::MissingField("ID"))?;
    let (series, number) = match id.split_once('-') {
        Some((series, number)) => (series.to_owned(), number.to_owned()),
        None => (String::new(), id.clone()),
    };

    let supplier = party(root, "AccountingSupplierParty");
    if supplier.doc_number.is_empty() {
        return Err(ParseError::MissingField("AccountingSupplierParty"));
    }

    let mut legends = Vec::new();
    let mut notes = Vec::new();
    for note in children(root, "Note") {
        let Some(value) = own_text(note) else {
            continue;
        };
        match note.attribute("languageLocaleID") {
            Some(code) => legends.push(Legend {
                code: code.to_owned(),
                text: value,
            }),
            None => notes.push(value),
        }
    }

    let monetary_total =
        child(root, "LegalMonetaryTotal").or_else(|| child(root, "RequestedMonetaryTotal"));
    let tax_total = child(root, "TaxTotal");
    let mut totals = monetary_total.map(totals).unwrap_or_default();
    totals.total_taxes = tax_total
        .and_then(|node| decimal(node, "TaxAmount"))
        .unwrap_or_default();

    Ok(Document {
        kind,
        type_code,
        id,
        series,
        number,
        issue_date: text(root, "IssueDate").ok_or(ParseError::MissingField("IssueDate"))?,
        issue_time: text(root, "IssueTime"),
        due_date: text(root, "DueDate").or_else(|| last_installment_date(root)),
        currency: text(root, "DocumentCurrencyCode").unwrap_or_else(|| "PEN".to_owned()),
        operation_type: child(root, "InvoiceTypeCode")
            .and_then(|node| node.attribute("listID"))
            .map(str::to_owned),
        supplier,
        customer: party(root, "AccountingCustomerParty"),
        lines: children(root, line_tag)
            .map(|node| line(node, quantity_tag))
            .collect(),
        taxes: tax_total.map(tax_subtotals).unwrap_or_default(),
        totals,
        legends,
        notes,
        payment: payment(root),
        detraction: detraction(root),
        affected: affected(root),
        related: related(root),
        order_reference: path(root, &["OrderReference", "ID"]).and_then(own_text),
        hash: root
            .descendants()
            .find(|node| node.tag_name().name() == "DigestValue")
            .and_then(own_text),
    })
}

/// Decodes XML bytes. SUNAT systems emit UTF-8 or ISO-8859-1; the declared
/// encoding wins, and invalid UTF-8 falls back to ISO-8859-1.
fn decode(bytes: &[u8]) -> Cow<'_, str> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let declaration = &bytes[..bytes.len().min(200)];
    let declaration = String::from_utf8_lossy(declaration).to_ascii_lowercase();
    let declares_latin1 = declaration
        .split("?>")
        .next()
        .is_some_and(|head| head.contains("iso-8859-1") || head.contains("windows-1252"));

    if declares_latin1 {
        return Cow::Owned(latin1(bytes));
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => Cow::Borrowed(text),
        Err(_) => Cow::Owned(latin1(bytes)),
    }
}

/// Decodes ISO-8859-1 and rewrites the declaration so the XML parser accepts
/// the now UTF-8 text.
fn latin1(bytes: &[u8]) -> String {
    let text: String = bytes.iter().map(|&byte| byte as char).collect();
    match text.find("?>") {
        Some(end) if text.trim_start().starts_with("<?xml") => {
            format!("<?xml version=\"1.0\" encoding=\"UTF-8\"{}", &text[end..])
        }
        _ => text,
    }
}

fn child<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == name)
}

fn children<'a, 'input: 'a>(
    node: Node<'a, 'input>,
    name: &'a str,
) -> impl Iterator<Item = Node<'a, 'input>> {
    node.children()
        .filter(move |child| child.is_element() && child.tag_name().name() == name)
}

fn path<'a, 'input>(node: Node<'a, 'input>, names: &[&str]) -> Option<Node<'a, 'input>> {
    names
        .iter()
        .try_fold(node, |current, name| child(current, name))
}

/// Trimmed text of a node, or `None` when it is empty.
fn own_text(node: Node) -> Option<String> {
    let value = node
        .text()?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!value.is_empty()).then_some(value)
}

fn text(node: Node, name: &str) -> Option<String> {
    child(node, name).and_then(own_text)
}

fn decimal(node: Node, name: &str) -> Option<Decimal> {
    text(node, name).and_then(|value| Decimal::from_str(&value).ok())
}

fn party(root: Node, wrapper: &str) -> Party {
    let Some(party) = path(root, &[wrapper, "Party"]) else {
        return Party::default();
    };
    let identification = path(party, &["PartyIdentification", "ID"]);
    let legal_entity = child(party, "PartyLegalEntity");

    Party {
        doc_type: identification
            .and_then(|node| node.attribute("schemeID"))
            .unwrap_or_default()
            .to_owned(),
        doc_number: identification.and_then(own_text).unwrap_or_default(),
        legal_name: legal_entity
            .and_then(|node| text(node, "RegistrationName"))
            .unwrap_or_default(),
        trade_name: path(party, &["PartyName", "Name"]).and_then(own_text),
        address: legal_entity
            .and_then(|node| child(node, "RegistrationAddress"))
            .map(address),
    }
}

fn address(node: Node) -> Address {
    Address {
        line: path(node, &["AddressLine", "Line"]).and_then(own_text),
        urbanization: text(node, "CitySubdivisionName").filter(|value| value != "-"),
        district: text(node, "District"),
        province: text(node, "CityName"),
        department: text(node, "CountrySubentity"),
        ubigeo: text(node, "ID"),
        country: path(node, &["Country", "IdentificationCode"]).and_then(own_text),
    }
}

fn line(node: Node, quantity_tag: &str) -> Line {
    let quantity = child(node, quantity_tag);
    let item = child(node, "Item");
    let tax_category = path(node, &["TaxTotal", "TaxSubtotal", "TaxCategory"]);

    let mut unit_price = None;
    let mut free = false;
    if let Some(pricing) = child(node, "PricingReference") {
        for price in children(pricing, "AlternativeConditionPrice") {
            let amount = decimal(price, "PriceAmount");
            match text(price, "PriceTypeCode").as_deref() {
                Some("02") => {
                    free = true;
                    unit_price = unit_price.or(amount);
                }
                _ => unit_price = amount.or(unit_price),
            }
        }
    }

    let discount = children(node, "AllowanceCharge")
        .filter(|charge| text(*charge, "ChargeIndicator").as_deref() == Some("false"))
        .filter_map(|charge| decimal(charge, "Amount"))
        .sum();

    let description = item
        .map(|item| {
            children(item, "Description")
                .filter_map(own_text)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();

    let igv_percent = child(node, "TaxTotal").and_then(|total| {
        children(total, "TaxSubtotal")
            .filter_map(|subtotal| child(subtotal, "TaxCategory"))
            .find(|category| {
                path(*category, &["TaxScheme", "ID"])
                    .and_then(own_text)
                    .as_deref()
                    == Some(crate::model::tax_scheme::IGV)
            })
            .and_then(|category| decimal(category, "Percent"))
    });

    Line {
        number: text(node, "ID").unwrap_or_default(),
        code: item
            .and_then(|item| path(item, &["SellersItemIdentification", "ID"]))
            .and_then(own_text),
        description,
        unit_code: quantity
            .and_then(|node| node.attribute("unitCode"))
            .unwrap_or("NIU")
            .to_owned(),
        quantity: quantity
            .and_then(own_text)
            .and_then(|value| Decimal::from_str(&value).ok())
            .unwrap_or_default(),
        unit_value: path(node, &["Price", "PriceAmount"])
            .and_then(own_text)
            .and_then(|value| Decimal::from_str(&value).ok())
            .unwrap_or_default(),
        unit_price,
        line_total: decimal(node, "LineExtensionAmount").unwrap_or_default(),
        tax_amount: child(node, "TaxTotal")
            .and_then(|total| decimal(total, "TaxAmount"))
            .unwrap_or_default(),
        discount,
        tax_affectation: tax_category.and_then(|category| text(category, "TaxExemptionReasonCode")),
        igv_percent,
        free,
    }
}

fn tax_subtotals(tax_total: Node) -> Vec<TaxSubtotal> {
    children(tax_total, "TaxSubtotal")
        .map(|subtotal| {
            let scheme = path(subtotal, &["TaxCategory", "TaxScheme"]);
            TaxSubtotal {
                scheme_id: scheme.and_then(|node| text(node, "ID")).unwrap_or_default(),
                scheme_name: scheme
                    .and_then(|node| text(node, "Name"))
                    .unwrap_or_default(),
                taxable: decimal(subtotal, "TaxableAmount").unwrap_or_default(),
                amount: decimal(subtotal, "TaxAmount").unwrap_or_default(),
            }
        })
        .collect()
}

fn totals(node: Node) -> Totals {
    let amount = |name| decimal(node, name).unwrap_or_default();
    Totals {
        line_extension: amount("LineExtensionAmount"),
        total_taxes: Decimal::ZERO,
        tax_inclusive: amount("TaxInclusiveAmount"),
        discounts: amount("AllowanceTotalAmount"),
        charges: amount("ChargeTotalAmount"),
        prepaid: amount("PrepaidAmount"),
        payable: amount("PayableAmount"),
    }
}

/// Payment terms identified by `FormaPago`: `Contado`, `Credito` or `CuotaNNN`.
fn payment(root: Node) -> Option<Payment> {
    let mut form = None;
    let mut pending = None;
    let mut installments = Vec::new();

    for terms in children(root, "PaymentTerms") {
        if text(terms, "ID").as_deref() != Some("FormaPago") {
            continue;
        }
        let means = text(terms, "PaymentMeansID").unwrap_or_default();
        let lowered = means.to_ascii_lowercase();
        if lowered == "contado" {
            form = Some(PaymentForm::Cash);
        } else if lowered == "credito" {
            form = Some(PaymentForm::Credit);
            pending = decimal(terms, "Amount");
        } else if let Some(number) = lowered.strip_prefix("cuota") {
            installments.push(Installment {
                number: number.trim_start_matches('0').to_owned(),
                amount: decimal(terms, "Amount").unwrap_or_default(),
                due_date: text(terms, "PaymentDueDate"),
            });
        }
    }

    let form = form.or((!installments.is_empty()).then_some(PaymentForm::Credit))?;
    Some(Payment {
        form,
        pending,
        installments,
    })
}

fn last_installment_date(root: Node) -> Option<String> {
    children(root, "PaymentTerms")
        .filter_map(|terms| text(terms, "PaymentDueDate"))
        .max()
}

fn detraction(root: Node) -> Option<Detraction> {
    let terms = children(root, "PaymentTerms")
        .find(|terms| text(*terms, "ID").as_deref() == Some("Detraccion"))?;
    Some(Detraction {
        code: text(terms, "PaymentMeansID"),
        percent: decimal(terms, "PaymentPercent"),
        amount: decimal(terms, "Amount"),
        account: path(root, &["PaymentMeans", "PayeeFinancialAccount", "ID"]).and_then(own_text),
    })
}

fn affected(root: Node) -> Option<AffectedDocument> {
    let discrepancy = child(root, "DiscrepancyResponse");
    let reference = path(root, &["BillingReference", "InvoiceDocumentReference"]);
    let id = reference
        .and_then(|node| text(node, "ID"))
        .or_else(|| discrepancy.and_then(|node| text(node, "ReferenceID")))?;

    Some(AffectedDocument {
        id,
        type_code: reference.and_then(|node| text(node, "DocumentTypeCode")),
        reason_code: discrepancy.and_then(|node| text(node, "ResponseCode")),
        description: discrepancy.and_then(|node| text(node, "Description")),
    })
}

fn related(root: Node) -> Vec<RelatedDocument> {
    ["DespatchDocumentReference", "AdditionalDocumentReference"]
        .into_iter()
        .flat_map(|tag| children(root, tag))
        .filter_map(|node| {
            Some(RelatedDocument {
                id: text(node, "ID")?,
                type_code: text(node, "DocumentTypeCode"),
            })
        })
        .collect()
}
