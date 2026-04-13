use std::env;
use std::process::Command;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};

use crate::config::FileConfig;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ValueEnum, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    #[default]
    Auto,
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ValueEnum, PartialEq, Eq, Default)]
pub enum HistoryWindow {
    #[serde(rename = "24h")]
    #[value(name = "24h", alias = "h24")]
    #[default]
    H24,
    #[serde(rename = "3d")]
    #[value(name = "3d", alias = "d3")]
    D3,
    #[serde(rename = "7d")]
    #[value(name = "7d", alias = "d7")]
    D7,
}

#[allow(dead_code)]
impl HistoryWindow {
    pub fn label(self) -> &'static str {
        match self {
            Self::H24 => "24h",
            Self::D3 => "3d",
            Self::D7 => "7d",
        }
    }

    pub fn sacct_start(self) -> &'static str {
        match self {
            Self::H24 => "now-24hours",
            Self::D3 => "now-3days",
            Self::D7 => "now-7days",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::H24 => Self::D3,
            Self::D3 => Self::D7,
            Self::D7 => Self::H24,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "sqtop",
    version,
    about = "Interactive Slurm partition and queue monitor"
)]
pub struct Cli {
    #[arg(long)]
    pub interval: Option<f64>,
    #[arg(long)]
    pub user: Option<String>,
    #[arg(long, help = "Start on the All Jobs page")]
    pub all: bool,
    #[arg(long, help = "Disable the All Jobs page")]
    pub no_all_jobs: bool,
    #[arg(long, value_enum)]
    pub theme: Option<ThemeChoice>,
    #[arg(long, help = "Collect once and print raw + parsed data as JSON")]
    pub debug_dump: bool,
    #[arg(long, help = "Collect once and print a plain-text summary")]
    pub once: bool,
    #[arg(long)]
    pub compact: bool,
    #[arg(long)]
    pub no_color: bool,
    #[arg(long, value_enum, help = "Default history page window")]
    pub history_window: Option<HistoryWindow>,
    #[arg(long, help = "Show advanced resource columns when available")]
    pub advanced_resources: bool,
    #[arg(long, help = "Hide advanced resource columns")]
    pub no_advanced_resources: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedCli {
    pub interval: f64,
    pub user: String,
    pub start_in_all_jobs: bool,
    pub all_jobs_enabled: bool,
    pub theme: ThemeChoice,
    pub debug_dump: bool,
    pub once: bool,
    pub compact: bool,
    pub no_color: bool,
    pub history_window: HistoryWindow,
    pub show_advanced_resources: bool,
}

impl ResolvedCli {
    pub fn resolve(cli: Cli, file: Option<FileConfig>) -> Result<Self> {
        let file = file.unwrap_or_default();
        let interval = cli.interval.or(file.interval).unwrap_or(2.0);
        let interval = interval.max(0.25);

        let user = cli
            .user
            .or(file.user)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(detect_current_user);

        let all_jobs_enabled = if cli.no_all_jobs {
            false
        } else {
            file.all_jobs_enabled.unwrap_or(true)
        };

        let start_in_all_jobs = if cli.all {
            true
        } else {
            file.start_in_all_jobs.unwrap_or(false) && all_jobs_enabled
        };

        let theme = cli.theme.or(file.theme).unwrap_or_default();
        let compact = cli.compact || file.compact.unwrap_or(false);
        let no_color = cli.no_color || file.no_color.unwrap_or(false);
        let history_window = cli
            .history_window
            .or(file.history_window)
            .unwrap_or_default();
        let show_advanced_resources = if cli.no_advanced_resources {
            false
        } else if cli.advanced_resources {
            true
        } else {
            file.show_advanced_resources.unwrap_or(true)
        };

        Ok(Self {
            interval,
            user,
            start_in_all_jobs,
            all_jobs_enabled,
            theme,
            debug_dump: cli.debug_dump,
            once: cli.once,
            compact,
            no_color,
            history_window,
            show_advanced_resources,
        })
    }
}

fn detect_current_user() -> String {
    detect_current_user_from_sources(
        env::var("USER").ok().as_deref(),
        env::var("LOGNAME").ok().as_deref(),
        Command::new("id")
            .arg("-un")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .as_ref()
            .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
            .as_deref(),
    )
    .unwrap_or_else(|| "unknown".to_string())
}

pub(crate) fn detect_current_user_from_sources(
    user_env: Option<&str>,
    logname_env: Option<&str>,
    id_output: Option<&str>,
) -> Option<String> {
    for key in ["USER", "LOGNAME"] {
        let candidate = match key {
            "USER" => user_env,
            _ => logname_env,
        };
        if let Some(value) = candidate {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    id_output
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{HistoryWindow, detect_current_user_from_sources};

    #[test]
    fn detects_user_from_env_prefer_user_over_logname() {
        let user = detect_current_user_from_sources(Some("alice"), Some("bob"), Some("carol"));
        assert_eq!(user.as_deref(), Some("alice"));
    }

    #[test]
    fn detects_user_from_logname_and_id_output() {
        let from_logname = detect_current_user_from_sources(None, Some("bob"), Some("carol"));
        let from_id = detect_current_user_from_sources(None, None, Some("carol\n"));
        assert_eq!(from_logname.as_deref(), Some("bob"));
        assert_eq!(from_id.as_deref(), Some("carol"));
    }

    #[test]
    fn cycles_history_window() {
        assert_eq!(HistoryWindow::H24.next(), HistoryWindow::D3);
        assert_eq!(HistoryWindow::D3.next(), HistoryWindow::D7);
        assert_eq!(HistoryWindow::D7.next(), HistoryWindow::H24);
    }
}
