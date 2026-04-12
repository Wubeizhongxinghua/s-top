use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::cli::{HistoryWindow, ThemeChoice};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileConfig {
    pub interval: Option<f64>,
    pub user: Option<String>,
    pub all_jobs_enabled: Option<bool>,
    pub start_in_all_jobs: Option<bool>,
    pub compact: Option<bool>,
    pub no_color: Option<bool>,
    pub theme: Option<ThemeChoice>,
    pub history_window: Option<HistoryWindow>,
    pub show_advanced_resources: Option<bool>,
}

impl FileConfig {
    pub fn load() -> Result<Option<Self>> {
        let Some(path) = config_path() else {
            return Ok(None);
        };

        if !path.exists() {
            return Ok(None);
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config file {}", path.display()))?;
        let config = toml::from_str(&contents)
            .with_context(|| format!("failed to parse config file {}", path.display()))?;
        Ok(Some(config))
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|path| path.join("s-top").join("config.toml"))
}
