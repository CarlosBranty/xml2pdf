//! Input handling: SUNAT portals often deliver the XML inside a ZIP file.

use std::collections::HashSet;
use std::io::{Cursor, Read, Write};

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("the ZIP file cannot be read: {0}")]
    Zip(String),
    #[error("the ZIP file does not contain any XML document")]
    Empty,
    #[error("`{0}` inside the ZIP file is larger than 20 MB, too large for an electronic document")]
    TooLarge(String),
    #[error("the ZIP file holds more than 1000 documents; split it into smaller files")]
    TooMany,
}

/// Largest XML accepted from an archive. Real documents are a few hundred
/// kilobytes; the cap stops a small ZIP from expanding into gigabytes.
const MAX_XML_BYTES: u64 = 20 * 1024 * 1024;
/// Most documents accepted from one archive.
const MAX_DOCUMENTS: usize = 1000;

/// An XML document found in the input, with the name it had there.
#[derive(Debug, Clone)]
pub struct XmlSource {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Returns the XML documents contained in `bytes`.
///
/// A ZIP archive yields every `.xml` entry except SUNAT responses (CDR files,
/// named `R-...`); any other input is returned as a single source.
pub fn xml_sources(name: &str, bytes: &[u8]) -> Result<Vec<XmlSource>, ArchiveError> {
    if !bytes.starts_with(b"PK\x03\x04") {
        return Ok(vec![XmlSource {
            name: name.to_owned(),
            bytes: bytes.to_vec(),
        }]);
    }

    let zip_error = |err: zip::result::ZipError| ArchiveError::Zip(err.to_string());
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(zip_error)?;
    let mut sources = Vec::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(zip_error)?;
        let full_name = entry.name().map_err(zip_error)?.into_owned();
        let entry_name = full_name.rsplit('/').next().unwrap_or_default().to_owned();
        let lowered = entry_name.to_ascii_lowercase();
        if !entry.is_file() || !lowered.ends_with(".xml") || lowered.starts_with("r-") {
            continue;
        }
        if sources.len() == MAX_DOCUMENTS {
            return Err(ArchiveError::TooMany);
        }
        // The size declared in the archive cannot be trusted, so the read itself is capped.
        let mut content = Vec::new();
        entry
            .by_ref()
            .take(MAX_XML_BYTES + 1)
            .read_to_end(&mut content)
            .map_err(|err| ArchiveError::Zip(err.to_string()))?;
        if content.len() as u64 > MAX_XML_BYTES {
            return Err(ArchiveError::TooLarge(entry_name));
        }
        sources.push(XmlSource {
            name: entry_name,
            bytes: content,
        });
    }

    if sources.is_empty() {
        return Err(ArchiveError::Empty);
    }
    Ok(sources)
}

/// Packs files into a ZIP archive, in the given order.
///
/// A repeated name gets a numeric suffix (`a (2).pdf`), so no file replaces
/// another. Files are stored without compression: PDFs are already compressed.
pub fn write_zip(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, ArchiveError> {
    let zip_error = |err: zip::result::ZipError| ArchiveError::Zip(err.to_string());
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut used = HashSet::new();

    for (name, bytes) in files {
        writer
            .start_file(unique_name(name, &mut used), options)
            .map_err(zip_error)?;
        writer
            .write_all(bytes)
            .map_err(|err| ArchiveError::Zip(err.to_string()))?;
    }
    Ok(writer.finish().map_err(zip_error)?.into_inner())
}

/// Returns `name`, or `name (N).ext` when the name was already used.
fn unique_name(name: &str, used: &mut HashSet<String>) -> String {
    if used.insert(name.to_owned()) {
        return name.to_owned();
    }
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, extension)) => (stem, format!(".{extension}")),
        None => (name, String::new()),
    };
    (2..)
        .map(|count| format!("{stem} ({count}){extension}"))
        .find(|candidate| used.insert(candidate.clone()))
        .expect("an unused name always exists")
}
