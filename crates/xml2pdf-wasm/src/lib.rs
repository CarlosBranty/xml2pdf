//! WebAssembly bindings: the browser converts SUNAT XML documents to PDF
//! locally, so the files never leave the user's machine.

use wasm_bindgen::prelude::*;
use xml2pdf_render::{Logo, PaperFormat, RenderOptions, Settings};

/// A PDF generated from one XML document.
#[wasm_bindgen]
pub struct PdfFile {
    name: String,
    pdf: Vec<u8>,
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
    let rendered = xml2pdf_render::convert(name, bytes, &options)?;
    Ok(rendered
        .into_iter()
        .map(|file| PdfFile {
            name: format!("{}.pdf", file.file_stem),
            pdf: file.pdf,
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
