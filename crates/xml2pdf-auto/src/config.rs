//! Automation settings, stored as JSON.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use xml2pdf_render::config::{load_logo, load_template};
use xml2pdf_render::{RenderOptions, Settings};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
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
    #[error("invalid configuration `{}`: {message}", path.display())]
    Invalid { path: PathBuf, message: String },
    #[error("number of copies must be between 1 and 20")]
    Copies,
    #[error(transparent)]
    Render(#[from] xml2pdf_render::config::ConfigError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AutoConfig {
    /// Folder watched for new XML or ZIP files.
    pub watch_dir: Option<PathBuf>,
    /// Folder receiving the PDFs. Defaults to `pdf` inside the watched folder.
    pub output_dir: Option<PathBuf>,
    /// Send every generated PDF to the printer.
    pub print: bool,
    /// Printer name. `None` uses the system default printer.
    pub printer: Option<String>,
    pub copies: u32,
    /// Start watching when the desktop app opens.
    pub watch_on_start: bool,
    pub settings: Settings,
    pub logo: Option<PathBuf>,
    pub template: Option<PathBuf>,
}

impl Default for AutoConfig {
    fn default() -> Self {
        Self {
            watch_dir: None,
            output_dir: None,
            print: false,
            printer: None,
            copies: 1,
            watch_on_start: false,
            settings: Settings::default(),
            logo: None,
            template: None,
        }
    }
}

impl AutoConfig {
    /// Loads the configuration, or the defaults when the file does not exist.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(ConfigError::Read {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        let config: Self = serde_json::from_slice(&bytes).map_err(|err| ConfigError::Invalid {
            path: path.to_owned(),
            message: err.to_string(),
        })?;
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        self.validate()?;
        let write_error = |source| ConfigError::Write {
            path: path.to_owned(),
            source,
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(write_error)?;
        }
        let json = serde_json::to_vec_pretty(self).expect("configuration serializes to JSON");
        std::fs::write(path, json).map_err(write_error)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if !(1..=20).contains(&self.copies) {
            return Err(ConfigError::Copies);
        }
        Ok(())
    }

    /// Folder receiving the PDFs for a given input file.
    pub fn output_dir_for(&self, input: &Path) -> PathBuf {
        match (&self.output_dir, &self.watch_dir) {
            (Some(output), _) => output.clone(),
            (None, Some(watched)) => watched.join("pdf"),
            (None, None) => input.parent().unwrap_or(Path::new(".")).join("pdf"),
        }
    }

    /// Reads the logo and template files into render options.
    pub fn render_options(&self) -> Result<RenderOptions, ConfigError> {
        self.validate()?;
        Ok(RenderOptions {
            settings: self.settings.clone(),
            logo: self.logo.as_deref().map(load_logo).transpose()?,
            template: self.template.as_deref().map(load_template).transpose()?,
        })
    }
}
