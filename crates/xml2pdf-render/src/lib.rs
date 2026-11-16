//! Renders SUNAT electronic documents to PDF with Typst.
//!
//! A template is a plain Typst file that reads `data.json` (see
//! [`template_data`]) and lays the document out. The built-in templates cover
//! A4, A5 and thermal tickets; a custom template can replace them.

#[cfg(not(target_arch = "wasm32"))]
pub mod config;
pub mod format;
mod view;
mod world;

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use typst_layout::PagedDocument;
use typst_pdf::PdfOptions;
use xml2pdf_core::{ArchiveError, Document, ParseError};

pub use view::template_data;
use world::TemplateWorld;

const PAGE_TEMPLATE: &str = include_str!("../templates/page.typ");
const TICKET_TEMPLATE: &str = include_str!("../templates/ticket.typ");

/// Paper formats supported by the built-in templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaperFormat {
    #[default]
    A4,
    A5,
    /// Thermal paper roll, 80 mm wide.
    Ticket80,
    /// Thermal paper roll, 58 mm wide.
    Ticket58,
}

impl PaperFormat {
    pub const ALL: [PaperFormat; 4] = [
        PaperFormat::A4,
        PaperFormat::A5,
        PaperFormat::Ticket80,
        PaperFormat::Ticket58,
    ];

    /// Identifier used in settings and on the command line.
    pub fn as_str(self) -> &'static str {
        match self {
            PaperFormat::A4 => "a4",
            PaperFormat::A5 => "a5",
            PaperFormat::Ticket80 => "ticket80",
            PaperFormat::Ticket58 => "ticket58",
        }
    }

    /// Paper width in millimetres.
    pub fn width_mm(self) -> u32 {
        match self {
            PaperFormat::A4 => 210,
            PaperFormat::A5 => 148,
            PaperFormat::Ticket80 => 80,
            PaperFormat::Ticket58 => 58,
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
    /// Accent color as `#rrggbb`, used by the page templates.
    pub accent_color: String,
    /// Extra lines printed under the issuer, such as phone, email or website.
    pub issuer_lines: Vec<String>,
    /// Extra text printed at the end of the document.
    pub footer: Option<String>,
    pub show_qr: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            format: PaperFormat::default(),
            accent_color: "#1f3a5f".to_owned(),
            issuer_lines: Vec::new(),
            footer: None,
            show_qr: true,
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
        let hex = self.accent_color.strip_prefix('#').unwrap_or_default();
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(RenderError::InvalidColor(self.accent_color.clone()));
        }
        Ok(())
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
    #[error("unknown format `{0}`: expected a4, a5, ticket80 or ticket58")]
    UnknownFormat(String),
    #[error("invalid settings: {0}")]
    Settings(String),
    #[error("invalid accent color `{0}`: expected #rrggbb")]
    InvalidColor(String),
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
}

/// Converts an input file, either an XML document or a ZIP archive holding
/// one or more of them, into PDFs.
pub fn convert(
    name: &str,
    bytes: &[u8],
    options: &RenderOptions,
) -> Result<Vec<Rendered>, ConvertError> {
    xml2pdf_core::xml_sources(name, bytes)?
        .into_iter()
        .map(|source| {
            let document =
                xml2pdf_core::parse_bytes(&source.bytes).map_err(|err| ConvertError::Parse {
                    name: source.name.clone(),
                    source: err,
                })?;
            let (pdf, page_mm) =
                render_sized(&document, options).map_err(|err| ConvertError::Render {
                    name: source.name.clone(),
                    source: err,
                })?;
            Ok(Rendered {
                source_name: source.name,
                file_stem: document.file_stem(),
                pdf,
                page_mm,
            })
        })
        .collect()
}

/// Source of the built-in template for a format, a starting point for custom templates.
pub fn builtin_template(format: PaperFormat) -> &'static str {
    if format.is_ticket() {
        TICKET_TEMPLATE
    } else {
        PAGE_TEMPLATE
    }
}

/// Renders a document to PDF bytes.
pub fn render(document: &Document, options: &RenderOptions) -> Result<Vec<u8>, RenderError> {
    render_sized(document, options).map(|(pdf, _)| pdf)
}

/// Renders a document and also returns the size of its first page in
/// millimetres, which printing needs for continuous ticket paper.
fn render_sized(
    document: &Document,
    options: &RenderOptions,
) -> Result<(Vec<u8>, (f64, f64)), RenderError> {
    options.settings.validate()?;

    let data = template_data(document, options);
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
    let pdf = typst_pdf::pdf(&paged, &PdfOptions::default()).map_err(|err| describe(&err))?;
    Ok((pdf, page_mm))
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
