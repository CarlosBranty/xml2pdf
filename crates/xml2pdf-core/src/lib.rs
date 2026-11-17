//! Core of xml2pdf: reads SUNAT electronic documents (UBL 2.1 XML) into a
//! typed model. It performs no I/O, so it runs unchanged on native targets
//! and on WebAssembly.

pub mod archive;
pub mod catalog;
pub mod model;
pub mod parser;
pub mod words;

pub use archive::{ArchiveError, XmlSource, write_zip, xml_sources};
pub use model::*;
pub use parser::{ParseError, parse_bytes, parse_str};
pub use rust_decimal::Decimal;
