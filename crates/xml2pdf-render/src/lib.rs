//! Renders SUNAT electronic documents to PDF with Typst.
//!
//! A template is a plain Typst file that reads `data.json` (see
//! [`template_data`]) and lays the document out. The built-in template draws
//! the rows and blocks of a [`Design`] for every paper format; a custom
//! template can replace it.

#[cfg(not(target_arch = "wasm32"))]
pub mod config;
mod design;
pub mod format;
mod view;
mod world;

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use typst::foundations::{Label, Selector};
use typst::introspection::{Introspector, MetadataElem};
use typst::layout::Abs;
use typst::utils::PicoStr;
use typst_layout::PagedDocument;
use typst_pdf::PdfOptions;
use typst_svg::SvgOptions;
use xml2pdf_core::{ArchiveError, Document, ParseError};

pub use design::{
    Align, Block, BlockKind, BoxStyle, Design, Family, HeadingStyle, ItemColumn, Preset, Row,
    Style, TextSize, catalog as design_catalog,
};
pub use view::{document_summary, template_data};
use world::TemplateWorld;

const TEMPLATE: &str = include_str!("../templates/document.typ");

/// Paper formats supported by the built-in templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaperFormat {
    #[default]
    A4,
    A5,
    /// US letter, 216 x 279 mm.
    Letter,
    /// Thermal paper roll, 80 mm wide.
    Ticket80,
    /// Thermal paper roll, 58 mm wide.
    Ticket58,
}

impl PaperFormat {
    pub const ALL: [PaperFormat; 5] = [
        PaperFormat::A4,
        PaperFormat::A5,
        PaperFormat::Letter,
        PaperFormat::Ticket80,
        PaperFormat::Ticket58,
    ];

    /// Identifier used in settings and on the command line.
    pub fn as_str(self) -> &'static str {
        match self {
            PaperFormat::A4 => "a4",
            PaperFormat::A5 => "a5",
            PaperFormat::Letter => "letter",
            PaperFormat::Ticket80 => "ticket80",
            PaperFormat::Ticket58 => "ticket58",
        }
    }

    /// Paper width in millimetres.
    pub fn width_mm(self) -> u32 {
        match self {
            PaperFormat::A4 => 210,
            PaperFormat::A5 => 148,
            PaperFormat::Letter => 216,
            PaperFormat::Ticket80 => 80,
            PaperFormat::Ticket58 => 58,
        }
    }

    /// Name of the paper in Typst, for the formats that are fixed pages.
    pub(crate) fn typst_paper(self) -> Option<&'static str> {
        match self {
            PaperFormat::A4 => Some("a4"),
            PaperFormat::A5 => Some("a5"),
            PaperFormat::Letter => Some("us-letter"),
            PaperFormat::Ticket80 | PaperFormat::Ticket58 => None,
        }
    }

    /// True for continuous thermal paper, whose height grows with the content.
    pub fn is_ticket(self) -> bool {
        matches!(self, PaperFormat::Ticket80 | PaperFormat::Ticket58)
    }
}

impl fmt::Display for PaperFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for PaperFormat {
    type Err = RenderError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        PaperFormat::ALL
            .into_iter()
            .find(|format| format.as_str() == value.to_ascii_lowercase())
            .ok_or_else(|| RenderError::UnknownFormat(value.to_owned()))
    }
}

/// User-facing settings, serializable as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub format: PaperFormat,
    /// Extra lines printed under the issuer, such as phone, email or website.
    pub issuer_lines: Vec<String>,
    /// Extra text printed at the end of the document.
    pub footer: Option<String>,
    pub show_qr: bool,
    /// Design of the page formats. `None` uses the classic design.
    pub page_design: Option<Design>,
    /// Design of the ticket formats. `None` uses the classic design.
    pub ticket_design: Option<Design>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            format: PaperFormat::default(),
            issuer_lines: Vec::new(),
            footer: None,
            show_qr: true,
            page_design: None,
            ticket_design: None,
        }
    }
}

impl Settings {
    /// Parses settings from JSON. Unknown keys are rejected so typos surface.
    pub fn from_json(json: &str) -> Result<Self, RenderError> {
        let settings: Settings =
            serde_json::from_str(json).map_err(|err| RenderError::Settings(err.to_string()))?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn validate(&self) -> Result<(), RenderError> {
        [&self.page_design, &self.ticket_design]
            .into_iter()
            .flatten()
            .try_for_each(Design::validate)
    }

    /// The design used for a format: the chosen one, or the classic preset.
    pub fn design_for(&self, format: PaperFormat) -> Design {
        let family = Family::of(format);
        let chosen = match family {
            Family::Page => &self.page_design,
            Family::Ticket => &self.ticket_design,
        };
        chosen
            .clone()
            .unwrap_or_else(|| Design::preset(Preset::Clasico, family))
    }
}

/// Issuer logo. The image type is detected from its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Logo {
    bytes: Vec<u8>,
    extension: &'static str,
}

impl Logo {
    /// Accepts PNG, JPEG, GIF, WebP and SVG images.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, RenderError> {
        let head = &bytes[..bytes.len().min(512)];
        let extension = if head.starts_with(b"\x89PNG") {
            "png"
        } else if head.starts_with(b"\xFF\xD8") {
            "jpg"
        } else if head.starts_with(b"GIF8") {
            "gif"
        } else if head.starts_with(b"RIFF") && head.get(8..12) == Some(b"WEBP") {
            "webp"
        } else if String::from_utf8_lossy(head).contains("<svg") {
            "svg"
        } else {
            return Err(RenderError::UnsupportedLogo);
        };
        Ok(Self { bytes, extension })
    }

    fn file_name(&self) -> String {
        format!("logo.{}", self.extension)
    }
}

#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    pub settings: Settings,
    pub logo: Option<Logo>,
    /// Typst source that replaces the built-in template.
    pub template: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("unknown format `{0}`: expected a4, a5, letter, ticket80 or ticket58")]
    UnknownFormat(String),
    #[error("invalid settings: {0}")]
    Settings(String),
    #[error("invalid design: {0}")]
    Design(String),
    #[error("unsupported logo: expected a PNG, JPEG, GIF, WebP or SVG image")]
    UnsupportedLogo,
    #[error("the QR code cannot be generated: {0}")]
    Qr(String),
    #[error("the template failed to compile:\n{}", .0.join("\n"))]
    Template(Vec<String>),
}

#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error("{name}: {source}")]
    Parse { name: String, source: ParseError },
    #[error("{name}: {source}")]
    Render { name: String, source: RenderError },
}

/// A PDF produced from one XML document.
#[derive(Debug, Clone)]
pub struct Rendered {
    /// Name of the XML inside the input.
    pub source_name: String,
    /// Suggested file name without extension: `RUC-TYPE-SERIES-NUMBER`.
    pub file_stem: String,
    pub pdf: Vec<u8>,
    /// Width and height of the first page, in millimetres.
    pub page_mm: (f64, f64),
    /// Every page drawn as one SVG image, when a preview was requested.
    pub preview_svg: Option<String>,
}

/// Converts an input file, either an XML document or a ZIP archive holding
/// one or more of them, into PDFs.
pub fn convert(
    name: &str,
    bytes: &[u8],
    options: &RenderOptions,
) -> Result<Vec<Rendered>, ConvertError> {
    convert_each(name, bytes, options, Mode::Pdf)
}

/// Like [`convert`], and also draws each document as an SVG image so a user
/// interface can show it without a PDF viewer.
pub fn convert_with_preview(
    name: &str,
    bytes: &[u8],
    options: &RenderOptions,
) -> Result<Vec<Rendered>, ConvertError> {
    convert_each(name, bytes, options, Mode::Preview)
}

fn convert_each(
    name: &str,
    bytes: &[u8],
    options: &RenderOptions,
    mode: Mode,
) -> Result<Vec<Rendered>, ConvertError> {
    xml2pdf_core::xml_sources(name, bytes)?
        .into_iter()
        .map(|source| {
            let document =
                xml2pdf_core::parse_bytes(&source.bytes).map_err(|err| ConvertError::Parse {
                    name: source.name.clone(),
                    source: err,
                })?;
            let output = compile(&document, options, mode).map_err(|err| ConvertError::Render {
                name: source.name.clone(),
                source: err,
            })?;
            Ok(Rendered {
                source_name: source.name,
                file_stem: document.file_stem(),
                pdf: output.pdf,
                page_mm: output.page_mm,
                preview_svg: output.preview_svg,
            })
        })
        .collect()
}

/// Source of the built-in template, a starting point for custom templates.
/// One template draws every format; the format only changes the page setup.
pub fn builtin_template(_format: PaperFormat) -> &'static str {
    TEMPLATE
}

/// What the format designer shows: the document drawn as SVG and where each
/// block of the design landed.
#[derive(Debug, Clone)]
pub struct DesignPreview {
    /// Every page drawn as one SVG image.
    pub svg: String,
    /// Size of that image in points.
    pub width_pt: f64,
    pub height_pt: f64,
    pub blocks: Vec<BlockRect>,
}

/// Place of a block inside the preview image, in points from its top left corner.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BlockRect {
    /// Identifier of the block in the design: `r<row>b<block>`.
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Renders a document for the format designer.
pub fn design_preview(
    document: &Document,
    options: &RenderOptions,
) -> Result<DesignPreview, RenderError> {
    let output = compile(document, options, Mode::Designer)?;
    Ok(output
        .designer
        .expect("the designer mode produces a preview"))
}

/// Renders a document to PDF bytes.
pub fn render(document: &Document, options: &RenderOptions) -> Result<Vec<u8>, RenderError> {
    compile(document, options, Mode::Pdf).map(|output| output.pdf)
}

/// Result of compiling one document.
struct Output {
    pdf: Vec<u8>,
    /// Size of the first page in millimetres, which printing needs for
    /// continuous ticket paper.
    page_mm: (f64, f64),
    preview_svg: Option<String>,
    designer: Option<DesignPreview>,
}

/// What a compilation produces besides the PDF.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Pdf,
    /// Also an SVG image of the document.
    Preview,
    /// An SVG image with the place of every block, for the format designer.
    Designer,
}

/// Gap between pages in the SVG preview.
const PREVIEW_PAGE_GAP_PT: f64 = 12.0;

fn compile(
    document: &Document,
    options: &RenderOptions,
    mode: Mode,
) -> Result<Output, RenderError> {
    options.settings.validate()?;
    let data = view::template_data_for(document, options, mode == Mode::Designer);
    let mut files = vec![("data.json".to_owned(), data.to_string().into_bytes())];
    if options.settings.show_qr {
        files.push((
            "qr.svg".to_owned(),
            qr_svg(&document.qr_payload())?.into_bytes(),
        ));
    }
    if let Some(logo) = &options.logo {
        files.push((logo.file_name(), logo.bytes.clone()));
    }

    let template = options
        .template
        .clone()
        .unwrap_or_else(|| builtin_template(options.settings.format).to_owned());
    let world = TemplateWorld::new(template, files);
    let describe = |diagnostics: &[typst::diag::SourceDiagnostic]| {
        RenderError::Template(
            diagnostics
                .iter()
                .map(|item| world.describe(item))
                .collect(),
        )
    };

    let paged = typst::compile::<PagedDocument>(&world)
        .output
        .map_err(|err| describe(&err))?;
    let page_mm = paged
        .pages()
        .first()
        .map(|page| page.frame.size())
        .map(|size| (size.x.to_mm(), size.y.to_mm()))
        .unwrap_or_default();
    let draw =
        || typst_svg::svg_merged(&paged, &SvgOptions::default(), Abs::pt(PREVIEW_PAGE_GAP_PT));
    let preview_svg = (mode == Mode::Preview).then(draw);
    let designer = (mode == Mode::Designer).then(|| {
        let (width_pt, height_pt, blocks) = block_rects(&paged);
        DesignPreview {
            svg: draw(),
            width_pt,
            height_pt,
            blocks,
        }
    });
    let pdf = typst_pdf::pdf(&paged, &PdfOptions::default()).map_err(|err| describe(&err))?;
    Ok(Output {
        pdf,
        page_mm,
        preview_svg,
        designer,
    })
}

/// Size of the merged preview image and the place of each block the template
/// marked, in points. Pages are stacked from the top with a fixed gap.
fn block_rects(paged: &PagedDocument) -> (f64, f64, Vec<BlockRect>) {
    let mut page_tops = Vec::new();
    let (mut width, mut height) = (0.0_f64, 0.0_f64);
    for (index, page) in paged.pages().iter().enumerate() {
        if index > 0 {
            height += PREVIEW_PAGE_GAP_PT;
        }
        page_tops.push(height);
        let size = page.frame.size();
        width = width.max(size.x.to_pt());
        height += size.y.to_pt();
    }

    let label = Label::new(PicoStr::intern("block")).expect("the label is not empty");
    let introspector = paged.introspector();
    let blocks = introspector
        .query(&Selector::Label(label))
        .iter()
        .filter_map(|content| {
            let position = introspector.position(content.location()?)?;
            let value = serde_json::to_value(&content.to_packed::<MetadataElem>()?.value).ok()?;
            Some(BlockRect {
                id: value.get("id")?.as_str()?.to_owned(),
                x: position.point.x.to_pt(),
                y: page_tops.get(position.page.get() - 1)? + position.point.y.to_pt(),
                width: value.get("w")?.as_f64()?,
                height: value.get("h")?.as_f64()?,
            })
        })
        .collect();
    (width, height, blocks)
}

fn qr_svg(payload: &str) -> Result<String, RenderError> {
    let code = qrcode::QrCode::with_error_correction_level(payload, qrcode::EcLevel::Q)
        .map_err(|err| RenderError::Qr(err.to_string()))?;
    Ok(code
        .render::<qrcode::render::svg::Color>()
        .quiet_zone(false)
        .min_dimensions(240, 240)
        .build())
}
