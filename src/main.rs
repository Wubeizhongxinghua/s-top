mod app;
mod cli;
mod collector;
mod config;
mod model;
mod ui;

use anyhow::Result;
use clap::Parser;

use crate::app::{print_debug_dump, print_once_summary, run_tui};
use crate::cli::{Cli, ResolvedCli};
use crate::collector::Collector;
use crate::config::FileConfig;
use crate::model::{DebugDump, build_snapshot};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let file_config = FileConfig::load()?;
    let settings = ResolvedCli::resolve(cli, file_config)?;

    if settings.debug_dump || settings.once {
        let mut collector = Collector::new(&settings);
        let raw = collector.collect_raw();
        let snapshot = build_snapshot(&raw, &settings.user);

        if settings.debug_dump {
            let dump = DebugDump {
                raw,
                snapshot: snapshot.clone(),
            };
            print_debug_dump(&dump)?;
            return Ok(());
        }

        print_once_summary(&snapshot);
        return Ok(());
    }

    run_tui(settings)
}
