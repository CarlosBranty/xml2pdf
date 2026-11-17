//! Desktop app. Its window shows the same interface as the web app, built on
//! WebAssembly; this process adds what a browser cannot do: saving through
//! the system dialog, printing without a dialog, and watching a folder to
//! convert and print every new document.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use xml2pdf_auto::{AutoConfig, PrintJob, Printer, SystemPrinter, WatchHandle, media_for, watch};
use xml2pdf_render::{Design, Logo, PaperFormat, Settings};

/// Event sent to the window for every file the watcher processes.
const WATCH_EVENT: &str = "watch-event";
/// Files written next to the configuration for the watcher to read.
const LOGO_FILE: &str = "logo";
const TEMPLATE_FILE: &str = "template.typ";

struct Desktop {
    config: Mutex<AutoConfig>,
    watcher: Mutex<Option<WatchHandle>>,
    printer: Arc<dyn Printer>,
}

/// State shown by the window.
#[derive(Serialize)]
struct Snapshot {
    config: AutoConfig,
    watching: bool,
    printers: Vec<String>,
    /// Why the printers could not be listed, when that failed.
    printer_error: Option<String>,
}

/// Folder watching and printing choices made in the window.
#[derive(Debug, Deserialize)]
struct WatchChoices {
    watch_dir: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    print: bool,
    printer: Option<String>,
    copies: u32,
    watch_on_start: bool,
    format: PaperFormat,
}

/// Document options edited in the window, which the watcher must also apply.
#[derive(Debug, Deserialize)]
struct RenderChoices {
    issuer_lines: Vec<String>,
    footer: Option<String>,
    show_qr: bool,
    /// Logo image, or `None` for no logo.
    logo: Option<Vec<u8>>,
    /// Custom Typst template for the watched format, or `None` for the built-in one.
    template: Option<String>,
    /// Designs made in the format designer, or `None` for the classic ones.
    #[serde(default)]
    page_design: Option<Design>,
    #[serde(default)]
    ticket_design: Option<Design>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Logs a command when `XML2PDF_LOG_COMMANDS` is set, to diagnose the window.
fn trace(command: &str) {
    if std::env::var_os("XML2PDF_LOG_COMMANDS").is_some() {
        eprintln!("command: {command}");
    }
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_config_dir().map_err(|err| err.to_string())
}

/// Builds the configuration from the window's choices. The logo and the
/// template are written into `folder`, because the watcher reads them from disk.
fn build_config(
    folder: &Path,
    watch: WatchChoices,
    render: RenderChoices,
) -> Result<AutoConfig, String> {
    std::fs::create_dir_all(folder)
        .map_err(|err| format!("cannot create `{}`: {err}", folder.display()))?;
    let store = |name: &str, content: Option<&[u8]>| -> Result<Option<PathBuf>, String> {
        let path = folder.join(name);
        match content {
            Some(bytes) => std::fs::write(&path, bytes)
                .map(|()| Some(path.clone()))
                .map_err(|err| format!("cannot write `{}`: {err}", path.display())),
            None => {
                // A leftover file would be picked up again by nothing, but removing it keeps the folder tidy.
                let _ = std::fs::remove_file(&path);
                Ok(None)
            }
        }
    };

    if let Some(bytes) = &render.logo {
        Logo::from_bytes(bytes.clone()).map_err(|err| err.to_string())?;
    }
    let config = AutoConfig {
        watch_dir: watch.watch_dir,
        output_dir: watch.output_dir,
        print: watch.print,
        printer: watch.printer.filter(|name| !name.is_empty()),
        copies: watch.copies,
        watch_on_start: watch.watch_on_start,
        settings: Settings {
            format: watch.format,
            issuer_lines: render.issuer_lines,
            footer: render.footer,
            show_qr: render.show_qr,
            page_design: render.page_design,
            ticket_design: render.ticket_design,
        },
        logo: store(LOGO_FILE, render.logo.as_deref())?,
        template: store(TEMPLATE_FILE, render.template.as_deref().map(str::as_bytes))?,
    };
    config.validate().map_err(|err| err.to_string())?;
    Ok(config)
}

/// File name made only of characters that are safe in any folder.
fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('.');
    if trimmed.is_empty() {
        "documento.pdf".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Writes a PDF into `folder` and sends it to the configured printer.
/// Returns the name of the printer used.
fn print_document(
    folder: &Path,
    name: &str,
    bytes: &[u8],
    format: PaperFormat,
    page_mm: (f64, f64),
    config: &AutoConfig,
    printer: &dyn Printer,
) -> Result<String, String> {
    std::fs::create_dir_all(folder)
        .map_err(|err| format!("cannot create `{}`: {err}", folder.display()))?;
    let path = folder.join(safe_file_name(name));
    std::fs::write(&path, bytes)
        .map_err(|err| format!("cannot write `{}`: {err}", path.display()))?;

    printer
        .print(&PrintJob {
            pdf: &path,
            printer: config.printer.as_deref(),
            copies: config.copies,
            media: media_for(format, page_mm),
        })
        .map_err(|err| err.to_string())?;
    Ok(config
        .printer
        .clone()
        .unwrap_or_else(|| "predeterminada".to_owned()))
}

fn start(app: &AppHandle, desktop: &Desktop) -> Result<(), String> {
    let config = lock(&desktop.config).clone();
    let options = config.render_options().map_err(|err| err.to_string())?;
    let window = app.clone();
    let handle = watch(
        config,
        options,
        Arc::clone(&desktop.printer),
        move |event| {
            let _ = window.emit(WATCH_EVENT, &event);
        },
    )
    .map_err(|err| err.to_string())?;
    *lock(&desktop.watcher) = Some(handle);
    Ok(())
}

#[tauri::command]
fn snapshot(desktop: State<Desktop>) -> Snapshot {
    trace("snapshot");
    let (printers, printer_error) = match desktop.printer.list() {
        Ok(printers) => (printers, None),
        Err(err) => (Vec::new(), Some(err.to_string())),
    };
    Snapshot {
        config: lock(&desktop.config).clone(),
        watching: lock(&desktop.watcher).is_some(),
        printers,
        printer_error,
    }
}

/// Saves the choices made in the window. A running watcher restarts with them.
#[tauri::command]
fn apply_settings(
    app: AppHandle,
    desktop: State<Desktop>,
    watch: WatchChoices,
    render: RenderChoices,
) -> Result<(), String> {
    trace("apply_settings");
    let folder = config_dir(&app)?;
    let config = build_config(&folder, watch, render)?;
    config
        .save(&folder.join("config.json"))
        .map_err(|err| err.to_string())?;
    *lock(&desktop.config) = config;

    let was_watching = lock(&desktop.watcher).take().is_some();
    if was_watching {
        start(&app, &desktop)?;
    }
    Ok(())
}

#[tauri::command]
fn start_watching(app: AppHandle, desktop: State<Desktop>) -> Result<(), String> {
    trace("start_watching");
    start(&app, &desktop)
}

#[tauri::command]
fn stop_watching(desktop: State<Desktop>) {
    trace("stop_watching");
    lock(&desktop.watcher).take();
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<PathBuf> {
    trace("pick_folder");
    app.dialog()
        .file()
        .blocking_pick_folder()
        .and_then(|folder| folder.into_path().ok())
}

/// Asks where to save a file and writes it there. Returns the chosen path,
/// or `None` when the dialog was cancelled.
#[tauri::command]
async fn save_file(
    app: AppHandle,
    name: String,
    bytes: Vec<u8>,
    description: String,
    extension: String,
) -> Result<Option<PathBuf>, String> {
    trace("save_file");
    let chosen = app
        .dialog()
        .file()
        .set_file_name(safe_file_name(&name))
        .add_filter(description, &[extension.as_str()])
        .blocking_save_file();
    let Some(path) = chosen.and_then(|file| file.into_path().ok()) else {
        return Ok(None);
    };
    std::fs::write(&path, bytes)
        .map_err(|err| format!("cannot write `{}`: {err}", path.display()))?;
    Ok(Some(path))
}

/// Prints a PDF produced in the window on the configured printer.
#[tauri::command]
fn print_pdf(
    app: AppHandle,
    desktop: State<Desktop>,
    name: String,
    bytes: Vec<u8>,
    format: PaperFormat,
    page_mm: (f64, f64),
) -> Result<String, String> {
    trace("print_pdf");
    let folder = app
        .path()
        .app_cache_dir()
        .map_err(|err| err.to_string())?
        .join("print");
    let config = lock(&desktop.config).clone();
    print_document(
        &folder,
        &name,
        &bytes,
        format,
        page_mm,
        &config,
        desktop.printer.as_ref(),
    )
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            // An unreadable configuration starts from the defaults; the
            // window writes a fresh one as soon as it loads.
            let config = config_dir(&handle)
                .ok()
                .and_then(|folder| AutoConfig::load(&folder.join("config.json")).ok())
                .unwrap_or_default();
            let watch_on_start = config.watch_on_start && config.watch_dir.is_some();

            let desktop = Desktop {
                config: Mutex::new(config),
                watcher: Mutex::new(None),
                printer: Arc::new(SystemPrinter),
            };
            if watch_on_start {
                // A failure here is visible in the window: it shows the watcher as stopped.
                let _ = start(&handle, &desktop);
            }
            app.manage(desktop);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            apply_settings,
            start_watching,
            stop_watching,
            pick_folder,
            save_file,
            print_pdf,
        ])
        .run(tauri::generate_context!())
        .expect("the desktop app failed to start");
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use xml2pdf_auto::PrintError;

    use super::*;

    const SVG_LOGO: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="2"/>"#;

    fn watch_choices() -> WatchChoices {
        WatchChoices {
            watch_dir: Some("/descargas".into()),
            output_dir: None,
            print: true,
            printer: Some("Caja".to_owned()),
            copies: 2,
            watch_on_start: true,
            format: PaperFormat::Ticket80,
        }
    }

    fn render_choices(logo: Option<&[u8]>, template: Option<&str>) -> RenderChoices {
        RenderChoices {
            issuer_lines: vec!["Tel. 01 555 0100".to_owned()],
            footer: Some("Gracias".to_owned()),
            show_qr: false,
            logo: logo.map(<[u8]>::to_vec),
            template: template.map(str::to_owned),
            page_design: None,
            ticket_design: None,
        }
    }

    /// What the fake printer was asked to print.
    #[derive(Debug, PartialEq)]
    struct Printed {
        pdf: PathBuf,
        printer: Option<String>,
        copies: u32,
        media: Option<String>,
    }

    /// Printer double that records the jobs it receives.
    #[derive(Default)]
    struct FakePrinter {
        jobs: Mutex<Vec<Printed>>,
        broken: bool,
    }

    impl Printer for FakePrinter {
        fn list(&self) -> Result<Vec<String>, PrintError> {
            Ok(Vec::new())
        }

        fn print(&self, job: &PrintJob) -> Result<(), PrintError> {
            if self.broken {
                return Err(PrintError::Failed {
                    program: "lp".into(),
                    message: "sin papel".into(),
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

    #[test]
    fn builds_the_configuration_from_the_window_choices() {
        let folder = tempfile::tempdir().unwrap();

        let config = build_config(
            folder.path(),
            watch_choices(),
            render_choices(Some(SVG_LOGO), Some("#json(\"data.json\").doc.id")),
        )
        .unwrap();

        assert_eq!(config.watch_dir.as_deref(), Some(Path::new("/descargas")));
        assert_eq!(
            (config.print, config.copies, config.watch_on_start),
            (true, 2, true)
        );
        assert_eq!(config.printer.as_deref(), Some("Caja"));
        assert_eq!(config.settings.format, PaperFormat::Ticket80);
        assert_eq!(config.settings.issuer_lines, ["Tel. 01 555 0100"]);
        assert_eq!(config.settings.footer.as_deref(), Some("Gracias"));
        assert!(!config.settings.show_qr);
        assert_eq!(
            std::fs::read(config.logo.as_ref().unwrap()).unwrap(),
            SVG_LOGO
        );
        assert_eq!(
            std::fs::read_to_string(config.template.as_ref().unwrap()).unwrap(),
            "#json(\"data.json\").doc.id"
        );
        // The watcher can load what was stored.
        assert!(config.render_options().unwrap().logo.is_some());
    }

    #[test]
    fn removes_the_logo_and_template_when_they_are_cleared() {
        let folder = tempfile::tempdir().unwrap();
        build_config(
            folder.path(),
            watch_choices(),
            render_choices(Some(SVG_LOGO), Some("x")),
        )
        .unwrap();

        let config =
            build_config(folder.path(), watch_choices(), render_choices(None, None)).unwrap();

        assert_eq!((config.logo, config.template), (None, None));
        assert!(!folder.path().join(LOGO_FILE).exists());
        assert!(!folder.path().join(TEMPLATE_FILE).exists());
    }

    #[test]
    fn rejects_invalid_choices() {
        let folder = tempfile::tempdir().unwrap();

        let bad_logo = build_config(
            folder.path(),
            watch_choices(),
            render_choices(Some(b"texto"), None),
        );
        assert!(bad_logo.unwrap_err().contains("unsupported logo"));
        assert!(
            !folder.path().join(LOGO_FILE).exists(),
            "an invalid logo is not stored"
        );

        let no_copies = WatchChoices {
            copies: 0,
            ..watch_choices()
        };
        let error = build_config(folder.path(), no_copies, render_choices(None, None));
        assert!(error.unwrap_err().contains("copies"));

        let unnamed = WatchChoices {
            printer: Some(String::new()),
            ..watch_choices()
        };
        let config = build_config(folder.path(), unnamed, render_choices(None, None)).unwrap();
        assert_eq!(
            config.printer, None,
            "an empty name means the default printer"
        );
    }

    #[test]
    fn prints_a_pdf_with_the_configured_printer() {
        let folder = tempfile::tempdir().unwrap();
        let config =
            build_config(folder.path(), watch_choices(), render_choices(None, None)).unwrap();
        let printer = FakePrinter::default();
        let spool = folder.path().join("print");

        let used = print_document(
            &spool,
            "20123456789-03-B001-1-ticket80.pdf",
            b"%PDF-ticket",
            PaperFormat::Ticket80,
            (80.0, 163.2),
            &config,
            &printer,
        )
        .unwrap();

        assert_eq!(used, "Caja");
        let pdf = spool.join("20123456789-03-B001-1-ticket80.pdf");
        assert_eq!(std::fs::read(&pdf).unwrap(), b"%PDF-ticket");
        assert_eq!(
            *printer.jobs.lock().unwrap(),
            [Printed {
                pdf,
                printer: Some("Caja".to_owned()),
                copies: 2,
                media: Some("Custom.80x164mm".to_owned()),
            }]
        );
    }

    #[test]
    fn reports_printing_failures() {
        let folder = tempfile::tempdir().unwrap();
        let printer = FakePrinter {
            broken: true,
            ..FakePrinter::default()
        };

        let error = print_document(
            folder.path(),
            "a.pdf",
            b"%PDF-",
            PaperFormat::A4,
            (210.0, 297.0),
            &AutoConfig::default(),
            &printer,
        );

        assert_eq!(error.unwrap_err(), "`lp` failed: sin papel");
    }

    #[test]
    fn keeps_file_names_inside_their_folder() {
        assert_eq!(
            safe_file_name("20123456789-01-F001-123-a4.pdf"),
            "20123456789-01-F001-123-a4.pdf"
        );
        assert_eq!(safe_file_name("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(safe_file_name("factura nº 1.pdf"), "factura_n__1.pdf");
        assert_eq!(safe_file_name(".."), "documento.pdf");
        assert_eq!(safe_file_name(""), "documento.pdf");
    }
}
