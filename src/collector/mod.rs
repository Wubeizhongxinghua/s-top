pub mod command;

use std::time::{Duration, Instant};

use chrono::{DateTime, Local, Utc};
use serde::Serialize;

use crate::cli::HistoryWindow;
use crate::cli::ResolvedCli;
use crate::collector::command::{CancelFlag, CommandCapture, CommandRunner};

pub const FIELD_SEP: char = '\u{1f}';
const PARTITION_CACHE_TTL: Duration = Duration::from_secs(30);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize)]
pub struct RawSnapshot {
    pub collected_at: DateTime<Utc>,
    pub local_time: String,
    pub hostname: String,
    pub current_user: String,
    pub sample_duration_ms: u128,
    pub stale_partition_static: bool,
    pub degraded_notes: Vec<String>,
    pub nodes: CommandCapture,
    pub jobs: CommandCapture,
    pub partition_static: Option<CommandCapture>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct HistoryRaw {
    pub window: HistoryWindow,
    pub all_users: bool,
    pub capture: CommandCapture,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobDetailRaw {
    pub job_id: String,
    pub live: CommandCapture,
    pub accounting: CommandCapture,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeDetailRaw {
    pub node_name: String,
    pub node: CommandCapture,
    pub jobs: CommandCapture,
}

pub struct Collector {
    runner: CommandRunner,
    cached_partition_static: Option<(Instant, CommandCapture)>,
    hostname: String,
    current_user: String,
}

impl Collector {
    pub fn new(settings: &ResolvedCli) -> Self {
        Self::with_cancel(settings, CancelFlag::default())
    }

    pub fn with_cancel(settings: &ResolvedCli, cancel: CancelFlag) -> Self {
        Self {
            runner: CommandRunner::with_cancel(COMMAND_TIMEOUT, cancel),
            cached_partition_static: None,
            hostname: resolve_hostname(),
            current_user: settings.user.clone(),
        }
    }

    pub fn collect_raw(&mut self) -> RawSnapshot {
        let started = Instant::now();
        let nodes = self.collect_nodes();
        let jobs = self.collect_jobs();
        let (partition_static, stale_partition_static) = self.collect_partition_static();

        let mut degraded_notes = Vec::new();
        for capture in [&nodes, &jobs] {
            if let Some(error) = capture.short_error() {
                degraded_notes.push(error);
            }
        }

        if let Some(capture) = &partition_static {
            if let Some(error) = capture.short_error() {
                degraded_notes.push(format!("partition static data unavailable: {error}"));
            }
        } else {
            degraded_notes.push("partition static data unavailable".to_string());
        }

        RawSnapshot {
            collected_at: Utc::now(),
            local_time: Local::now().format("%F %T").to_string(),
            hostname: self.hostname.clone(),
            current_user: self.current_user.clone(),
            sample_duration_ms: started.elapsed().as_millis(),
            stale_partition_static,
            degraded_notes,
            nodes,
            jobs,
            partition_static,
        }
    }

    fn collect_nodes(&self) -> CommandCapture {
        self.runner.run(
            "sinfo",
            &[
                "-Nh".to_string(),
                "-o".to_string(),
                format!("%P{sep}%t{sep}%N{sep}%c{sep}%m{sep}%G", sep = FIELD_SEP),
            ],
        )
    }

    fn collect_jobs(&self) -> CommandCapture {
        self.runner.run(
            "squeue",
            &[
                "-h".to_string(),
                "-t".to_string(),
                "PENDING,RUNNING,CONFIGURING,COMPLETING,SUSPENDED".to_string(),
                "-o".to_string(),
                format!(
                    "%i{sep}%u{sep}%a{sep}%P{sep}%j{sep}%T{sep}%M{sep}%l{sep}%D{sep}%C{sep}%b{sep}%V{sep}%Q{sep}%R",
                    sep = FIELD_SEP
                ),
            ],
        )
    }

    fn collect_partition_static(&mut self) -> (Option<CommandCapture>, bool) {
        if let Some((captured_at, capture)) = &self.cached_partition_static {
            if captured_at.elapsed() < PARTITION_CACHE_TTL {
                return (Some(capture.clone()), true);
            }
        }

        let capture = self
            .runner
            .run("scontrol", &["show".to_string(), "partition".to_string()]);

        if capture.ok() {
            self.cached_partition_static = Some((Instant::now(), capture.clone()));
        }

        (Some(capture), false)
    }

    #[allow(dead_code)]
    pub fn collect_history(&self, window: HistoryWindow, all_users: bool) -> HistoryRaw {
        let mut args = vec![
            "-n".to_string(),
            "-P".to_string(),
            "-X".to_string(),
            "-S".to_string(),
            window.sacct_start().to_string(),
            "-o".to_string(),
            "JobIDRaw,JobName,User,Partition,State,ExitCode,Elapsed,Start,End,AllocTRES,ReqTRES"
                .to_string(),
        ];

        if all_users {
            args.insert(0, "-a".to_string());
        }

        HistoryRaw {
            window,
            all_users,
            capture: self.runner.run("sacct", &args),
        }
    }

    pub fn collect_job_detail(&self, job_id: &str) -> JobDetailRaw {
        JobDetailRaw {
            job_id: job_id.to_string(),
            live: self.runner.run(
                "scontrol",
                &["show".to_string(), "job".to_string(), "-o".to_string(), job_id.to_string()],
            ),
            accounting: self.runner.run(
                "sacct",
                &[
                    "-n".to_string(),
                    "-P".to_string(),
                    "-X".to_string(),
                    "-j".to_string(),
                    job_id.to_string(),
                    "-o".to_string(),
                    "JobIDRaw,JobName,User,Account,Partition,State,ExitCode,Elapsed,Submit,Start,End,AllocTRES,ReqTRES,NNodes,NTasks,NodeList".to_string(),
                ],
            ),
        }
    }

    pub fn cancel_jobs(&self, job_ids: &[String]) -> CommandCapture {
        let mut args = Vec::with_capacity(job_ids.len());
        args.extend(job_ids.iter().cloned());
        self.runner.run("scancel", &args)
    }

    pub fn collect_node_detail(&self, node_name: &str) -> NodeDetailRaw {
        NodeDetailRaw {
            node_name: node_name.to_string(),
            node: self.runner.run(
                "scontrol",
                &[
                    "show".to_string(),
                    "node".to_string(),
                    "-o".to_string(),
                    node_name.to_string(),
                ],
            ),
            jobs: self.runner.run(
                "squeue",
                &[
                    "-h".to_string(),
                    "-w".to_string(),
                    node_name.to_string(),
                    "-t".to_string(),
                    "PENDING,RUNNING,CONFIGURING,COMPLETING,SUSPENDED".to_string(),
                    "-o".to_string(),
                    format!(
                        "%i{sep}%u{sep}%a{sep}%P{sep}%j{sep}%T{sep}%M{sep}%l{sep}%D{sep}%C{sep}%b{sep}%V{sep}%Q{sep}%R",
                        sep = FIELD_SEP
                    ),
                ],
            ),
        }
    }
}

fn resolve_hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown-host".to_string())
}
