//! Automation for xml2pdf: watch a folder, convert every new SUNAT XML to
//! PDF and send it to a printer.
//!
//! The crate has no user interface. The command line (`xml2pdf watch`) and
//! the desktop app are thin layers over it.

mod config;
mod job;
mod printer;
mod watcher;

pub use config::{AutoConfig, ConfigError};
pub use job::{JobError, Processed, is_document, process_file};
pub use printer::{PrintError, PrintJob, Printer, SystemPrinter, media_for};
pub use watcher::{WatchError, WatchEvent, WatchHandle, watch};
