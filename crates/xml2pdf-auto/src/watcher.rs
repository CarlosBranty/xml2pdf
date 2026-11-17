//! Watches a folder and processes every XML or ZIP file that appears in it.

use std::any::Any;
use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use notify::{EventKind, RecursiveMode, Watcher};
use serde::Serialize;
use xml2pdf_render::RenderOptions;

use crate::config::{AutoConfig, ConfigError};
use crate::job::{Processed, is_document, process_file};
use crate::printer::Printer;

/// How often pending files and the stop flag are checked.
const TICK: Duration = Duration::from_millis(200);
/// A file is processed once its size has not changed for this long, so a
/// download still being written is not read half-way.
const SETTLE: Duration = Duration::from_millis(700);

#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("choose a folder to watch")]
    NoFolder,
    #[error("the folder `{}` does not exist", .0.display())]
    MissingFolder(PathBuf),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("cannot watch the folder: {0}")]
    Notify(#[from] notify::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WatchEvent {
    Processed(Processed),
    Failed { source: PathBuf, message: String },
}

/// Keeps the watcher running. Dropping the handle stops it.
pub struct WatchHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl WatchHandle {
    /// Stops watching and waits for the file being processed to finish.
    pub fn stop(self) {}
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Starts watching the configured folder, rendering with `options`. Files
/// already in the folder are left alone; only files created or changed
/// afterwards are processed.
pub fn watch(
    config: AutoConfig,
    options: RenderOptions,
    printer: Arc<dyn Printer>,
    on_event: impl Fn(WatchEvent) + Send + 'static,
) -> Result<WatchHandle, WatchError> {
    let folder = config.watch_dir.clone().ok_or(WatchError::NoFolder)?;
    if !folder.is_dir() {
        return Err(WatchError::MissingFolder(folder));
    }
    config.validate()?;

    let (sender, receiver) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = sender.send(event);
    })?;
    watcher.watch(&folder, RecursiveMode::NonRecursive)?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let thread = std::thread::spawn(move || {
        // The watcher lives as long as the loop that consumes its events.
        let _watcher = watcher;
        let mut pending: HashMap<PathBuf, Pending> = HashMap::new();
        let mut done: HashMap<PathBuf, Signature> = HashMap::new();

        while !stop_flag.load(Ordering::Relaxed) {
            match receiver.recv_timeout(TICK) {
                Ok(Ok(event)) => {
                    if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                        for path in event.paths.into_iter().filter(|path| is_document(path)) {
                            pending.entry(path).or_insert_with(Pending::new);
                        }
                    }
                    continue;
                }
                Ok(Err(_)) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }

            for path in settled(&mut pending) {
                let Some(signature) = Signature::of(&path) else {
                    continue;
                };
                if done.get(&path) == Some(&signature) {
                    continue;
                }
                done.insert(path.clone(), signature);

                // A crash while handling one file must not end the watcher.
                let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
                    process_file(&path, &config, &options, printer.as_ref())
                }));
                match outcome {
                    Ok(Ok(processed)) => processed
                        .into_iter()
                        .for_each(|item| on_event(WatchEvent::Processed(item))),
                    Ok(Err(err)) => on_event(WatchEvent::Failed {
                        source: path,
                        message: err.to_string(),
                    }),
                    Err(crash) => on_event(WatchEvent::Failed {
                        source: path,
                        message: format!("the conversion crashed: {}", crash_message(&*crash)),
                    }),
                }
            }
        }
    });

    Ok(WatchHandle {
        stop,
        thread: Some(thread),
    })
}

/// Text of a panic payload, when it carries one.
fn crash_message(crash: &(dyn Any + Send)) -> &str {
    crash
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| crash.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown error")
}

/// A file seen by the watcher that may still be growing.
struct Pending {
    size: Option<u64>,
    unchanged_since: Instant,
}

impl Pending {
    fn new() -> Self {
        Self {
            size: None,
            unchanged_since: Instant::now(),
        }
    }
}

/// Removes and returns the files whose size has been stable for [`SETTLE`].
/// Files that disappeared are dropped.
fn settled(pending: &mut HashMap<PathBuf, Pending>) -> Vec<PathBuf> {
    let mut ready = Vec::new();
    pending.retain(|path, state| {
        let Ok(metadata) = std::fs::metadata(path) else {
            return false;
        };
        if state.size != Some(metadata.len()) {
            state.size = Some(metadata.len());
            state.unchanged_since = Instant::now();
            return true;
        }
        if metadata.len() > 0 && state.unchanged_since.elapsed() >= SETTLE {
            ready.push(path.clone());
            return false;
        }
        true
    });
    ready.sort();
    ready
}

/// Identifies a file version, so repeated events for the same content are
/// processed once.
#[derive(Debug, PartialEq, Eq)]
struct Signature {
    size: u64,
    modified: Option<SystemTime>,
}

impl Signature {
    fn of(path: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        Some(Self {
            size: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
}
