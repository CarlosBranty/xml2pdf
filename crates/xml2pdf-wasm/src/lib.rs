//! WebAssembly bindings: the browser converts SUNAT XML documents to PDF
//! locally, so the files never leave the user's machine.

use wasm_bindgen::prelude::*;
use xml2pdf_render::{Logo, PaperFormat, RenderOptions, Settings};

/// A PDF generated from one XML document.
#[wasm_bindgen]
pub struct PdfFile {
    name: String,
    pdf: Vec<u8>,
    preview: String,
}

#[wasm_bindgen]
impl PdfFile {
    /// Suggested file name, for example `20123456789-01-F001-00000123.pdf`.
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// PDF content.
    #[wasm_bindgen(getter)]
    pub fn pdf(&self) -> Vec<u8> {
        self.pdf.clone()
    }

    /// The document drawn as an SVG image, to show it without a PDF viewer.
    #[wasm_bindgen(getter)]
    pub fn preview(&self) -> String {
        self.preview.clone()
    }
}

/// Converts an XML document, or a ZIP archive containing several, to PDF.
///
/// `settings` is the JSON form of the render settings; `logo` is an optional
/// image and `template` an optional Typst source replacing the built-in one.
#[wasm_bindgen]
pub fn convert(
    name: &str,
    bytes: &[u8],
    settings: &str,
    logo: Option<Vec<u8>>,
    template: Option<String>,
) -> Result<Vec<PdfFile>, JsError> {
    let options = options(settings, logo, template)?;
    let rendered = xml2pdf_render::convert_with_preview(name, bytes, &options)?;
    Ok(rendered
        .into_iter()
        .map(|file| PdfFile {
            name: format!("{}.pdf", file.file_stem),
            pdf: file.pdf,
            preview: file.preview_svg.unwrap_or_default(),
        })
        .collect())
}

/// Returns the data a template receives for the first document of the input, as JSON.
#[wasm_bindgen(js_name = templateData)]
pub fn template_data(name: &str, bytes: &[u8], settings: &str) -> Result<String, JsError> {
    let options = options(settings, None, None)?;
    let source = xml2pdf_core::xml_sources(name, bytes)?.swap_remove(0);
    let document = xml2pdf_core::parse_bytes(&source.bytes)?;
    Ok(serde_json::to_string_pretty(
        &xml2pdf_render::template_data(&document, &options),
    )?)
}

/// Renders one XML document to PDF without drawing a preview, which is
/// faster when converting a whole batch.
#[wasm_bindgen(js_name = renderPdf)]
pub fn render_pdf(
    bytes: &[u8],
    settings: &str,
    logo: Option<Vec<u8>>,
    template: Option<String>,
) -> Result<Vec<u8>, JsError> {
    let options = options(settings, logo, template)?;
    let document = xml2pdf_core::parse_bytes(bytes)?;
    Ok(xml2pdf_render::render(&document, &options)?)
}

/// An XML document found in an input file.
#[wasm_bindgen]
pub struct XmlEntry {
    name: String,
    bytes: Vec<u8>,
}

#[wasm_bindgen]
impl XmlEntry {
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }
}

/// Splits an input file into its XML documents: a ZIP yields one entry per
/// XML inside it, any other file yields itself.
#[wasm_bindgen]
pub fn extract(name: &str, bytes: &[u8]) -> Result<Vec<XmlEntry>, JsError> {
    Ok(xml2pdf_core::xml_sources(name, bytes)?
        .into_iter()
        .map(|source| XmlEntry {
            name: source.name,
            bytes: source.bytes,
        })
        .collect())
}

/// Reads one XML document and returns its summary as JSON: `file_stem`,
/// `id`, `title`, `issue_date`, `customer` and `total`.
#[wasm_bindgen]
pub fn summary(bytes: &[u8]) -> Result<String, JsError> {
    let document = xml2pdf_core::parse_bytes(bytes)?;
    Ok(xml2pdf_render::document_summary(&document).to_string())
}

/// Collects files and packs them into a single ZIP archive.
#[wasm_bindgen]
#[derive(Default)]
pub struct ZipBundle {
    files: Vec<(String, Vec<u8>)>,
}

#[wasm_bindgen]
impl ZipBundle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, name: String, bytes: Vec<u8>) {
        self.files.push((name, bytes));
    }

    /// Returns the ZIP archive and releases the bundle.
    pub fn finish(self) -> Result<Vec<u8>, JsError> {
        Ok(xml2pdf_core::write_zip(&self.files)?)
    }
}

/// Returns the built-in Typst template of a format (`a4`, `a5`, `ticket80`, `ticket58`).
#[wasm_bindgen(js_name = builtinTemplate)]
pub fn builtin_template(format: &str) -> Result<String, JsError> {
    Ok(xml2pdf_render::builtin_template(format.parse::<PaperFormat>()?).to_owned())
}

/// Returns the default settings as JSON.
#[wasm_bindgen(js_name = defaultSettings)]
pub fn default_settings() -> String {
    serde_json::to_string(&Settings::default()).expect("settings serialize to JSON")
}

fn options(
    settings: &str,
    logo: Option<Vec<u8>>,
    template: Option<String>,
) -> Result<RenderOptions, JsError> {
    Ok(RenderOptions {
        settings: Settings::from_json(settings)?,
        logo: logo
            .filter(|bytes| !bytes.is_empty())
            .map(Logo::from_bytes)
            .transpose()?,
        template: template.filter(|source| !source.trim().is_empty()),
    })
}
