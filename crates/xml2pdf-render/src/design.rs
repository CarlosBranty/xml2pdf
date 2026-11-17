//! Designs: how a document is laid out.
//!
//! A design is a list of rows; each row holds one to three blocks placed side
//! by side, such as the issuer, the customer or the items table. Style
//! options apply to the whole document. Designs are plain data (JSON), so an
//! editor can build them and every surface renders them the same way.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{PaperFormat, RenderError};

/// Most rows a design may have, and most blocks in one row.
const MAX_ROWS: usize = 40;
const MAX_BLOCKS_PER_ROW: usize = 3;
/// Largest relative width of a block inside its row.
const MAX_WIDTH: u8 = 6;
/// Longest free text of a text block.
const MAX_TEXT_CHARS: usize = 600;

/// Kind of paper a design is made for. Pages and tickets are laid out differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Page,
    Ticket,
}

impl Family {
    pub fn of(format: PaperFormat) -> Self {
        if format.is_ticket() {
            Family::Ticket
        } else {
            Family::Page
        }
    }
}

/// Ready-made designs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// Boxed layout with shaded headings, as usual on Peruvian invoices.
    Clasico,
    /// Black headings and thin lines instead of boxes.
    Moderno,
    /// The classic layout with smaller text and no filler rows.
    Compacto,
    /// No boxes and no shading.
    Minimo,
}

impl Preset {
    pub const ALL: [Preset; 4] = [
        Preset::Clasico,
        Preset::Moderno,
        Preset::Compacto,
        Preset::Minimo,
    ];

    /// Name shown to users.
    pub fn label(self) -> &'static str {
        match self {
            Preset::Clasico => "Clásico",
            Preset::Moderno => "Moderno",
            Preset::Compacto => "Compacto",
            Preset::Minimo => "Mínimo",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextSize {
    Small,
    #[default]
    Normal,
    Large,
}

/// How boxed blocks are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoxStyle {
    #[default]
    Rounded,
    Square,
    /// A line under the block instead of a box.
    Lines,
    None,
}

/// How the document box and the items heading are filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadingStyle {
    #[default]
    Shaded,
    /// White text on black.
    Inverted,
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Style {
    pub text: TextSize,
    pub boxes: BoxStyle,
    pub heading: HeadingStyle,
    /// Shade every other row of the items table.
    pub stripes: bool,
    /// Pad a short items table with empty rows.
    pub filler_rows: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            text: TextSize::Normal,
            boxes: BoxStyle::Rounded,
            heading: HeadingStyle::Shaded,
            stripes: true,
            filler_rows: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    /// Logo, issuer name, address and extra lines.
    Issuer,
    /// RUC, document title and number.
    Document,
    Customer,
    /// Issue date, due date and currency.
    Dates,
    /// Document modified by a credit or debit note. Hidden on other documents.
    Affected,
    Items,
    Totals,
    /// Amount in words.
    Words,
    /// Related documents, payment terms and remarks.
    Details,
    /// Legal text, signature hash and configured footer.
    Legal,
    Qr,
    /// Credit of the tool.
    Credit,
    /// Free text written by the user.
    Text,
    /// Separator line.
    Rule,
    /// Empty space.
    Spacer,
}

impl BlockKind {
    pub const ALL: [BlockKind; 15] = [
        BlockKind::Issuer,
        BlockKind::Document,
        BlockKind::Customer,
        BlockKind::Dates,
        BlockKind::Affected,
        BlockKind::Items,
        BlockKind::Totals,
        BlockKind::Words,
        BlockKind::Details,
        BlockKind::Legal,
        BlockKind::Qr,
        BlockKind::Credit,
        BlockKind::Text,
        BlockKind::Rule,
        BlockKind::Spacer,
    ];

    /// Name shown to users.
    pub fn label(self) -> &'static str {
        match self {
            BlockKind::Issuer => "Logo y emisor",
            BlockKind::Document => "RUC y número",
            BlockKind::Customer => "Datos del cliente",
            BlockKind::Dates => "Fechas y moneda",
            BlockKind::Affected => "Documento que modifica",
            BlockKind::Items => "Tabla de ítems",
            BlockKind::Totals => "Totales",
            BlockKind::Words => "Importe en letras",
            BlockKind::Details => "Pago y observaciones",
            BlockKind::Legal => "Texto legal y resumen",
            BlockKind::Qr => "Código QR",
            BlockKind::Credit => "Crédito de la herramienta",
            BlockKind::Text => "Texto libre",
            BlockKind::Rule => "Línea separadora",
            BlockKind::Spacer => "Espacio",
        }
    }

    /// Whether the block is drawn inside a box unless the design says otherwise.
    fn boxed_by_default(self, family: Family) -> bool {
        family == Family::Page
            && matches!(
                self,
                BlockKind::Document
                    | BlockKind::Customer
                    | BlockKind::Dates
                    | BlockKind::Affected
                    | BlockKind::Words
                    | BlockKind::Legal
                    | BlockKind::Qr
            )
    }

    fn default_align(self, family: Family) -> Align {
        match self {
            BlockKind::Document | BlockKind::Qr | BlockKind::Credit => Align::Center,
            BlockKind::Totals => Align::Right,
            BlockKind::Issuer | BlockKind::Legal if family == Family::Ticket => Align::Center,
            _ => Align::Left,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Columns the items table can show. The names match the item fields of the
/// template data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemColumn {
    Quantity,
    UnitCode,
    Unit,
    Code,
    Description,
    UnitValue,
    UnitPrice,
    Discount,
    Value,
    Tax,
    Total,
}

impl ItemColumn {
    pub const ALL: [ItemColumn; 11] = [
        ItemColumn::Quantity,
        ItemColumn::UnitCode,
        ItemColumn::Unit,
        ItemColumn::Code,
        ItemColumn::Description,
        ItemColumn::UnitValue,
        ItemColumn::UnitPrice,
        ItemColumn::Discount,
        ItemColumn::Value,
        ItemColumn::Tax,
        ItemColumn::Total,
    ];

    /// Columns of the classic layout.
    pub const DEFAULT: [ItemColumn; 7] = [
        ItemColumn::Quantity,
        ItemColumn::UnitCode,
        ItemColumn::Code,
        ItemColumn::Description,
        ItemColumn::UnitValue,
        ItemColumn::UnitPrice,
        ItemColumn::Total,
    ];

    /// Name shown to users.
    pub fn label(self) -> &'static str {
        match self {
            ItemColumn::Quantity => "Cantidad",
            ItemColumn::UnitCode => "Unidad (código SUNAT)",
            ItemColumn::Unit => "Unidad (abreviada)",
            ItemColumn::Code => "Código",
            ItemColumn::Description => "Descripción",
            ItemColumn::UnitValue => "Valor unitario",
            ItemColumn::UnitPrice => "Precio unitario",
            ItemColumn::Discount => "Descuento",
            ItemColumn::Value => "Valor de venta",
            ItemColumn::Tax => "IGV del ítem",
            ItemColumn::Total => "Importe con IGV",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
    #[serde(rename = "type")]
    pub kind: BlockKind,
    /// Width relative to the other blocks of the row.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub width: u8,
    /// Draw the block inside a box. `None` keeps the default of its kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boxed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
    /// Content of a text block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Columns of an items block. `None` shows the classic ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<ItemColumn>>,
}

fn one() -> u8 {
    1
}

fn is_one(width: &u8) -> bool {
    *width == 1
}

impl Block {
    pub fn new(kind: BlockKind) -> Self {
        Self {
            kind,
            width: 1,
            boxed: None,
            align: None,
            text: None,
            columns: None,
        }
    }

    fn wide(kind: BlockKind, width: u8) -> Self {
        Self {
            width,
            ..Self::new(kind)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub blocks: Vec<Block>,
}

impl Row {
    fn of(blocks: impl IntoIterator<Item = Block>) -> Self {
        Self {
            blocks: blocks.into_iter().collect(),
        }
    }

    fn single(kind: BlockKind) -> Self {
        Self::of([Block::new(kind)])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Design {
    #[serde(default)]
    pub style: Style,
    pub rows: Vec<Row>,
}

impl Design {
    /// A ready-made design for a kind of paper.
    pub fn preset(preset: Preset, family: Family) -> Self {
        use BlockKind::*;
        let classic = Style::default();
        let style = match preset {
            Preset::Clasico => classic,
            Preset::Moderno => Style {
                boxes: BoxStyle::Lines,
                heading: HeadingStyle::Inverted,
                stripes: false,
                filler_rows: false,
                ..classic
            },
            Preset::Compacto => Style {
                text: TextSize::Small,
                boxes: BoxStyle::Square,
                filler_rows: false,
                ..classic
            },
            Preset::Minimo => Style {
                boxes: BoxStyle::None,
                heading: HeadingStyle::Plain,
                stripes: false,
                filler_rows: false,
                ..classic
            },
        };

        let rows = match family {
            Family::Ticket => {
                let with_rules = preset != Preset::Minimo;
                let mut rows = vec![Row::single(Issuer)];
                let sections: [&[BlockKind]; 5] = [
                    &[Document],
                    &[Dates, Customer, Affected],
                    &[Items],
                    &[Totals, Words, Details],
                    &[Qr, Legal, Credit],
                ];
                for section in sections {
                    if with_rules {
                        rows.push(Row::single(Rule));
                    }
                    rows.extend(section.iter().map(|kind| Row::single(*kind)));
                }
                rows
            }
            Family::Page if preset == Preset::Moderno => vec![
                Row::of([Block::wide(Issuer, 3), Block::wide(Document, 2)]),
                Row::single(Rule),
                Row::of([Block::wide(Customer, 3), Block::wide(Dates, 2)]),
                Row::single(Affected),
                Row::single(Items),
                Row::of([Block::wide(Words, 3), Block::wide(Totals, 2)]),
                Row::single(Details),
                Row::of([Block::wide(Legal, 5), Block::new(Qr)]),
                Row::single(Credit),
            ],
            Family::Page => vec![
                Row::of([Block::wide(Issuer, 3), Block::wide(Document, 2)]),
                Row::of([Block::wide(Customer, 3), Block::wide(Dates, 2)]),
                Row::single(Affected),
                Row::single(Items),
                Row::single(Totals),
                Row::single(Words),
                Row::single(Details),
                Row::of([Block::wide(Legal, 5), Block::new(Qr)]),
                Row::single(Credit),
            ],
        };
        Self { style, rows }
    }

    /// Parses a design from JSON and checks it.
    pub fn from_json(json: &str) -> Result<Self, RenderError> {
        let design: Design =
            serde_json::from_str(json).map_err(|err| RenderError::Design(err.to_string()))?;
        design.validate()?;
        Ok(design)
    }

    pub fn validate(&self) -> Result<(), RenderError> {
        let invalid = |message: String| Err(RenderError::Design(message));
        if self.rows.len() > MAX_ROWS {
            return invalid(format!("a design can have at most {MAX_ROWS} rows"));
        }
        for (index, row) in self.rows.iter().enumerate() {
            let number = index + 1;
            if row.blocks.is_empty() || row.blocks.len() > MAX_BLOCKS_PER_ROW {
                return invalid(format!(
                    "row {number} must have between 1 and {MAX_BLOCKS_PER_ROW} blocks"
                ));
            }
            for block in &row.blocks {
                if !(1..=MAX_WIDTH).contains(&block.width) {
                    return invalid(format!(
                        "row {number}: a block width must be between 1 and {MAX_WIDTH}"
                    ));
                }
                if block
                    .text
                    .as_ref()
                    .is_some_and(|text| text.chars().count() > MAX_TEXT_CHARS)
                {
                    return invalid(format!(
                        "row {number}: a text block can have at most {MAX_TEXT_CHARS} characters"
                    ));
                }
                if block.columns.as_ref().is_some_and(Vec::is_empty) {
                    return invalid(format!(
                        "row {number}: the items table needs at least one column"
                    ));
                }
            }
        }
        Ok(())
    }

    /// The design as templates receive it: every default filled in and an
    /// identifier (`r<row>b<block>`) on each block.
    pub(crate) fn resolved(&self, family: Family) -> Value {
        let rows: Vec<Value> = self
            .rows
            .iter()
            .enumerate()
            .map(|(row_index, row)| {
                let blocks: Vec<Value> = row
                    .blocks
                    .iter()
                    .enumerate()
                    .map(|(block_index, block)| {
                        json!({
                            "id": format!("r{row_index}b{block_index}"),
                            "type": block.kind,
                            "width": block.width,
                            "boxed": block.boxed.unwrap_or_else(|| block.kind.boxed_by_default(family)),
                            "align": block.align.unwrap_or_else(|| block.kind.default_align(family)),
                            "text": block.text.clone().unwrap_or_default(),
                            "columns": block.columns.clone().unwrap_or_else(|| ItemColumn::DEFAULT.to_vec()),
                        })
                    })
                    .collect();
                json!({ "blocks": blocks })
            })
            .collect();
        json!({ "style": self.style, "rows": rows })
    }
}

/// What a design editor needs to offer: presets, blocks and item columns,
/// with the names shown to users.
pub fn catalog() -> Value {
    let named = |id: Value, label: &str| json!({ "id": id, "label": label });
    json!({
        "presets": Preset::ALL.map(|preset| named(json!(preset), preset.label())),
        "blocks": BlockKind::ALL.map(|kind| named(json!(kind), kind.label())),
        "columns": ItemColumn::ALL.map(|column| named(json!(column), column.label())),
        "limits": { "rows": MAX_ROWS, "blocks_per_row": MAX_BLOCKS_PER_ROW, "width": MAX_WIDTH, "text": MAX_TEXT_CHARS },
    })
}
