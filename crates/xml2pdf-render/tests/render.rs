//! Rendering behavior over the sample documents in `fixtures/`.

use std::str::FromStr;

use rust_decimal::Decimal;
use xml2pdf_core::parse_bytes;
use xml2pdf_render::{
    ConvertError, Logo, PaperFormat, RenderError, RenderOptions, Settings, builtin_template,
    config, convert, convert_with_preview, document_summary, format, render, template_data,
};

const FIXTURES: [&str; 4] = [
    "factura.xml",
    "boleta.xml",
    "nota-credito.xml",
    "nota-debito.xml",
];
const SVG_LOGO: &[u8] =
    br##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="#123456"/></svg>"##;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/../../fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|err| panic!("cannot read {path}: {err}"))
}

fn options(format: PaperFormat) -> RenderOptions {
    RenderOptions {
        settings: Settings {
            format,
            ..Settings::default()
        },
        ..RenderOptions::default()
    }
}

fn data(name: &str, options: &RenderOptions) -> serde_json::Value {
    template_data(&parse_bytes(&fixture(name)).unwrap(), options)
}

#[test]
fn renders_every_document_type_in_every_format() {
    for name in FIXTURES {
        let document = parse_bytes(&fixture(name)).unwrap();
        for format in PaperFormat::ALL {
            let pdf = render(&document, &options(format))
                .unwrap_or_else(|err| panic!("{name} in {format}: {err}"));
            assert!(pdf.starts_with(b"%PDF-"), "{name} in {format} is not a PDF");
        }
    }
}

#[test]
fn reports_the_page_size_of_each_format() {
    let page = |format| {
        convert("factura.xml", &fixture("factura.xml"), &options(format)).unwrap()[0].page_mm
    };

    let (width, height) = page(PaperFormat::A4);
    assert!(
        (width - 210.0).abs() < 0.1 && (height - 297.0).abs() < 0.1,
        "A4 was {width}x{height}"
    );
    let (width, height) = page(PaperFormat::A5);
    assert!(
        (width - 148.0).abs() < 0.1 && (height - 210.0).abs() < 0.1,
        "A5 was {width}x{height}"
    );
    let (width, height) = page(PaperFormat::Ticket80);
    assert!(
        (width - 80.0).abs() < 0.1 && height > 60.0,
        "ticket80 was {width}x{height}"
    );
    let (width, _) = page(PaperFormat::Ticket58);
    assert!((width - 58.0).abs() < 0.1, "ticket58 width was {width}");
}

#[test]
fn converts_every_document_of_a_zip() {
    let rendered = convert(
        "documentos.zip",
        &fixture("documentos.zip"),
        &RenderOptions::default(),
    )
    .unwrap();

    let stems: Vec<&str> = rendered
        .iter()
        .map(|file| file.file_stem.as_str())
        .collect();
    assert_eq!(
        stems,
        [
            "20123456789-01-F001-00000123",
            "20123456789-07-FC01-00000008"
        ]
    );
    assert!(rendered.iter().all(|file| file.pdf.starts_with(b"%PDF-")));
}

#[test]
fn names_the_failing_document_when_conversion_fails() {
    let error = convert("roto.xml", b"<Invoice/>", &RenderOptions::default()).unwrap_err();

    assert!(matches!(&error, ConvertError::Parse { name, .. } if name == "roto.xml"));
    assert_eq!(
        error.to_string(),
        "roto.xml: required field `ID` is missing"
    );
}

#[test]
fn builds_the_invoice_data_for_templates() {
    let data = data("factura.xml", &RenderOptions::default());

    assert_eq!(data["format"], "a4");
    assert_eq!(data["doc"]["title"], "FACTURA ELECTRÓNICA");
    assert_eq!(data["doc"]["id"], "F001-00000123");
    assert_eq!(data["doc"]["issue_date"], "15/01/2026");
    assert_eq!(data["doc"]["due_date"], "15/03/2026");
    assert_eq!(data["issuer"]["ruc"], "20123456789");
    assert_eq!(
        data["issuer"]["address"],
        "AV. LOS HÉROES 1234, LIMA - LIMA - LIMA"
    );
    assert_eq!(data["customer"]["doc_label"], "RUC");
    assert_eq!(
        data["amount_in_words"],
        "MIL QUINIENTOS NOVENTA Y TRES CON 00/100 SOLES"
    );
    assert_eq!(
        data["legends"],
        serde_json::json!([]),
        "the amount in words is not repeated"
    );
    assert_eq!(
        data["notes"],
        serde_json::json!(["Entrega en almacén central"])
    );
    assert_eq!(
        data["total"],
        serde_json::json!({ "label": "TOTAL", "value": "S/ 1,593.00", "amount": "1,593.00" })
    );
    assert_eq!(
        data["totals"],
        serde_json::json!([
            { "label": "GRAVADA", "value": "S/ 1,350.00", "amount": "1,350.00" },
            { "label": "IGV 18.00 %", "value": "S/ 243.00", "amount": "243.00" },
        ])
    );
    assert_eq!(
        data["lines"][1],
        serde_json::json!({
            "number": "2", "code": "CLA-030",
            "description": "CLAVOS DE ACERO 3\" CON CABEZA, PRESENTACIÓN A GRANEL",
            "unit": "KG", "unit_code": "KGM", "quantity": "2.5", "unit_value": "140.00",
            "unit_price": "165.20", "discount": "0.00", "value": "350.00", "tax": "63.00",
            "total": "413.00", "free": false,
        })
    );
    assert_eq!(data["lines"][0]["quantity"], "10");
    assert_eq!(data["payment"]["form"], "CRÉDITO");
    assert_eq!(data["payment"]["pending"], "S/ 1,593.00");
    assert_eq!(
        data["payment"]["installments"][0],
        serde_json::json!({ "number": "1", "due_date": "15/02/2026", "amount": "S/ 796.50" })
    );
    assert_eq!(data["hash"], "q1W2e3R4t5Y6u7I8o9P0aSdFgHj=");
    assert_eq!(
        data["legal"],
        "Representación impresa de la FACTURA ELECTRÓNICA"
    );
    assert_eq!(data["qr"], "qr.svg");
    assert_eq!(data["logo"], serde_json::Value::Null);
    assert!(
        data.get("accent").is_none(),
        "documents are printed without colors"
    );

    let labels: Vec<&str> = data["doc"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field["label"].as_str().unwrap())
        .collect();
    assert_eq!(
        labels,
        [
            "FECHA DE EMISIÓN",
            "FECHA DE VENCIMIENTO",
            "MONEDA",
            "FORMA DE PAGO",
            "ORDEN DE COMPRA",
            "GUÍA DE REMISIÓN"
        ]
    );
}

#[test]
fn builds_receipt_totals_and_generates_the_amount_in_words() {
    let data = data("boleta.xml", &RenderOptions::default());

    assert_eq!(data["doc"]["title"], "BOLETA DE VENTA ELECTRÓNICA");
    assert_eq!(data["customer"]["doc_label"], "DNI");
    assert_eq!(data["amount_in_words"], "SETENTA Y NUEVE CON 50/100 SOLES");
    assert_eq!(
        data["totals"],
        serde_json::json!([
            { "label": "GRAVADA", "value": "S/ 50.00", "amount": "50.00" },
            { "label": "EXONERADA", "value": "S/ 20.00", "amount": "20.00" },
            { "label": "IGV 18.00 %", "value": "S/ 9.00", "amount": "9.00" },
            { "label": "ICBPER", "value": "S/ 0.50", "amount": "0.50" },
        ])
    );
    assert_eq!(data["payment"]["form"], "CONTADO");
    assert_eq!(data["affected"], serde_json::Value::Null);
}

#[test]
fn labels_the_igv_with_its_rate_only_when_every_line_shares_it() {
    let mixed = String::from_utf8(fixture("factura.xml")).unwrap().replacen(
        "<cbc:Percent>18</cbc:Percent>",
        "<cbc:Percent>10</cbc:Percent>",
        1,
    );
    let document = parse_bytes(mixed.as_bytes()).unwrap();

    let data = template_data(&document, &RenderOptions::default());

    assert_eq!(data["totals"][1]["label"], "IGV");
}

#[test]
fn describes_the_document_modified_by_a_note() {
    let credit = data("nota-credito.xml", &RenderOptions::default());
    assert_eq!(credit["doc"]["title"], "NOTA DE CRÉDITO ELECTRÓNICA");
    assert_eq!(
        credit["affected"],
        serde_json::json!({
            "document": "FACTURA F001-00000123",
            "reason": "DEVOLUCIÓN TOTAL",
            "description": "Devolución total de la mercadería",
        })
    );

    let debit = data("nota-debito.xml", &RenderOptions::default());
    assert_eq!(debit["affected"]["reason"], "INTERESES POR MORA");
    assert_eq!(debit["doc"]["currency_symbol"], "$");
    assert_eq!(debit["total"]["value"], "$ 1,593.00");
    assert_eq!(
        debit["amount_in_words"],
        "MIL QUINIENTOS NOVENTA Y TRES CON 00/100 DÓLARES AMERICANOS"
    );
}

#[test]
fn applies_the_settings_to_the_template_data() {
    let settings = Settings {
        format: PaperFormat::Ticket58,
        issuer_lines: vec!["Tel. 01 555 0100".to_owned()],
        footer: Some("Gracias por su compra".to_owned()),
        show_qr: false,
        ..Settings::default()
    };
    let options = RenderOptions {
        settings,
        logo: Some(Logo::from_bytes(SVG_LOGO.to_vec()).unwrap()),
        template: None,
    };
    let data = data("factura.xml", &options);

    assert_eq!(data["format"], "ticket58");
    assert_eq!(
        data["page"],
        serde_json::json!({ "width": 58, "ticket": true, "paper": null })
    );
    assert_eq!(
        data["issuer"]["extra"],
        serde_json::json!(["Tel. 01 555 0100"])
    );
    assert_eq!(data["footer"], "Gracias por su compra");
    assert_eq!(data["qr"], serde_json::Value::Null);
    assert_eq!(data["logo"], "logo.svg");

    let document = parse_bytes(&fixture("factura.xml")).unwrap();
    assert!(render(&document, &options).unwrap().starts_with(b"%PDF-"));
}

#[test]
fn renders_with_a_custom_template() {
    let document = parse_bytes(&fixture("factura.xml")).unwrap();
    let custom = RenderOptions {
        template: Some(
            "#let d = json(\"data.json\")\n#set page(width: 50mm, height: 30mm)\n#d.doc.id"
                .to_owned(),
        ),
        ..RenderOptions::default()
    };
    let rendered = convert("factura.xml", &fixture("factura.xml"), &custom).unwrap();
    assert_eq!(
        rendered[0].page_mm.0.round(),
        50.0,
        "the custom page size is used"
    );

    let broken = RenderOptions {
        template: Some("#let d = json(\"data.json\")\n#d.missing_field".to_owned()),
        ..RenderOptions::default()
    };
    match render(&document, &broken) {
        Err(RenderError::Template(messages)) => {
            assert!(
                messages[0].starts_with("line 2:"),
                "unexpected message: {messages:?}"
            )
        }
        other => panic!("expected a template error, got {other:?}"),
    }
}

#[test]
fn offers_the_built_in_template_as_a_starting_point() {
    let template = builtin_template(PaperFormat::A4);
    assert!(
        template.contains("d.design.rows"),
        "the template draws a design"
    );
    for format in PaperFormat::ALL {
        assert_eq!(
            builtin_template(format),
            template,
            "one template serves {format}"
        );
    }

    // Used as a custom template, it renders like the built-in one.
    let document = parse_bytes(&fixture("boleta.xml")).unwrap();
    let options = RenderOptions {
        template: Some(template.to_owned()),
        ..options(PaperFormat::Ticket80)
    };
    assert!(render(&document, &options).unwrap().starts_with(b"%PDF-"));
}

#[test]
fn validates_settings() {
    assert_eq!(Settings::from_json("{}").unwrap(), Settings::default());
    let parsed = Settings::from_json(r#"{"format":"ticket80","show_qr":false}"#).unwrap();
    assert_eq!(parsed.format, PaperFormat::Ticket80);
    assert!(!parsed.show_qr);

    for invalid in [
        r#"{"formato":"a4"}"#,
        r#"{"format":"legal"}"#,
        r##"{"accent_color":"#aa0000"}"##,
    ] {
        assert!(
            matches!(Settings::from_json(invalid), Err(RenderError::Settings(_))),
            "{invalid} must be rejected"
        );
    }
}

#[test]
fn parses_format_names() {
    for format in PaperFormat::ALL {
        assert_eq!(PaperFormat::from_str(format.as_str()).unwrap(), format);
    }
    assert_eq!(PaperFormat::from_str("A4").unwrap(), PaperFormat::A4);
    assert!(
        matches!(PaperFormat::from_str("legal"), Err(RenderError::UnknownFormat(name)) if name == "legal")
    );
}

#[test]
fn detects_logo_image_types() {
    assert!(Logo::from_bytes(SVG_LOGO.to_vec()).is_ok());
    assert!(Logo::from_bytes(b"\x89PNG\r\n\x1a\n....".to_vec()).is_ok());
    assert!(Logo::from_bytes(b"\xFF\xD8\xFF\xE0....".to_vec()).is_ok());
    assert!(matches!(
        Logo::from_bytes(b"plain text".to_vec()),
        Err(RenderError::UnsupportedLogo)
    ));
    assert!(matches!(
        Logo::from_bytes(Vec::new()),
        Err(RenderError::UnsupportedLogo)
    ));
}

#[test]
fn formats_numbers_and_dates() {
    let dec = |value: &str| Decimal::from_str(value).unwrap();

    assert_eq!(format::money(dec("1234567.891")), "1,234,567.89");
    assert_eq!(format::money(dec("0.005")), "0.01");
    assert_eq!(format::money(dec("-1500")), "-1,500.00");
    assert_eq!(format::money_with_symbol("S/", dec("79.5")), "S/ 79.50");
    assert_eq!(format::decimals(dec("2")), "2.00");
    assert_eq!(format::decimals(dec("0.12500")), "0.125");
    assert_eq!(format::decimals(dec("1500.5")), "1,500.50");
    assert_eq!(format::quantity(dec("10")), "10");
    assert_eq!(format::quantity(dec("2.50")), "2.5");
    assert_eq!(format::quantity(dec("1500.000")), "1,500");
    assert_eq!(format::date("2026-01-15"), "15/01/2026");
    assert_eq!(format::date("2026-01-15T10:30:00"), "15/01/2026");
    assert_eq!(format::date("sin fecha"), "sin fecha");
}

#[test]
fn loads_options_from_a_configuration_file() {
    let folder = tempfile::tempdir().unwrap();
    std::fs::write(folder.path().join("logo.svg"), SVG_LOGO).unwrap();
    std::fs::write(
        folder.path().join("mine.typ"),
        "#json(\"data.json\").doc.id",
    )
    .unwrap();
    let path = folder.path().join("config.json");
    std::fs::write(
        &path,
        r#"{"format":"a5","footer":"Gracias","logo":"logo.svg","template":"mine.typ"}"#,
    )
    .unwrap();

    let options = config::load(&path).unwrap();
    assert_eq!(options.settings.format, PaperFormat::A5);
    assert_eq!(options.settings.footer.as_deref(), Some("Gracias"));
    assert_eq!(
        options.logo,
        Some(Logo::from_bytes(SVG_LOGO.to_vec()).unwrap())
    );
    assert_eq!(
        options.template.as_deref(),
        Some("#json(\"data.json\").doc.id")
    );
}

#[test]
fn rejects_invalid_configuration_files() {
    let folder = tempfile::tempdir().unwrap();
    let write = |content: &str| {
        let path = folder.path().join("config.json");
        std::fs::write(&path, content).unwrap();
        path
    };

    let unknown_key = config::load(&write(r#"{"paper":"a4"}"#))
        .unwrap_err()
        .to_string();
    assert!(
        unknown_key.contains("unknown field `paper`"),
        "{unknown_key}"
    );
    let missing_logo = config::load(&write(r#"{"logo":"nope.png"}"#))
        .unwrap_err()
        .to_string();
    assert!(missing_logo.contains("nope.png"), "{missing_logo}");
    assert!(config::load(&folder.path().join("missing.json")).is_err());
}

#[test]
fn offers_an_svg_preview_next_to_the_pdf() {
    let source = fixture("documentos.zip");

    let previews =
        convert_with_preview("documentos.zip", &source, &options(PaperFormat::Ticket80)).unwrap();
    assert_eq!(previews.len(), 2);
    for file in &previews {
        let svg = file.preview_svg.as_deref().expect("a preview is rendered");
        assert!(
            svg.starts_with("<svg"),
            "unexpected preview: {}",
            &svg[..svg.len().min(60)]
        );
        assert!(file.pdf.starts_with(b"%PDF-"));
    }

    let plain = convert("documentos.zip", &source, &options(PaperFormat::Ticket80)).unwrap();
    assert!(
        plain.iter().all(|file| file.preview_svg.is_none()),
        "previews are rendered only on request"
    );
    assert_eq!(
        plain[0].pdf.len(),
        previews[0].pdf.len(),
        "the PDF is the same with or without preview"
    );
}

#[test]
fn summarizes_a_document_for_listings() {
    let invoice = document_summary(&parse_bytes(&fixture("factura.xml")).unwrap());
    assert_eq!(
        invoice,
        serde_json::json!({
            "file_stem": "20123456789-01-F001-00000123",
            "id": "F001-00000123",
            "title": "FACTURA ELECTRÓNICA",
            "issue_date": "15/01/2026",
            "customer": "CONSTRUCTORA DEL SUR E.I.R.L.",
            "total": "S/ 1,593.00",
        })
    );

    let debit = document_summary(&parse_bytes(&fixture("nota-debito.xml")).unwrap());
    assert_eq!(debit["title"], "NOTA DE DÉBITO ELECTRÓNICA");
    assert_eq!(debit["total"], "$ 1,593.00");
}

#[test]
fn survives_amounts_too_large_to_compute_with() {
    let xml = String::from_utf8(fixture("factura.xml")).unwrap();
    // Larger than any real amount: adding two of these overflows a decimal.
    let huge = xml.replace(">1000.00<", ">79228162514264337593543950335<");
    // A quantity so small that dividing by it overflows.
    let tiny = xml
        .replace(
            "unitCode=\"NIU\">10<",
            "unitCode=\"NIU\">0.0000000000000000000000000001<",
        )
        .replace(
            "<cbc:PriceTypeCode>01</cbc:PriceTypeCode>",
            "<cbc:PriceTypeCode>99</cbc:PriceTypeCode>",
        )
        .replace(
            "<cbc:PriceAmount currencyID=\"PEN\">118.00</cbc:PriceAmount>",
            "",
        );

    for (name, hostile) in [("huge", huge), ("tiny", tiny)] {
        let rendered = convert("factura.xml", hostile.as_bytes(), &RenderOptions::default());
        assert!(rendered.is_ok(), "{name}: {:?}", rendered.err());
    }
}

#[test]
fn credits_the_tool_on_every_document() {
    for name in FIXTURES {
        let data = data(name, &RenderOptions::default());
        assert_eq!(
            data["credit"],
            serde_json::json!({
                "text": "PDF generado por qxml2pdf · Qlab by Qodi. Visítanos:",
                "links": ["https://qlab.pe", "https://qodi.pe"],
            }),
            "{name}"
        );
    }
    for format in [PaperFormat::A4, PaperFormat::Ticket80] {
        assert!(
            builtin_template(format).contains("d.credit"),
            "{format} prints the credit"
        );
    }
}
