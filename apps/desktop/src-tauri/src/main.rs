//! Desktop app: watches a folder and converts and prints every new SUNAT XML.
//!
//! All the behavior lives in `xml2pdf-auto`; this file wires it to the window.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use xml2pdf_auto::{
    AutoConfig, Printer, SystemPrinter, WatchEvent, WatchHandle, process_file, watch,
};

/// Event sent to the window for every processed file.
const WATCH_EVENT: &str = "watch-event";

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

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let folder = app.path().app_config_dir().map_err(|err| err.to_string())?;
    Ok(folder.join("config.json"))
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

/// Saves the configuration. A running watcher restarts with the new values.
#[tauri::command]
fn save_config(app: AppHandle, desktop: State<Desktop>, config: AutoConfig) -> Result<(), String> {
    config
        .save(&config_path(&app)?)
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
    start(&app, &desktop)
}

#[tauri::command]
fn stop_watching(desktop: State<Desktop>) {
    lock(&desktop.watcher).take();
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<PathBuf> {
    app.dialog()
        .file()
        .blocking_pick_folder()
        .and_then(|folder| folder.into_path().ok())
}

/// Opens a file dialog for a `logo` image or a Typst `template`.
#[tauri::command]
async fn pick_file(app: AppHandle, kind: String) -> Option<PathBuf> {
    let dialog = match kind.as_str() {
        "logo" => app
            .dialog()
            .file()
            .add_filter("Imagen", &["png", "jpg", "jpeg", "gif", "webp", "svg"]),
        _ => app.dialog().file().add_filter("Plantilla Typst", &["typ"]),
    };
    dialog
        .blocking_pick_file()
        .and_then(|file| file.into_path().ok())
}

/// Lets the user choose XML or ZIP files and processes them right away.
#[tauri::command]
async fn convert_files(
    app: AppHandle,
    desktop: State<'_, Desktop>,
) -> Result<Vec<WatchEvent>, String> {
    let Some(files) = app
        .dialog()
        .file()
        .add_filter("Comprobantes SUNAT", &["xml", "zip"])
        .blocking_pick_files()
    else {
        return Ok(Vec::new());
    };

    let config = lock(&desktop.config).clone();
    let options = config.render_options().map_err(|err| err.to_string())?;
    let mut events = Vec::new();
    for path in files.into_iter().filter_map(|file| file.into_path().ok()) {
        match process_file(&path, &config, &options, desktop.printer.as_ref()) {
            Ok(processed) => events.extend(processed.into_iter().map(WatchEvent::Processed)),
            Err(err) => events.push(WatchEvent::Failed {
                source: path,
                message: err.to_string(),
            }),
        }
    }
    Ok(events)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            // An unreadable configuration starts from the defaults; the
            // window reports the problem when the user saves again.
            let config = config_path(&handle)
                .ok()
                .and_then(|path| AutoConfig::load(&path).ok())
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
            save_config,
            start_watching,
            stop_watching,
            pick_folder,
            pick_file,
            convert_files,
        ])
        .run(tauri::generate_context!())
        .expect("the desktop app failed to start");
}
