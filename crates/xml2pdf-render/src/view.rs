//! Builds the data handed to templates as `data.json`.
//!
//! Every value is already formatted for printing, so templates only decide
//! the layout. The contract is documented in `docs/templates.md`.

use rust_decimal::Decimal;
use serde_json::{Value, json};
use xml2pdf_core::{Document, DocumentKind, Line, PaymentForm, catalog, tax_scheme, words};

use crate::format::{date, decimals, money, money_with_symbol, quantity};
use crate::{Family, RenderOptions};

/// Credit printed at the end of every document, followed by the addresses.
const CREDIT_TEXT: &str = "PDF generado por qxml2pdf · Qlab by Qodi. Visítanos:";
const CREDIT_LINKS: [&str; 2] = ["https://qlab.pe", "https://qodi.pe"];

/// Legend code of the amount written in words (catalog 52).
const AMOUNT_IN_WORDS: &str = "1000";

/// Returns the template data for a document.
pub fn template_data(document: &Document, options: &RenderOptions) -> Value {
    template_data_for(document, options, false)
}

/// Template data, optionally asking the template to mark where each block of
/// the design lands (`editor`), which the format designer needs.
pub(crate) fn template_data_for(
    document: &Document,
    options: &RenderOptions,
    editor: bool,
) -> Value {
    let settings = &options.settings;
    let symbol = catalog::currency_symbol(&document.currency);
    let title = catalog::document_title(&document.type_code);
    let amount = |value: Decimal| money_with_symbol(symbol, value);

    json!({
        "format": settings.format.as_str(),
        "page": {
            "width": settings.format.width_mm(),
            "ticket": settings.format.is_ticket(),
            "paper": settings.format.typst_paper(),
        },
        "design": settings.design_for(settings.format).resolved(Family::of(settings.format)),
        "editor": editor,
        "logo": options.logo.as_ref().map(|logo| logo.file_name()),
        "qr": settings.show_qr.then_some("qr.svg"),
        "doc": {
            "title": title,
            "id": document.id,
            "type_code": document.type_code,
            "issue_date": date(&document.issue_date),
            "issue_time": document.issue_time,
            "due_date": document.due_date.as_deref().map(date),
            "currency": document.currency,
            "currency_name": catalog::currency_name(&document.currency),
            "currency_symbol": symbol,
            "order_reference": document.order_reference,
            "fields": document_fields(document),
        },
        "issuer": {
            "ruc": document.supplier.doc_number,
            "name": document.supplier.legal_name,
            "trade_name": document.supplier.trade_name,
            "address": document.supplier.address.as_ref().map(|address| address.one_line()),
            "extra": settings.issuer_lines,
        },
        "customer": {
            "doc_label": catalog::identity_label(&document.customer.doc_type),
            "doc_number": document.customer.doc_number,
            "name": document.customer.legal_name,
            "address": document.customer.address.as_ref().map(|address| address.one_line()),
            "fields": customer_fields(document),
        },
        "lines": document.lines.iter().map(line).collect::<Vec<_>>(),
        "totals": totals(document, &amount),
        "total": total_row("TOTAL", document.totals.payable, &amount),
        "amount_in_words": amount_in_words(document),
        "legends": document
            .legends
            .iter()
            .filter(|legend| legend.code != AMOUNT_IN_WORDS)
            .map(|legend| legend.text.clone())
            .collect::<Vec<_>>(),
        "notes": document.notes,
        "payment": payment(document, &amount),
        "detraction": detraction(document),
        "affected": affected(document),
        "related": document
            .related
            .iter()
            .map(|related| json!({
                "label": catalog::document_name(related.type_code.as_deref().unwrap_or_default()),
                "id": related.id,
            }))
            .collect::<Vec<_>>(),
        "hash": document.hash,
        "legal": format!("Representación impresa de la {title}"),
        "footer": settings.footer,
        "credit": { "text": CREDIT_TEXT, "links": CREDIT_LINKS },
    })
}

/// Short description of a document, for listings that show many of them.
pub fn document_summary(document: &Document) -> Value {
    let symbol = catalog::currency_symbol(&document.currency);
    json!({
        "file_stem": document.file_stem(),
        "id": document.id,
        "title": catalog::document_title(&document.type_code),
        "issue_date": date(&document.issue_date),
        "customer": document.customer.legal_name,
        "total": money_with_symbol(symbol, document.totals.payable),
    })
}

fn field(label: &str, value: impl Into<String>) -> Value {
    json!({ "label": label, "value": value.into() })
}

fn customer_fields(document: &Document) -> Vec<Value> {
    let customer = &document.customer;
    let mut fields = vec![
        field("CLIENTE", customer.legal_name.as_str()),
        field(
            catalog::identity_label(&customer.doc_type),
            customer.doc_number.as_str(),
        ),
    ];
    if let Some(address) = customer.address.as_ref().map(|address| address.one_line()) {
        fields.push(field("DIRECCIÓN", address));
    }
    fields.retain(|item| item["value"] != "");
    fields
}

fn document_fields(document: &Document) -> Vec<Value> {
    let mut fields = vec![field("FECHA DE EMISIÓN", date(&document.issue_date))];
    if let Some(due_date) = &document.due_date {
        fields.push(field("FECHA DE VENCIMIENTO", date(due_date)));
    }
    fields.push(field("MONEDA", catalog::currency_name(&document.currency)));
    if let Some(payment) = &document.payment {
        fields.push(field("FORMA DE PAGO", payment_form(payment.form)));
    }
    if let Some(order) = &document.order_reference {
        fields.push(field("ORDEN DE COMPRA", order.as_str()));
    }
    for related in &document.related {
        let label = catalog::document_name(related.type_code.as_deref().unwrap_or_default());
        fields.push(field(label, related.id.as_str()));
    }
    fields
}

fn line(line: &Line) -> Value {
    let with_taxes = line.line_total + line.tax_amount;
    // A zero or absurdly small quantity has no meaningful unit price.
    let unit_price = line
        .unit_price
        .unwrap_or_else(|| with_taxes.checked_div(line.quantity).unwrap_or_default());

    json!({
        "number": line.number,
        "code": line.code.as_deref().unwrap_or_default(),
        "description": line.description,
        "unit": catalog::unit_label(&line.unit_code),
        "unit_code": line.unit_code,
        "quantity": quantity(line.quantity),
        "unit_value": decimals(line.unit_value),
        "unit_price": decimals(unit_price.round_dp(6)),
        "discount": money(line.discount),
        "value": money(line.line_total),
        "tax": money(line.tax_amount),
        "total": money(with_taxes),
        "free": line.free,
    })
}

/// A totals row. `value` carries the currency symbol; `amount` is the number
/// alone, for layouts that print the symbol in its own column.
fn total_row(label: &str, value: Decimal, amount: &impl Fn(Decimal) -> String) -> Value {
    json!({ "label": label, "value": amount(value), "amount": money(value) })
}

/// Totals in printing order. Rows without an amount are left out.
fn totals(document: &Document, amount: &impl Fn(Decimal) -> String) -> Vec<Value> {
    let base = |scheme| document.taxable(scheme);
    let tax = |scheme| document.tax_amount(scheme);
    let taxed = base(tax_scheme::IGV);
    let igv = igv_label(document);

    let rows = [
        ("GRAVADA", taxed, false),
        ("EXONERADA", base(tax_scheme::EXEMPT), false),
        ("INAFECTA", base(tax_scheme::UNAFFECTED), false),
        ("EXPORTACIÓN", base(tax_scheme::EXPORT), false),
        ("GRATUITA", base(tax_scheme::FREE), false),
        ("DESCUENTOS", document.totals.discounts, false),
        ("ISC", tax(tax_scheme::ISC), false),
        (igv.as_str(), tax(tax_scheme::IGV), !taxed.is_zero()),
        ("IVAP", tax(tax_scheme::IVAP), false),
        ("ICBPER", tax(tax_scheme::ICBPER), false),
        ("OTROS TRIBUTOS", tax(tax_scheme::OTHER), false),
        ("OTROS CARGOS", document.totals.charges, false),
        ("ANTICIPOS", document.totals.prepaid, false),
    ];

    let mut printed: Vec<Value> = rows
        .into_iter()
        .filter(|(_, value, always)| *always || !value.is_zero())
        .map(|(label, value, _)| total_row(label, value, amount))
        .collect();
    if printed.is_empty() && !document.totals.line_extension.is_zero() {
        printed.push(total_row(
            "SUBTOTAL",
            document.totals.line_extension,
            amount,
        ));
    }
    printed
}

/// `IGV 18.00 %` when every taxed line shares one rate, otherwise `IGV`.
fn igv_label(document: &Document) -> String {
    let mut rates = document
        .lines
        .iter()
        .filter_map(|line| line.igv_percent)
        .filter(|rate| !rate.is_zero());
    match rates.next() {
        Some(rate) if rates.all(|other| other == rate) => format!("IGV {rate:.2} %"),
        _ => "IGV".to_owned(),
    }
}

/// Amount in words without the `SON:` prefix: the legend of the XML when
/// present, otherwise generated.
fn amount_in_words(document: &Document) -> String {
    let Some(legend) = document.legend(AMOUNT_IN_WORDS) else {
        return words::amount_in_words(document.totals.payable, &document.currency);
    };
    let legend = legend.trim();
    match legend.get(..4) {
        Some(prefix) if prefix.eq_ignore_ascii_case("SON:") => legend[4..].trim_start().to_owned(),
        _ => legend.to_owned(),
    }
}

fn payment_form(form: PaymentForm) -> &'static str {
    match form {
        PaymentForm::Cash => "CONTADO",
        PaymentForm::Credit => "CRÉDITO",
    }
}

fn payment(document: &Document, amount: &impl Fn(Decimal) -> String) -> Value {
    let Some(payment) = &document.payment else {
        return Value::Null;
    };
    json!({
        "form": payment_form(payment.form),
        "pending": payment.pending.map(amount),
        "installments": payment
            .installments
            .iter()
            .map(|installment| json!({
                "number": installment.number,
                "due_date": installment.due_date.as_deref().map(date),
                "amount": amount(installment.amount),
            }))
            .collect::<Vec<_>>(),
    })
}

/// Detraction data. Its amount is always expressed in soles.
fn detraction(document: &Document) -> Value {
    let Some(detraction) = &document.detraction else {
        return Value::Null;
    };
    json!({
        "code": detraction.code,
        "percent": detraction.percent.map(|percent| format!("{}%", percent.normalize())),
        "amount": detraction.amount.map(|value| money_with_symbol("S/", value)),
        "account": detraction.account,
    })
}

fn affected(document: &Document) -> Value {
    let Some(affected) = &document.affected else {
        return Value::Null;
    };
    let reason_code = affected.reason_code.as_deref().unwrap_or_default();
    let reason = match document.kind {
        DocumentKind::DebitNote => catalog::debit_reason(reason_code),
        _ => catalog::credit_reason(reason_code),
    };
    json!({
        "document": format!(
            "{} {}",
            catalog::document_name(affected.type_code.as_deref().unwrap_or_default()),
            affected.id
        ),
        "reason": reason,
        "description": affected.description,
    })
}
