//! Loads render options from a JSON configuration file.
//!
//! The file holds the [`Settings`] fields plus two optional paths, `logo`
//! and `template`, resolved relative to the configuration file.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::{Logo, RenderError, RenderOptions, Settings};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read `{}`: {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid configuration `{}`: {message}", path.display())]
    Invalid { path: PathBuf, message: String },
    #[error(transparent)]
    Render(#[from] RenderError),
}

/// Reads a configuration file into render options.
pub fn load(path: &Path) -> Result<RenderOptions, ConfigError> {
    let invalid = |message: String| ConfigError::Invalid {
        path: path.to_owned(),
        message,
    };
    let mut json: Value =
        serde_json::from_slice(&read(path)?).map_err(|err| invalid(err.to_string()))?;
    let fields = json
        .as_object_mut()
        .ok_or_else(|| invalid("expected a JSON object".into()))?;

    let base = path.parent().unwrap_or(Path::new("."));
    let mut linked = |key: &str| -> Result<Option<PathBuf>, ConfigError> {
        match fields.remove(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(file)) => Ok(Some(base.join(file))),
            Some(_) => Err(invalid(format!("`{key}` must be a file path"))),
        }
    };
    let logo = linked("logo")?;
    let template = linked("template")?;

    let settings: Settings =
        serde_json::from_value(json).map_err(|err| invalid(err.to_string()))?;

    Ok(RenderOptions {
        settings,
        logo: logo.as_deref().map(load_logo).transpose()?,
        template: template.as_deref().map(load_template).transpose()?,
    })
}

/// Reads a logo image from disk.
pub fn load_logo(path: &Path) -> Result<Logo, ConfigError> {
    Ok(Logo::from_bytes(read(path)?)?)
}

/// Reads a custom Typst template from disk.
pub fn load_template(path: &Path) -> Result<String, ConfigError> {
    String::from_utf8(read(path)?).map_err(|_| ConfigError::Invalid {
        path: path.to_owned(),
        message: "the template is not valid UTF-8".into(),
    })
}

fn read(path: &Path) -> Result<Vec<u8>, ConfigError> {
    std::fs::read(path).map_err(|source| ConfigError::Read {
        path: path.to_owned(),
        source,
    })
}
