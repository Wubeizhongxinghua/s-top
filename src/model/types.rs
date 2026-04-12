use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::collector::RawSnapshot;

#[derive(Debug, Clone, Serialize)]
pub struct NodeRecord {
    pub partition: String,
    pub state: String,
    pub node_name: String,
    pub cpus: Option<u32>,
    pub memory_mb: Option<u64>,
    pub gpus: Option<u32>,
    pub gres: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobRecord {
    pub job_id: String,
    pub user: String,
    pub account: Option<String>,
    pub partition_raw: String,
    pub partitions: Vec<String>,
    pub name: String,
    pub state: String,
    pub runtime_raw: String,
    pub time_limit_raw: String,
    pub runtime_secs: Option<u64>,
    pub time_limit_secs: Option<u64>,
    pub nodes: u32,
    pub cpus: Option<u32>,
    pub memory_mb: Option<u64>,
    pub requested_gpus: Option<u32>,
    pub gres: Option<String>,
    pub req_tres: Option<String>,
    pub alloc_tres: Option<String>,
    pub location_or_reason: String,
    pub submit_time: Option<String>,
    pub priority: Option<u64>,
    pub is_mine: bool,
    pub active: bool,
    pub running: bool,
    pub pending: bool,
}

impl JobRecord {
    pub fn primary_partition(&self) -> &str {
        self.partitions
            .first()
            .map(String::as_str)
            .unwrap_or(self.partition_raw.as_str())
    }

    #[allow(dead_code)]
    pub fn resources_summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(cpus) = self.cpus {
            parts.push(format!("cpu={cpus}"));
        }
        if let Some(gpus) = self.requested_gpus {
            parts.push(format!("gpu={gpus}"));
        }
        if let Some(memory_mb) = self.memory_mb {
            parts.push(format!("mem={}", format_mem_mb(memory_mb)));
        }
        if self.nodes > 0 {
            parts.push(format!("node={}", self.nodes));
        }
        if parts.is_empty() {
            "n/a".to_string()
        } else {
            parts.join(" ")
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryRecord {
    pub job_id: String,
    pub name: String,
    pub user: String,
    pub partition: Option<String>,
    pub state: String,
    pub exit_code: String,
    pub elapsed_raw: String,
    pub elapsed_secs: Option<u64>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub alloc_tres: Option<String>,
    pub req_tres: Option<String>,
    pub alloc_cpus: Option<u32>,
    pub alloc_gpus: Option<u32>,
    pub alloc_mem_mb: Option<u64>,
    pub is_mine: bool,
}

impl HistoryRecord {
    #[allow(dead_code)]
    pub fn resources_summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(cpus) = self.alloc_cpus {
            parts.push(format!("cpu={cpus}"));
        }
        if let Some(gpus) = self.alloc_gpus {
            parts.push(format!("gpu={gpus}"));
        }
        if let Some(memory_mb) = self.alloc_mem_mb {
            parts.push(format!("mem={}", format_mem_mb(memory_mb)));
        }
        if parts.is_empty() {
            "n/a".to_string()
        } else {
            parts.join(" ")
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct JobDetail {
    pub job_id: String,
    pub name: Option<String>,
    pub user: Option<String>,
    pub account: Option<String>,
    pub partition: Option<String>,
    pub state: Option<String>,
    pub reason: Option<String>,
    pub exit_code: Option<String>,
    pub nodes: Option<u32>,
    pub n_tasks: Option<u32>,
    pub cpus: Option<u32>,
    pub memory_mb: Option<u64>,
    pub requested_gpus: Option<u32>,
    pub gres: Option<String>,
    pub req_tres: Option<String>,
    pub alloc_tres: Option<String>,
    pub runtime_raw: Option<String>,
    pub time_limit_raw: Option<String>,
    pub submit_time: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub node_list: Option<String>,
    pub work_dir: Option<String>,
    pub command: Option<String>,
    pub stdout_path: Option<String>,
    pub stderr_path: Option<String>,
    pub source_notes: Vec<String>,
    pub active: bool,
    pub is_mine: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct NodeDetail {
    pub node_name: String,
    pub partition: Option<String>,
    pub state: Option<String>,
    pub reason: Option<String>,
    pub cpu_alloc: Option<u32>,
    pub cpu_total: Option<u32>,
    pub mem_alloc_mb: Option<u64>,
    pub mem_total_mb: Option<u64>,
    pub gpu_alloc: Option<u32>,
    pub gpu_total: Option<u32>,
    pub gres: Option<String>,
    pub alloc_tres: Option<String>,
    pub cfg_tres: Option<String>,
    pub source_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartitionSpec {
    pub name: String,
    pub state: String,
    pub total_nodes: Option<u32>,
    pub total_cpus: Option<u64>,
    pub total_mem_mb: Option<u64>,
    pub total_gpus: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UsageStats {
    pub running_jobs: u32,
    pub pending_jobs: u32,
    pub running_nodes: u32,
    pub pending_nodes: u32,
    pub running_cpus: u64,
    pub pending_cpus: u64,
    pub running_gpus: u64,
    pub pending_gpus: u64,
}

impl UsageStats {
    pub fn observe(&mut self, job: &JobRecord) {
        if job.running {
            self.running_jobs += 1;
            self.running_nodes += job.nodes;
            self.running_cpus += u64::from(job.cpus.unwrap_or(0));
            self.running_gpus += u64::from(job.requested_gpus.unwrap_or(0));
        }
        if job.pending {
            self.pending_jobs += 1;
            self.pending_nodes += job.nodes;
            self.pending_cpus += u64::from(job.cpus.unwrap_or(0));
            self.pending_gpus += u64::from(job.requested_gpus.unwrap_or(0));
        }
    }

    pub fn running_total(&self, metric: MetricMode) -> u64 {
        match metric {
            MetricMode::Jobs => u64::from(self.running_jobs),
            MetricMode::Nodes => u64::from(self.running_nodes),
            MetricMode::Cpus => self.running_cpus,
            MetricMode::Gpus => self.running_gpus,
        }
    }

    pub fn pending_total(&self, metric: MetricMode) -> u64 {
        match metric {
            MetricMode::Jobs => u64::from(self.pending_jobs),
            MetricMode::Nodes => u64::from(self.pending_nodes),
            MetricMode::Cpus => self.pending_cpus,
            MetricMode::Gpus => self.pending_gpus,
        }
    }

    pub fn active_total(&self, metric: MetricMode) -> u64 {
        self.running_total(metric) + self.pending_total(metric)
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum MetricMode {
    Jobs,
    Nodes,
    Cpus,
    Gpus,
}

impl MetricMode {
    pub fn next(self) -> Self {
        match self {
            Self::Jobs => Self::Nodes,
            Self::Nodes => Self::Cpus,
            Self::Cpus => Self::Gpus,
            Self::Gpus => Self::Jobs,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Jobs => "jobs",
            Self::Nodes => "nodes",
            Self::Cpus => "cpu",
            Self::Gpus => "gpu",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PartitionOverview {
    pub name: String,
    pub state: String,
    pub total_nodes: u32,
    pub total_cpus: Option<u64>,
    pub total_mem_mb: Option<u64>,
    pub total_gpus: Option<u64>,
    pub node_state_counts: BTreeMap<String, u32>,
    pub mine: UsageStats,
    pub others: UsageStats,
}

impl PartitionOverview {
    pub fn total_usage(&self) -> UsageStats {
        UsageStats {
            running_jobs: self.mine.running_jobs + self.others.running_jobs,
            pending_jobs: self.mine.pending_jobs + self.others.pending_jobs,
            running_nodes: self.mine.running_nodes + self.others.running_nodes,
            pending_nodes: self.mine.pending_nodes + self.others.pending_nodes,
            running_cpus: self.mine.running_cpus + self.others.running_cpus,
            pending_cpus: self.mine.pending_cpus + self.others.pending_cpus,
            running_gpus: self.mine.running_gpus + self.others.running_gpus,
            pending_gpus: self.mine.pending_gpus + self.others.pending_gpus,
        }
    }

    pub fn used_for_pressure(&self, metric: MetricMode) -> u64 {
        match metric {
            MetricMode::Nodes => {
                u64::from(*self.node_state_counts.get("alloc").unwrap_or(&0))
                    + u64::from(*self.node_state_counts.get("mix").unwrap_or(&0))
            }
            _ => self.total_usage().running_total(metric),
        }
    }

    pub fn capacity_for(&self, metric: MetricMode) -> Option<u64> {
        match metric {
            MetricMode::Jobs => {
                let total = self.total_usage();
                Some(u64::from(total.running_jobs + total.pending_jobs).max(1))
            }
            MetricMode::Nodes => Some(u64::from(self.total_nodes).max(1)),
            MetricMode::Cpus => self.total_cpus.filter(|value| *value > 0),
            MetricMode::Gpus => self.total_gpus.filter(|value| *value > 0),
        }
    }

    pub fn pressure_ratio(&self, metric: MetricMode) -> Option<f64> {
        let usage = self.used_for_pressure(metric) as f64;
        self.capacity_for(metric)
            .map(|capacity| usage / (capacity as f64))
            .filter(|ratio| ratio.is_finite())
    }

    pub fn node_state_summary(&self) -> String {
        let mut parts = Vec::new();
        for key in ["idle", "mix", "alloc", "drain", "down"] {
            if let Some(count) = self.node_state_counts.get(key) {
                parts.push(format!("{key}:{count}"));
            }
        }
        if parts.is_empty() {
            "n/a".to_string()
        } else {
            parts.join(" ")
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UserUsage {
    pub user: String,
    pub is_current_user: bool,
    pub jobs: UsageStats,
    pub partitions: BTreeMap<String, UsageStats>,
}

impl UserUsage {
    pub fn total_jobs(&self) -> u32 {
        self.jobs.running_jobs + self.jobs.pending_jobs
    }

    pub fn total_nodes(&self) -> u32 {
        self.jobs.running_nodes + self.jobs.pending_nodes
    }

    pub fn total_cpus(&self) -> u64 {
        self.jobs.running_cpus + self.jobs.pending_cpus
    }

    pub fn total_gpus(&self) -> u64 {
        self.jobs.running_gpus + self.jobs.pending_gpus
    }

    pub fn top_partitions_summary(&self, limit: usize) -> String {
        let mut rows: Vec<(&String, &UsageStats)> = self.partitions.iter().collect();
        rows.sort_by(|(left_name, left_stats), (right_name, right_stats)| {
            right_stats
                .active_total(MetricMode::Jobs)
                .cmp(&left_stats.active_total(MetricMode::Jobs))
                .then_with(|| left_name.cmp(right_name))
        });
        if rows.is_empty() {
            return "No active partitions".to_string();
        }

        rows.into_iter()
            .take(limit)
            .map(|(name, stats)| {
                format!(
                    "{} (Running: {}, Pending: {})",
                    name, stats.running_jobs, stats.pending_jobs
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Capabilities {
    pub account: bool,
    pub submit_time: bool,
    pub priority: bool,
    pub job_cpus: bool,
    pub job_gpus: bool,
    pub partition_cpu_totals: bool,
    pub partition_gpu_totals: bool,
    pub partition_static: bool,
    pub sacct_history: bool,
    pub job_detail: bool,
    pub req_tres: bool,
    pub alloc_tres: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceHealth {
    pub nodes_ok: bool,
    pub jobs_ok: bool,
    pub partition_static_ok: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClusterSnapshot {
    pub collected_at: DateTime<Utc>,
    pub local_time: String,
    pub hostname: String,
    pub current_user: String,
    pub sample_duration_ms: u128,
    pub stale_partition_static: bool,
    pub degraded: bool,
    pub notes: Vec<String>,
    pub capabilities: Capabilities,
    pub source_health: SourceHealth,
    pub nodes: Vec<NodeRecord>,
    pub partitions: Vec<PartitionOverview>,
    pub jobs: Vec<JobRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugDump {
    pub raw: RawSnapshot,
    pub snapshot: ClusterSnapshot,
}

pub fn format_mem_mb(memory_mb: u64) -> String {
    if memory_mb >= 1024 * 1024 {
        format!("{:.1}T", memory_mb as f64 / 1024.0 / 1024.0)
    } else if memory_mb >= 1024 {
        format!("{:.1}G", memory_mb as f64 / 1024.0)
    } else {
        format!("{memory_mb}M")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{MetricMode, PartitionOverview, UsageStats};

    #[test]
    fn pressure_ratio_prefers_capacity_and_stays_bounded() {
        let mut node_state_counts = BTreeMap::new();
        node_state_counts.insert("alloc".to_string(), 2);
        node_state_counts.insert("mix".to_string(), 1);
        let partition = PartitionOverview {
            name: "gpu_l48".to_string(),
            state: "UP".to_string(),
            total_nodes: 4,
            total_cpus: Some(64),
            total_mem_mb: Some(256_000),
            total_gpus: Some(8),
            node_state_counts,
            mine: UsageStats {
                running_jobs: 1,
                pending_jobs: 0,
                running_nodes: 1,
                pending_nodes: 0,
                running_cpus: 8,
                pending_cpus: 0,
                running_gpus: 2,
                pending_gpus: 0,
            },
            others: UsageStats {
                running_jobs: 2,
                pending_jobs: 1,
                running_nodes: 2,
                pending_nodes: 1,
                running_cpus: 24,
                pending_cpus: 8,
                running_gpus: 4,
                pending_gpus: 0,
            },
        };

        assert_eq!(partition.pressure_ratio(MetricMode::Nodes), Some(0.75));
        assert_eq!(partition.pressure_ratio(MetricMode::Gpus), Some(0.75));
        assert_eq!(partition.used_for_pressure(MetricMode::Nodes), 3);
    }

    #[test]
    fn job_capacity_falls_back_to_observed_workload() {
        let partition = PartitionOverview {
            name: "cpu".to_string(),
            state: "UP".to_string(),
            total_nodes: 0,
            total_cpus: None,
            total_mem_mb: None,
            total_gpus: None,
            node_state_counts: BTreeMap::new(),
            mine: UsageStats {
                running_jobs: 1,
                pending_jobs: 1,
                running_nodes: 0,
                pending_nodes: 0,
                running_cpus: 0,
                pending_cpus: 0,
                running_gpus: 0,
                pending_gpus: 0,
            },
            others: UsageStats {
                running_jobs: 2,
                pending_jobs: 0,
                running_nodes: 0,
                pending_nodes: 0,
                running_cpus: 0,
                pending_cpus: 0,
                running_gpus: 0,
                pending_gpus: 0,
            },
        };

        assert_eq!(partition.capacity_for(MetricMode::Jobs), Some(4));
        assert_eq!(partition.pressure_ratio(MetricMode::Jobs), Some(0.75));
    }
}
