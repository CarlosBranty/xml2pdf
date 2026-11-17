//! Automation behavior: conversion jobs, printing through the printer port,
//! configuration files and the folder watcher.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use xml2pdf_auto::{
    AutoConfig, ConfigError, PrintError, PrintJob, Printer, WatchError, WatchEvent, is_document,
    media_for, process_file, watch,
};
use xml2pdf_render::{PaperFormat, RenderOptions, Settings};

/// What the fake printer was asked to print.
#[derive(Debug, Clone, PartialEq)]
struct Printed {
    pdf: PathBuf,
    printer: Option<String>,
    copies: u32,
    media: Option<String>,
}

/// Printer port double: records jobs, fails every job when `broken`, or
/// panics on its first job when `panics_once`.
#[derive(Default)]
struct FakePrinter {
    jobs: Mutex<Vec<Printed>>,
    broken: bool,
    panics_once: std::sync::atomic::AtomicBool,
}

impl Printer for FakePrinter {
    fn list(&self) -> Result<Vec<String>, PrintError> {
        Ok(vec!["Caja".to_owned()])
    }

    fn print(&self, job: &PrintJob) -> Result<(), PrintError> {
        if self
            .panics_once
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            panic!("the printer driver crashed");
        }
        if self.broken {
            return Err(PrintError::Failed {
                program: "lp".into(),
                message: "out of paper".into(),
            });
        }
        self.jobs.lock().unwrap().push(Printed {
            pdf: job.pdf.to_owned(),
            printer: job.printer.map(str::to_owned),
            copies: job.copies,
            media: job.media.clone(),
        });
        Ok(())
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn is_pdf(path: &Path) -> bool {
    std::fs::read(path).is_ok_and(|bytes| bytes.starts_with(b"%PDF-"))
}

fn config(output: &Path, format: PaperFormat) -> AutoConfig {
    AutoConfig {
        output_dir: Some(output.to_owned()),
        settings: Settings {
            format,
            ..Settings::default()
        },
        ..AutoConfig::default()
    }
}

#[test]
fn converts_without_printing_by_default() {
    let output = tempfile::tempdir().unwrap();
    let config = config(output.path(), PaperFormat::A4);
    let printer = FakePrinter::default();

    let processed = process_file(
        &fixture("factura.xml"),
        &config,
        &config.render_options().unwrap(),
        &printer,
    )
    .unwrap();

    assert_eq!(processed.len(), 1);
    assert_eq!(
        processed[0].pdf,
        output.path().join("20123456789-01-F001-00000123.pdf")
    );
    assert_eq!(processed[0].source, fixture("factura.xml"));
    assert!(!processed[0].printed);
    assert_eq!(processed[0].print_error, None);
    assert!(is_pdf(&processed[0].pdf));
    assert!(printer.jobs.lock().unwrap().is_empty());
}

#[test]
fn prints_each_pdf_with_the_configured_printer_copies_and_media() {
    let output = tempfile::tempdir().unwrap();
    let config = AutoConfig {
        print: true,
        printer: Some("Caja".to_owned()),
        copies: 2,
        ..config(output.path(), PaperFormat::A5)
    };
    let printer = FakePrinter::default();

    let processed = process_file(
        &fixture("documentos.zip"),
        &config,
        &config.render_options().unwrap(),
        &printer,
    )
    .unwrap();

    assert_eq!(processed.len(), 2, "one PDF per document of the archive");
    assert!(
        processed
            .iter()
            .all(|item| item.printed && item.print_error.is_none())
    );
    let jobs = printer.jobs.lock().unwrap();
    assert_eq!(
        *jobs,
        processed
            .iter()
            .map(|item| Printed {
                pdf: item.pdf.clone(),
                printer: Some("Caja".to_owned()),
                copies: 2,
                media: Some("A5".to_owned()),
            })
            .collect::<Vec<_>>()
    );
}

#[test]
fn prints_tickets_on_a_custom_media_matching_the_page() {
    let output = tempfile::tempdir().unwrap();
    let config = AutoConfig {
        print: true,
        ..config(output.path(), PaperFormat::Ticket80)
    };
    let printer = FakePrinter::default();

    process_file(
        &fixture("boleta.xml"),
        &config,
        &config.render_options().unwrap(),
        &printer,
    )
    .unwrap();

    let jobs = printer.jobs.lock().unwrap();
    assert_eq!(jobs[0].printer, None, "the default printer is used");
    let media = jobs[0].media.as_deref().unwrap();
    let height: u32 = media
        .strip_prefix("Custom.80x")
        .and_then(|rest| rest.strip_suffix("mm"))
        .and_then(|height| height.parse().ok())
        .unwrap_or_else(|| panic!("unexpected media `{media}`"));
    assert!(height > 60, "ticket height was {height} mm");
}

#[test]
fn keeps_the_pdf_when_printing_fails() {
    let output = tempfile::tempdir().unwrap();
    let config = AutoConfig {
        print: true,
        ..config(output.path(), PaperFormat::A4)
    };
    let printer = FakePrinter {
        broken: true,
        ..FakePrinter::default()
    };

    let processed = process_file(
        &fixture("factura.xml"),
        &config,
        &config.render_options().unwrap(),
        &printer,
    )
    .unwrap();

    assert!(!processed[0].printed);
    assert_eq!(
        processed[0].print_error.as_deref(),
        Some("`lp` failed: out of paper")
    );
    assert!(is_pdf(&processed[0].pdf));
}

#[test]
fn reports_files_that_cannot_be_converted() {
    let folder = tempfile::tempdir().unwrap();
    let broken = folder.path().join("roto.xml");
    std::fs::write(&broken, "not xml").unwrap();
    let config = config(&folder.path().join("pdf"), PaperFormat::A4);
    let options = RenderOptions::default();
    let printer = FakePrinter::default();

    let invalid = process_file(&broken, &config, &options, &printer)
        .unwrap_err()
        .to_string();
    assert!(
        invalid.starts_with("roto.xml: the file is not valid XML"),
        "{invalid}"
    );
    let missing =
        process_file(&folder.path().join("nope.xml"), &config, &options, &printer).unwrap_err();
    assert!(missing.to_string().starts_with("cannot read"), "{missing}");
}

#[test]
fn chooses_the_media_for_each_format() {
    assert_eq!(
        media_for(PaperFormat::A4, (210.0, 297.0)).as_deref(),
        Some("A4")
    );
    assert_eq!(
        media_for(PaperFormat::A5, (148.0, 210.0)).as_deref(),
        Some("A5")
    );
    assert_eq!(
        media_for(PaperFormat::Ticket80, (80.0, 163.2)).as_deref(),
        Some("Custom.80x164mm")
    );
    assert_eq!(
        media_for(PaperFormat::Ticket58, (58.0, 120.0)).as_deref(),
        Some("Custom.58x120mm")
    );
    assert_eq!(media_for(PaperFormat::Ticket58, (0.0, 0.0)), None);
}

#[test]
fn recognizes_the_files_it_handles() {
    assert!(is_document(Path::new("F001-1.xml")));
    assert!(is_document(Path::new("descarga.ZIP")));
    assert!(!is_document(Path::new("F001-1.pdf")));
    assert!(!is_document(Path::new("xml")));
}

#[test]
fn stores_the_configuration_as_json() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("nested/config.json");
    assert_eq!(
        AutoConfig::load(&path).unwrap(),
        AutoConfig::default(),
        "a missing file gives the defaults"
    );

    let config = AutoConfig {
        watch_dir: Some(PathBuf::from("/descargas")),
        print: true,
        printer: Some("Caja".to_owned()),
        copies: 3,
        watch_on_start: true,
        settings: Settings {
            format: PaperFormat::Ticket58,
            ..Settings::default()
        },
        ..AutoConfig::default()
    };
    config.save(&path).unwrap();
    assert_eq!(AutoConfig::load(&path).unwrap(), config);
}

#[test]
fn rejects_invalid_configuration() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("config.json");

    let zero_copies = AutoConfig {
        copies: 0,
        ..AutoConfig::default()
    };
    assert!(matches!(zero_copies.save(&path), Err(ConfigError::Copies)));
    assert!(!path.exists(), "an invalid configuration is not written");

    std::fs::write(&path, r#"{"impresora":"Caja"}"#).unwrap();
    assert!(matches!(
        AutoConfig::load(&path),
        Err(ConfigError::Invalid { .. })
    ));
    std::fs::write(&path, r#"{"copies":50}"#).unwrap();
    assert!(matches!(AutoConfig::load(&path), Err(ConfigError::Copies)));
}

#[test]
fn chooses_the_output_folder() {
    let input = Path::new("/descargas/F001-1.xml");
    let watched = AutoConfig {
        watch_dir: Some("/vigilada".into()),
        ..AutoConfig::default()
    };
    let explicit = AutoConfig {
        output_dir: Some("/salida".into()),
        ..watched.clone()
    };

    assert_eq!(
        AutoConfig::default().output_dir_for(input),
        Path::new("/descargas/pdf")
    );
    assert_eq!(watched.output_dir_for(input), Path::new("/vigilada/pdf"));
    assert_eq!(explicit.output_dir_for(input), Path::new("/salida"));
}

/// Starts a watcher over `folder` and returns it with the stream of its events.
fn start_watching(
    folder: &Path,
    printer: Arc<FakePrinter>,
    print: bool,
) -> (xml2pdf_auto::WatchHandle, Receiver<WatchEvent>) {
    let config = AutoConfig {
        watch_dir: Some(folder.to_owned()),
        print,
        ..AutoConfig::default()
    };
    let (sender, receiver) = mpsc::channel();
    let handle = watch(config, RenderOptions::default(), printer, move |event| {
        let _ = sender.send(event);
    })
    .unwrap();
    (handle, receiver)
}

const EVENT_TIMEOUT: Duration = Duration::from_secs(20);

#[test]
fn watches_a_folder_and_prints_new_documents() {
    let folder = tempfile::tempdir().unwrap();
    std::fs::copy(fixture("boleta.xml"), folder.path().join("anterior.xml")).unwrap();
    let printer = Arc::new(FakePrinter::default());
    let (handle, events) = start_watching(folder.path(), Arc::clone(&printer), true);

    std::fs::copy(fixture("factura.xml"), folder.path().join("nueva.xml")).unwrap();
    std::fs::write(folder.path().join("notas.txt"), "ignored").unwrap();

    let expected_pdf = folder.path().join("pdf/20123456789-01-F001-00000123.pdf");
    match events.recv_timeout(EVENT_TIMEOUT).unwrap() {
        WatchEvent::Processed(item) => {
            assert_eq!(item.source, folder.path().join("nueva.xml"));
            assert_eq!(item.pdf, expected_pdf);
            assert!(item.printed);
        }
        other => panic!("unexpected event {other:?}"),
    }
    assert!(is_pdf(&expected_pdf));

    // Neither the file that was already there nor the non-document produce events.
    assert!(events.recv_timeout(Duration::from_secs(2)).is_err());
    handle.stop();
    let jobs = printer.jobs.lock().unwrap();
    assert_eq!(jobs.len(), 1, "the new document is printed exactly once");
    assert_eq!(jobs[0].pdf, expected_pdf);
}

#[test]
fn reports_watched_files_that_fail() {
    let folder = tempfile::tempdir().unwrap();
    let (handle, events) = start_watching(folder.path(), Arc::new(FakePrinter::default()), false);

    std::fs::write(folder.path().join("roto.xml"), "not xml").unwrap();

    match events.recv_timeout(EVENT_TIMEOUT).unwrap() {
        WatchEvent::Failed { source, message } => {
            assert_eq!(source, folder.path().join("roto.xml"));
            assert!(message.contains("not valid XML"), "{message}");
        }
        other => panic!("unexpected event {other:?}"),
    }
    handle.stop();
}

#[test]
fn refuses_to_watch_without_a_valid_folder() {
    let start = |config| {
        watch(
            config,
            RenderOptions::default(),
            Arc::new(FakePrinter::default()),
            |_| {},
        )
    };

    assert!(matches!(
        start(AutoConfig::default()),
        Err(WatchError::NoFolder)
    ));
    let missing = AutoConfig {
        watch_dir: Some("/does/not/exist".into()),
        ..AutoConfig::default()
    };
    assert!(matches!(start(missing), Err(WatchError::MissingFolder(_))));
}

#[test]
fn keeps_watching_after_a_document_crashes_the_conversion() {
    let folder = tempfile::tempdir().unwrap();
    let printer = Arc::new(FakePrinter {
        panics_once: std::sync::atomic::AtomicBool::new(true),
        ..FakePrinter::default()
    });
    let (handle, events) = start_watching(folder.path(), Arc::clone(&printer), true);

    std::fs::copy(fixture("boleta.xml"), folder.path().join("primero.xml")).unwrap();
    match events.recv_timeout(EVENT_TIMEOUT).unwrap() {
        WatchEvent::Failed { source, message } => {
            assert_eq!(source, folder.path().join("primero.xml"));
            assert!(message.contains("the printer driver crashed"), "{message}");
        }
        other => panic!("unexpected event {other:?}"),
    }

    std::fs::copy(fixture("factura.xml"), folder.path().join("segundo.xml")).unwrap();
    match events.recv_timeout(EVENT_TIMEOUT).unwrap() {
        WatchEvent::Processed(item) => assert!(item.printed, "the next document is still handled"),
        other => panic!("unexpected event {other:?}"),
    }
    handle.stop();
}
