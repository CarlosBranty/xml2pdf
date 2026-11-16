//! Parser behavior over the sample documents in `fixtures/`.

use std::io::Write;
use std::str::FromStr;

use xml2pdf_core::{
    ArchiveError, Decimal, DocumentKind, ParseError, PaymentForm, parse_bytes, parse_str,
    tax_scheme, words, xml_sources,
};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/../../fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|err| panic!("cannot read {path}: {err}"))
}

fn dec(value: &str) -> Decimal {
    Decimal::from_str(value).unwrap()
}

#[test]
fn parses_an_invoice() {
    let doc = parse_bytes(&fixture("factura.xml")).unwrap();

    assert_eq!(doc.kind, DocumentKind::Invoice);
    assert_eq!(doc.type_code, "01");
    assert_eq!(
        (doc.id.as_str(), doc.series.as_str(), doc.number.as_str()),
        ("F001-00000123", "F001", "00000123")
    );
    assert_eq!(doc.issue_date, "2026-01-15");
    assert_eq!(doc.issue_time.as_deref(), Some("10:30:00"));
    assert_eq!(doc.currency, "PEN");
    assert_eq!(doc.operation_type.as_deref(), Some("0101"));
    assert_eq!(doc.order_reference.as_deref(), Some("OC-2026-0045"));
    assert_eq!(doc.hash.as_deref(), Some("q1W2e3R4t5Y6u7I8o9P0aSdFgHj="));

    assert_eq!(doc.supplier.doc_type, "6");
    assert_eq!(doc.supplier.doc_number, "20123456789");
    assert_eq!(doc.supplier.legal_name, "COMERCIAL ANDINA S.A.C.");
    assert_eq!(
        doc.supplier.trade_name.as_deref(),
        Some("FERRETERÍA ANDINA")
    );
    let address = doc.supplier.address.as_ref().unwrap();
    assert_eq!(
        address.one_line(),
        "AV. LOS HÉROES 1234, LIMA - LIMA - LIMA"
    );
    assert_eq!(address.ubigeo.as_deref(), Some("150101"));
    assert_eq!(
        address.urbanization, None,
        "a `-` placeholder is not an urbanization"
    );

    assert_eq!(doc.customer.doc_number, "20987654321");
    assert_eq!(doc.customer.legal_name, "CONSTRUCTORA DEL SUR E.I.R.L.");

    assert_eq!(
        doc.legend("1000"),
        Some("MIL QUINIENTOS NOVENTA Y TRES CON 00/100 SOLES")
    );
    assert_eq!(doc.notes, ["Entrega en almacén central"]);
    assert_eq!(doc.related.len(), 1);
    assert_eq!(doc.related[0].id, "T001-00000087");
    assert_eq!(doc.related[0].type_code.as_deref(), Some("09"));
}

#[test]
fn parses_invoice_lines_and_totals() {
    let doc = parse_bytes(&fixture("factura.xml")).unwrap();

    assert_eq!(doc.lines.len(), 2);
    let line = &doc.lines[1];
    assert_eq!(line.number, "2");
    assert_eq!(line.code.as_deref(), Some("CLA-030"));
    assert_eq!(
        line.description,
        "CLAVOS DE ACERO 3\" CON CABEZA, PRESENTACIÓN A GRANEL"
    );
    assert_eq!(line.unit_code, "KGM");
    assert_eq!(line.quantity, dec("2.5"));
    assert_eq!(line.unit_value, dec("140.00"));
    assert_eq!(line.unit_price, Some(dec("165.20")));
    assert_eq!(line.line_total, dec("350.00"));
    assert_eq!(line.tax_amount, dec("63.00"));
    assert_eq!(line.tax_affectation.as_deref(), Some("10"));
    assert!(!line.free);

    assert_eq!(doc.taxable(tax_scheme::IGV), dec("1350.00"));
    assert_eq!(doc.tax_amount(tax_scheme::IGV), dec("243.00"));
    assert_eq!(doc.totals.line_extension, dec("1350.00"));
    assert_eq!(doc.totals.total_taxes, dec("243.00"));
    assert_eq!(doc.totals.payable, dec("1593.00"));
}

#[test]
fn parses_credit_payment_with_installments() {
    let doc = parse_bytes(&fixture("factura.xml")).unwrap();
    let payment = doc.payment.unwrap();

    assert_eq!(payment.form, PaymentForm::Credit);
    assert_eq!(payment.pending, Some(dec("1593.00")));
    assert_eq!(payment.installments.len(), 2);
    assert_eq!(payment.installments[1].number, "2");
    assert_eq!(payment.installments[1].amount, dec("796.50"));
    assert_eq!(
        payment.installments[1].due_date.as_deref(),
        Some("2026-03-15")
    );
    assert_eq!(
        doc.due_date.as_deref(),
        Some("2026-03-15"),
        "falls back to the last installment"
    );
}

#[test]
fn parses_a_receipt_encoded_in_iso_8859_1() {
    let doc = parse_bytes(&fixture("boleta.xml")).unwrap();

    assert_eq!(doc.kind, DocumentKind::Receipt);
    assert_eq!(doc.type_code, "03");
    assert_eq!(doc.id, "B001-00004567");
    assert_eq!(doc.customer.doc_type, "1");
    assert_eq!(doc.customer.legal_name, "MARÍA FERNANDA QUISPE NUÑEZ");
    assert_eq!(
        doc.supplier.trade_name.as_deref(),
        Some("FERRETERÍA ANDINA")
    );
    assert_eq!(doc.payment.as_ref().unwrap().form, PaymentForm::Cash);
    assert_eq!(doc.taxable(tax_scheme::EXEMPT), dec("20.00"));
    assert_eq!(doc.tax_amount(tax_scheme::ICBPER), dec("0.50"));
    assert_eq!(doc.totals.payable, dec("79.50"));
    assert!(doc.legends.is_empty());
}

#[test]
fn parses_a_credit_note() {
    let doc = parse_bytes(&fixture("nota-credito.xml")).unwrap();

    assert_eq!(doc.kind, DocumentKind::CreditNote);
    assert_eq!(doc.type_code, "07");
    assert_eq!(doc.id, "FC01-00000008");
    assert_eq!(doc.lines.len(), 2);
    assert_eq!(doc.lines[0].quantity, dec("10"));
    let affected = doc.affected.unwrap();
    assert_eq!(affected.id, "F001-00000123");
    assert_eq!(affected.type_code.as_deref(), Some("01"));
    assert_eq!(affected.reason_code.as_deref(), Some("06"));
    assert_eq!(
        affected.description.as_deref(),
        Some("Devolución total de la mercadería")
    );
}

#[test]
fn parses_a_debit_note_in_dollars() {
    let doc = parse_bytes(&fixture("nota-debito.xml")).unwrap();

    assert_eq!(doc.kind, DocumentKind::DebitNote);
    assert_eq!(doc.type_code, "08");
    assert_eq!(doc.currency, "USD");
    assert_eq!(doc.lines.len(), 2);
    assert_eq!(
        doc.totals.payable,
        dec("1593.00"),
        "read from RequestedMonetaryTotal"
    );
    assert_eq!(doc.affected.unwrap().reason_code.as_deref(), Some("01"));
}

#[test]
fn builds_the_qr_payload_and_file_name() {
    let doc = parse_bytes(&fixture("factura.xml")).unwrap();

    assert_eq!(
        doc.qr_payload(),
        "20123456789|01|F001|00000123|243.00|1593.00|2026-01-15|6|20987654321|q1W2e3R4t5Y6u7I8o9P0aSdFgHj=|"
    );
    assert_eq!(doc.file_stem(), "20123456789-01-F001-00000123");
}

#[test]
fn rejects_documents_it_cannot_read() {
    assert!(matches!(parse_str("not xml"), Err(ParseError::Xml(_))));
    assert!(matches!(
        parse_str("<ApplicationResponse/>"),
        Err(ParseError::UnsupportedDocument(root)) if root == "ApplicationResponse"
    ));
    assert!(matches!(
        parse_str("<Invoice/>"),
        Err(ParseError::MissingField("ID"))
    ));
    assert!(matches!(
        parse_str("<Invoice><ID>F001-1</ID><IssueDate>2026-01-01</IssueDate></Invoice>"),
        Err(ParseError::MissingField("AccountingSupplierParty"))
    ));
}

#[test]
fn reads_xml_documents_from_a_zip_skipping_sunat_responses() {
    let sources = xml_sources("documentos.zip", &fixture("documentos.zip")).unwrap();

    let names: Vec<&str> = sources.iter().map(|source| source.name.as_str()).collect();
    assert_eq!(names, ["factura.xml", "nota-credito.xml"]);
    assert_eq!(parse_bytes(&sources[1].bytes).unwrap().id, "FC01-00000008");
}

#[test]
fn passes_a_plain_xml_through_as_a_single_source() {
    let sources = xml_sources("factura.xml", &fixture("factura.xml")).unwrap();

    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].name, "factura.xml");
    assert_eq!(sources[0].bytes, fixture("factura.xml"));
}

#[test]
fn rejects_zip_files_without_documents() {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    writer
        .start_file("leeme.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(b"sin comprobantes").unwrap();
    let without_xml = writer.finish().unwrap().into_inner();

    assert!(matches!(
        xml_sources("vacio.zip", &without_xml),
        Err(ArchiveError::Empty)
    ));
    assert!(matches!(
        xml_sources("roto.zip", b"PK\x03\x04 truncated"),
        Err(ArchiveError::Zip(_))
    ));
}

#[test]
fn writes_amounts_in_spanish_words() {
    let cases = [
        ("0", "CERO CON 00/100 SOLES"),
        ("1", "UNO CON 00/100 SOLES"),
        ("21", "VEINTIUNO CON 00/100 SOLES"),
        ("79.50", "SETENTA Y NUEVE CON 50/100 SOLES"),
        ("100", "CIEN CON 00/100 SOLES"),
        ("101", "CIENTO UNO CON 00/100 SOLES"),
        ("1000", "MIL CON 00/100 SOLES"),
        ("1593", "MIL QUINIENTOS NOVENTA Y TRES CON 00/100 SOLES"),
        ("21000", "VEINTIUN MIL CON 00/100 SOLES"),
        ("1000000", "UN MILLON CON 00/100 SOLES"),
        ("2500000.05", "DOS MILLONES QUINIENTOS MIL CON 05/100 SOLES"),
        ("0.995", "UNO CON 00/100 SOLES"),
    ];
    for (amount, expected) in cases {
        assert_eq!(
            words::amount_in_words(dec(amount), "PEN"),
            expected,
            "amount {amount}"
        );
    }
    assert_eq!(
        words::amount_in_words(dec("15"), "USD"),
        "QUINCE CON 00/100 DÓLARES AMERICANOS"
    );
}
