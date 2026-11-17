//! Domain model of a SUNAT electronic document.
//!
//! The model keeps the values as they appear in the XML (ISO dates, catalog
//! codes, exact decimals). Presentation concerns such as labels and number
//! formatting belong to the renderer.

use rust_decimal::{Decimal, RoundingStrategy};
use serde::Serialize;

/// SUNAT tax scheme codes (catalog 05) used to classify amounts.
pub mod tax_scheme {
    pub const IGV: &str = "1000";
    pub const IVAP: &str = "1016";
    pub const ISC: &str = "2000";
    pub const ICBPER: &str = "7152";
    pub const EXPORT: &str = "9995";
    pub const FREE: &str = "9996";
    pub const EXEMPT: &str = "9997";
    pub const UNAFFECTED: &str = "9998";
    pub const OTHER: &str = "9999";
}

/// Kind of electronic document, derived from the XML root and type code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    /// Factura (type code 01).
    Invoice,
    /// Boleta de venta (type code 03).
    Receipt,
    /// Nota de crédito (type code 07).
    CreditNote,
    /// Nota de débito (type code 08).
    DebitNote,
}

#[derive(Debug, Clone, Serialize)]
pub struct Document {
    pub kind: DocumentKind,
    /// Catalog 01 code: `01`, `03`, `07`, `08`.
    pub type_code: String,
    /// Full identifier, for example `F001-00000123`.
    pub id: String,
    pub series: String,
    pub number: String,
    /// ISO date (`YYYY-MM-DD`).
    pub issue_date: String,
    pub issue_time: Option<String>,
    pub due_date: Option<String>,
    /// ISO 4217 currency code.
    pub currency: String,
    /// Catalog 51 operation type (`0101` internal sale, ...).
    pub operation_type: Option<String>,
    pub supplier: Party,
    pub customer: Party,
    pub lines: Vec<Line>,
    pub taxes: Vec<TaxSubtotal>,
    pub totals: Totals,
    /// Notes carrying a `languageLocaleID` (catalog 52), such as the amount in words.
    pub legends: Vec<Legend>,
    /// Free-text notes without a catalog code.
    pub notes: Vec<String>,
    pub payment: Option<Payment>,
    pub detraction: Option<Detraction>,
    /// Document modified by a credit or debit note.
    pub affected: Option<AffectedDocument>,
    pub related: Vec<RelatedDocument>,
    pub order_reference: Option<String>,
    /// Digest of the XML signature, printed as the document hash.
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Party {
    /// Catalog 06 identity document type (`6` RUC, `1` DNI, ...).
    pub doc_type: String,
    pub doc_number: String,
    pub legal_name: String,
    pub trade_name: Option<String>,
    pub address: Option<Address>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Address {
    pub line: Option<String>,
    pub urbanization: Option<String>,
    pub district: Option<String>,
    pub province: Option<String>,
    pub department: Option<String>,
    pub ubigeo: Option<String>,
    pub country: Option<String>,
}

impl Address {
    /// Single-line address: street, urbanization and `DISTRICT - PROVINCE - DEPARTMENT`.
    pub fn one_line(&self) -> String {
        let region = [&self.district, &self.province, &self.department]
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" - ");
        [
            self.line.as_deref(),
            self.urbanization.as_deref(),
            Some(region.as_str()),
        ]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Line {
    pub number: String,
    pub code: Option<String>,
    pub description: String,
    /// Catalog 03 unit of measure (`NIU`, `ZZ`, ...).
    pub unit_code: String,
    pub quantity: Decimal,
    /// Unit value without taxes.
    pub unit_value: Decimal,
    /// Unit price including taxes (price type 01), when present.
    pub unit_price: Option<Decimal>,
    /// Line value without taxes.
    pub line_total: Decimal,
    pub tax_amount: Decimal,
    pub discount: Decimal,
    /// Catalog 07 IGV affectation code (`10` gravado, `20` exonerado, ...).
    pub tax_affectation: Option<String>,
    /// IGV rate applied to the line, for example `18`. `None` when the line pays no IGV.
    pub igv_percent: Option<Decimal>,
    /// True for free-of-charge items (price type 02).
    pub free: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaxSubtotal {
    /// Catalog 05 tax scheme code, see [`tax_scheme`].
    pub scheme_id: String,
    pub scheme_name: String,
    pub taxable: Decimal,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Totals {
    /// Sum of line values without taxes.
    pub line_extension: Decimal,
    pub total_taxes: Decimal,
    pub tax_inclusive: Decimal,
    pub discounts: Decimal,
    pub charges: Decimal,
    pub prepaid: Decimal,
    pub payable: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct Legend {
    /// Catalog 52 code (`1000` amount in words, `2006` detraction, ...).
    pub code: String,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentForm {
    Cash,
    Credit,
}

#[derive(Debug, Clone, Serialize)]
pub struct Payment {
    pub form: PaymentForm,
    /// Net amount pending payment, for credit sales.
    pub pending: Option<Decimal>,
    pub installments: Vec<Installment>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Installment {
    pub number: String,
    pub amount: Decimal,
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Detraction {
    /// Catalog 54 goods or service code.
    pub code: Option<String>,
    pub percent: Option<Decimal>,
    pub amount: Option<Decimal>,
    /// Banco de la Nación account.
    pub account: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AffectedDocument {
    pub id: String,
    pub type_code: Option<String>,
    /// Catalog 09 (credit) or 10 (debit) reason code.
    pub reason_code: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelatedDocument {
    pub id: String,
    pub type_code: Option<String>,
}

impl Document {
    /// Taxable base reported for a tax scheme, summed over the subtotals.
    pub fn taxable(&self, scheme_id: &str) -> Decimal {
        self.taxes
            .iter()
            .filter(|tax| tax.scheme_id == scheme_id)
            .map(|tax| tax.taxable)
            .sum()
    }

    /// Tax amount reported for a tax scheme, summed over the subtotals.
    pub fn tax_amount(&self, scheme_id: &str) -> Decimal {
        self.taxes
            .iter()
            .filter(|tax| tax.scheme_id == scheme_id)
            .map(|tax| tax.amount)
            .sum()
    }

    /// Text of the first legend with the given catalog 52 code.
    pub fn legend(&self, code: &str) -> Option<&str> {
        self.legends
            .iter()
            .find(|legend| legend.code == code)
            .map(|legend| legend.text.as_str())
    }

    /// Payload of the QR code required on the printed representation:
    /// `RUC|TYPE|SERIES|NUMBER|IGV|TOTAL|DATE|CUSTOMER DOC TYPE|CUSTOMER DOC|HASH|`.
    pub fn qr_payload(&self) -> String {
        format!(
            "{}|{}|{}|{}|{:.2}|{:.2}|{}|{}|{}|{}|",
            self.supplier.doc_number,
            self.type_code,
            self.series,
            self.number,
            round2(self.tax_amount(tax_scheme::IGV)),
            round2(self.totals.payable),
            self.issue_date,
            self.customer.doc_type,
            self.customer.doc_number,
            self.hash.as_deref().unwrap_or_default(),
        )
    }

    /// Suggested PDF file name without extension: `RUC-TYPE-SERIES-NUMBER`.
    ///
    /// The values come from the XML, so every character other than letters,
    /// digits, `-` and `_` is replaced: the name can never leave its folder.
    pub fn file_stem(&self) -> String {
        format!(
            "{}-{}-{}",
            self.supplier.doc_number, self.type_code, self.id
        )
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect()
    }
}

fn round2(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}
