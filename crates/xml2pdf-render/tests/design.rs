//! Designs: rows of blocks, presets, and the preview used by the editor.

use xml2pdf_core::parse_bytes;
use xml2pdf_render::{
    Block, BlockKind, Design, Family, ItemColumn, PaperFormat, Preset, RenderError, RenderOptions,
    Row, Settings, convert, design_catalog, design_preview, render, template_data,
};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/../../fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|err| panic!("cannot read {path}: {err}"))
}

fn options(format: PaperFormat, design: Option<Design>) -> RenderOptions {
    let family = Family::of(format);
    let settings = Settings {
        format,
        page_design: design.clone().filter(|_| family == Family::Page),
        ticket_design: design.filter(|_| family == Family::Ticket),
        ..Settings::default()
    };
    RenderOptions {
        settings,
        ..RenderOptions::default()
    }
}

fn rows_of(kinds: &[&[BlockKind]]) -> Vec<Row> {
    kinds
        .iter()
        .map(|row| Row {
            blocks: row.iter().map(|kind| Block::new(*kind)).collect(),
        })
        .collect()
}

#[test]
fn renders_every_preset_for_pages_and_tickets() {
    for name in [
        "factura.xml",
        "boleta.xml",
        "nota-credito.xml",
        "nota-debito.xml",
    ] {
        let document = parse_bytes(&fixture(name)).unwrap();
        for preset in Preset::ALL {
            for format in [PaperFormat::A4, PaperFormat::Ticket80] {
                let design = Design::preset(preset, Family::of(format));
                let pdf = render(&document, &options(format, Some(design)))
                    .unwrap_or_else(|err| panic!("{name}, {preset:?}, {format}: {err}"));
                assert!(pdf.starts_with(b"%PDF-"));
            }
        }
    }
}

#[test]
fn uses_the_classic_design_when_none_is_chosen() {
    let document = parse_bytes(&fixture("factura.xml")).unwrap();
    for format in [PaperFormat::A4, PaperFormat::Ticket58] {
        let default = template_data(&document, &options(format, None));
        let classic = Design::preset(Preset::Clasico, Family::of(format));
        let chosen = template_data(&document, &options(format, Some(classic)));
        assert_eq!(default["design"], chosen["design"], "{format}");
    }
}

#[test]
fn hands_templates_a_design_with_every_default_filled_in() {
    let document = parse_bytes(&fixture("factura.xml")).unwrap();
    let data = template_data(&document, &options(PaperFormat::A4, None));

    assert_eq!(
        data["design"]["style"],
        serde_json::json!({
            "text": "normal", "boxes": "rounded", "heading": "shaded",
            "stripes": true, "filler_rows": true,
        })
    );
    let first_row = &data["design"]["rows"][0]["blocks"];
    assert_eq!(
        first_row[0],
        serde_json::json!({
            "id": "r0b0", "type": "issuer", "width": 3, "boxed": false, "align": "left",
            "text": "", "columns": ["quantity", "unit_code", "code", "description", "unit_value", "unit_price", "total"],
        })
    );
    assert_eq!(first_row[1]["id"], "r0b1");
    assert_eq!(first_row[1]["type"], "document");
    assert_eq!(first_row[1]["boxed"], true);
    assert_eq!(first_row[1]["align"], "center");

    let ticket = template_data(&document, &options(PaperFormat::Ticket80, None));
    assert_eq!(
        ticket["design"]["rows"][0]["blocks"][0]["align"], "center",
        "tickets center the issuer"
    );
    assert_eq!(
        ticket["design"]["rows"][2]["blocks"][0]["boxed"], false,
        "tickets draw no boxes"
    );
}

#[test]
fn reads_and_writes_designs_as_json() {
    let design = Design::preset(Preset::Moderno, Family::Page);
    let json = serde_json::to_string(&design).unwrap();
    assert_eq!(Design::from_json(&json).unwrap(), design);

    let custom = Design::from_json(
        r#"{"rows":[{"blocks":[{"type":"document"},{"type":"text","text":"Gracias","width":2,"align":"right"}]},
            {"blocks":[{"type":"items","columns":["description","total"],"boxed":false}]}]}"#,
    )
    .unwrap();
    assert_eq!(custom.rows[0].blocks[1].text.as_deref(), Some("Gracias"));
    assert_eq!(
        custom.rows[1].blocks[0].columns,
        Some(vec![ItemColumn::Description, ItemColumn::Total])
    );

    let settings =
        Settings::from_json(&format!(r#"{{"format":"a5","page_design":{json}}}"#)).unwrap();
    assert_eq!(settings.page_design, Some(design));
    assert_eq!(settings.ticket_design, None);
}

#[test]
fn rejects_invalid_designs() {
    let rejected = |json: &str| match Design::from_json(json) {
        Err(RenderError::Design(message)) => message,
        other => panic!("expected a design error for {json}, got {other:?}"),
    };

    assert!(
        rejected(r#"{"rows":[{"blocks":[{"type":"banner"}]}]}"#)
            .contains("unknown variant `banner`")
    );
    assert!(
        rejected(r#"{"rows":[{"blocks":[]}]}"#).contains("row 1 must have between 1 and 3 blocks")
    );
    let crowded =
        r#"{"rows":[{"blocks":[{"type":"qr"},{"type":"qr"},{"type":"qr"},{"type":"qr"}]}]}"#;
    assert!(rejected(crowded).contains("row 1 must have between 1 and 3 blocks"));
    assert!(
        rejected(r#"{"rows":[{"blocks":[{"type":"qr","width":9}]}]}"#)
            .contains("width must be between 1 and 6")
    );
    assert!(
        rejected(r#"{"rows":[{"blocks":[{"type":"items","columns":[]}]}]}"#)
            .contains("at least one column")
    );
    assert!(
        rejected(r#"{"rows":[{"blocks":[{"type":"qr","colour":"red"}]}]}"#)
            .contains("unknown field `colour`")
    );
    let long = format!(
        r#"{{"rows":[{{"blocks":[{{"type":"text","text":"{}"}}]}}]}}"#,
        "a".repeat(601)
    );
    assert!(rejected(&long).contains("at most 600 characters"));

    // An invalid design inside the settings is refused when rendering too.
    let document = parse_bytes(&fixture("factura.xml")).unwrap();
    let empty_row = Design {
        rows: vec![Row { blocks: Vec::new() }],
        ..Design::preset(Preset::Clasico, Family::Page)
    };
    assert!(matches!(
        render(&document, &options(PaperFormat::A4, Some(empty_row))),
        Err(RenderError::Design(_))
    ));
}

#[test]
fn locates_every_visible_block_in_the_preview() {
    let document = parse_bytes(&fixture("factura.xml")).unwrap();
    let preview = design_preview(&document, &options(PaperFormat::A4, None)).unwrap();

    assert!(preview.svg.starts_with("<svg"));
    assert!((preview.width_pt - 595.28).abs() < 0.5 && (preview.height_pt - 841.89).abs() < 0.5);

    let ids: Vec<&str> = preview
        .blocks
        .iter()
        .map(|block| block.id.as_str())
        .collect();
    // Row 2 is the block for the document a note modifies: an invoice has none.
    assert_eq!(
        ids,
        [
            "r0b0", "r0b1", "r1b0", "r1b1", "r3b0", "r4b0", "r5b0", "r6b0", "r7b0", "r7b1", "r8b0"
        ]
    );
    for block in &preview.blocks {
        assert!(block.width > 10.0 && block.height > 3.0, "{block:?}");
        assert!(
            block.x >= 0.0 && block.x + block.width <= preview.width_pt + 0.5,
            "{block:?}"
        );
        assert!(
            block.y >= 0.0 && block.y + block.height <= preview.height_pt + 0.5,
            "{block:?}"
        );
    }

    let by_id = |id: &str| preview.blocks.iter().find(|block| block.id == id).unwrap();
    let (issuer, number) = (by_id("r0b0"), by_id("r0b1"));
    assert!(
        (issuer.y - number.y).abs() < 0.5,
        "blocks of a row start at the same height"
    );
    assert!(
        issuer.x + issuer.width <= number.x + 0.5,
        "and sit side by side"
    );
    assert!(
        issuer.width > number.width,
        "the wider block gets more room"
    );
    assert!(
        by_id("r1b0").y > issuer.y + issuer.height - 0.5,
        "the next row is below"
    );

    let note = parse_bytes(&fixture("nota-credito.xml")).unwrap();
    let with_affected = design_preview(&note, &options(PaperFormat::A4, None)).unwrap();
    assert!(
        with_affected.blocks.iter().any(|block| block.id == "r2b0"),
        "a note shows the modified document"
    );
}

#[test]
fn draws_only_the_blocks_of_a_custom_design() {
    let document = parse_bytes(&fixture("factura.xml")).unwrap();
    let design = Design {
        rows: rows_of(&[
            &[BlockKind::Document],
            &[BlockKind::Items],
            &[BlockKind::Totals, BlockKind::Qr],
        ]),
        ..Design::preset(Preset::Minimo, Family::Page)
    };
    let preview =
        design_preview(&document, &options(PaperFormat::A4, Some(design.clone()))).unwrap();
    let ids: Vec<&str> = preview
        .blocks
        .iter()
        .map(|block| block.id.as_str())
        .collect();
    assert_eq!(ids, ["r0b0", "r1b0", "r2b0", "r2b1"]);

    // The same design gives the same PDF with or without the editor marks.
    let rendered = convert(
        "factura.xml",
        &fixture("factura.xml"),
        &options(PaperFormat::A4, Some(design)),
    )
    .unwrap();
    assert!(rendered[0].pdf.starts_with(b"%PDF-"));
    assert!((rendered[0].page_mm.0 - 210.0).abs() < 0.1);
}

#[test]
fn offers_letter_paper() {
    let rendered = convert(
        "factura.xml",
        &fixture("factura.xml"),
        &options(PaperFormat::Letter, None),
    )
    .unwrap();
    let (width, height) = rendered[0].page_mm;
    assert!(
        (width - 215.9).abs() < 0.2 && (height - 279.4).abs() < 0.2,
        "letter was {width}x{height}"
    );
    assert_eq!(
        "letter".parse::<PaperFormat>().unwrap(),
        PaperFormat::Letter
    );
    assert_eq!(Family::of(PaperFormat::Letter), Family::Page);
}

#[test]
fn describes_what_an_editor_can_offer() {
    let catalog = design_catalog();
    assert_eq!(
        catalog["presets"],
        serde_json::json!([
            { "id": "clasico", "label": "Clásico" }, { "id": "moderno", "label": "Moderno" },
            { "id": "compacto", "label": "Compacto" }, { "id": "minimo", "label": "Mínimo" },
        ])
    );
    assert_eq!(catalog["blocks"].as_array().unwrap().len(), 15);
    assert_eq!(
        catalog["blocks"][5],
        serde_json::json!({ "id": "items", "label": "Tabla de ítems" })
    );
    assert_eq!(catalog["columns"].as_array().unwrap().len(), 11);
    assert_eq!(catalog["limits"]["blocks_per_row"], 3);
}
