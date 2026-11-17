//! Converts one input file and optionally prints the result.

use std::path::{Path, PathBuf};

use serde::Serialize;
use xml2pdf_render::{ConvertError, RenderOptions};

use crate::config::AutoConfig;
use crate::printer::{PrintJob, Printer, media_for};

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("cannot read `{}`: {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot write `{}`: {source}", path.display())]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Convert(#[from] ConvertError),
}

/// Outcome of one document: the PDF is always written; printing may fail
/// without discarding it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Processed {
    pub source: PathBuf,
    pub pdf: PathBuf,
    pub printed: bool,
    pub print_error: Option<String>,
}

/// True for the files the automation handles: `.xml` and `.zip`.
pub fn is_document(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("xml") || ext.eq_ignore_ascii_case("zip"))
}

/// Converts `input` into the configured output folder and prints each PDF
/// when printing is enabled.
pub fn process_file(
    input: &Path,
    config: &AutoConfig,
    options: &RenderOptions,
    printer: &dyn Printer,
) -> Result<Vec<Processed>, JobError> {
    let bytes = std::fs::read(input).map_err(|source| JobError::Read {
        path: input.to_owned(),
        source,
    })?;
    let name = input.file_name().unwrap_or_default().to_string_lossy();
    let rendered = xml2pdf_render::convert(&name, &bytes, options)?;

    let output_dir = config.output_dir_for(input);
    std::fs::create_dir_all(&output_dir).map_err(|source| JobError::Write {
        path: output_dir.clone(),
        source,
    })?;

    let mut processed = Vec::new();
    for document in rendered {
        let pdf = output_dir.join(format!("{}.pdf", document.file_stem));
        std::fs::write(&pdf, &document.pdf).map_err(|source| JobError::Write {
            path: pdf.clone(),
            source,
        })?;

        let print_result = config.print.then(|| {
            printer.print(&PrintJob {
                pdf: &pdf,
                printer: config.printer.as_deref(),
                copies: config.copies,
                media: media_for(config.settings.format, document.page_mm),
            })
        });
        processed.push(Processed {
            source: input.to_owned(),
            printed: matches!(print_result, Some(Ok(()))),
            print_error: print_result
                .and_then(Result::err)
                .map(|err| err.to_string()),
            pdf,
        });
    }
    Ok(processed)
}
