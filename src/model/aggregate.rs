use std::collections::{BTreeMap, BTreeSet};

use crate::collector::RawSnapshot;
use crate::model::parse::{parse_jobs, parse_nodes, parse_partition_specs};
use crate::model::types::{
    Capabilities, ClusterSnapshot, PartitionOverview, PartitionSpec, SourceHealth, UsageStats,
    UserUsage,
};

pub fn build_snapshot(raw: &RawSnapshot, current_user: &str) -> ClusterSnapshot {
    let mut notes = raw.degraded_notes.clone();

    let (nodes, parse_notes) = if raw.nodes.ok() || !raw.nodes.stdout.trim().is_empty() {
        parse_nodes(&raw.nodes.stdout)
    } else {
        (Vec::new(), Vec::new())
    };
    notes.extend(parse_notes);

    let (jobs, parse_notes) = if raw.jobs.ok() || !raw.jobs.stdout.trim().is_empty() {
        parse_jobs(&raw.jobs.stdout, current_user)
    } else {
        (Vec::new(), Vec::new())
    };
    notes.extend(parse_notes);

    let (partition_specs, parse_notes) = match &raw.partition_static {
        Some(capture) if capture.ok() || !capture.stdout.trim().is_empty() => {
            parse_partition_specs(&capture.stdout)
        }
        _ => (Vec::new(), Vec::new()),
    };
    notes.extend(parse_notes);

    let partitions = aggregate_partitions(&partition_specs, &nodes, &jobs);
    let capabilities = derive_capabilities(&partition_specs, &partitions, &jobs);

    ClusterSnapshot {
        collected_at: raw.collected_at,
        local_time: raw.local_time.clone(),
        hostname: raw.hostname.clone(),
        current_user: current_user.to_string(),
        sample_duration_ms: raw.sample_duration_ms,
        stale_partition_static: raw.stale_partition_static,
        degraded: !notes.is_empty()
            || !raw.nodes.ok()
            || !raw.jobs.ok()
            || raw
                .partition_static
                .as_ref()
                .is_some_and(|capture| !capture.ok()),
        notes,
        capabilities,
        source_health: SourceHealth {
            nodes_ok: raw.nodes.ok(),
            jobs_ok: raw.jobs.ok(),
            partition_static_ok: raw
                .partition_static
                .as_ref()
                .is_some_and(|capture| capture.ok()),
        },
        nodes,
        partitions,
        jobs,
    }
}

fn aggregate_partitions(
    specs: &[PartitionSpec],
    nodes: &[crate::model::types::NodeRecord],
    jobs: &[crate::model::types::JobRecord],
) -> Vec<PartitionOverview> {
    let mut partitions = BTreeMap::<String, PartitionOverview>::new();

    for spec in specs {
        partitions.insert(
            spec.name.clone(),
            PartitionOverview {
                name: spec.name.clone(),
                state: spec.state.clone(),
                total_nodes: spec.total_nodes.unwrap_or(0),
                total_cpus: spec.total_cpus,
                total_mem_mb: spec.total_mem_mb,
                total_gpus: spec.total_gpus,
                node_state_counts: BTreeMap::new(),
                mine: UsageStats::default(),
                others: UsageStats::default(),
            },
        );
    }

    for node in nodes {
        let should_infer_totals = !specs.iter().any(|spec| spec.name == node.partition);
        let entry = partitions
            .entry(node.partition.clone())
            .or_insert_with(|| PartitionOverview {
                name: node.partition.clone(),
                state: "UNKNOWN".to_string(),
                total_nodes: 0,
                total_cpus: None,
                total_mem_mb: None,
                total_gpus: None,
                node_state_counts: BTreeMap::new(),
                mine: UsageStats::default(),
                others: UsageStats::default(),
            });

        *entry
            .node_state_counts
            .entry(node.state.clone())
            .or_insert(0) += 1;

        if should_infer_totals {
            entry.total_nodes += 1;
            if entry.total_cpus.is_none() {
                entry.total_cpus = Some(0);
            }
            if let (Some(total), Some(cpus)) = (&mut entry.total_cpus, node.cpus) {
                *total += u64::from(cpus);
            }
            if entry.total_mem_mb.is_none() {
                entry.total_mem_mb = Some(0);
            }
            if let (Some(total), Some(memory_mb)) = (&mut entry.total_mem_mb, node.memory_mb) {
                *total += memory_mb;
            }
            if entry.total_gpus.is_none() && node.gpus.is_some() {
                entry.total_gpus = Some(0);
            }
            if let (Some(total), Some(gpus)) = (&mut entry.total_gpus, node.gpus) {
                *total += u64::from(gpus);
            }
        }
        if entry.state == "UNKNOWN" {
            entry.state = "UP".to_string();
        }
    }

    for job in jobs {
        let targets: BTreeSet<String> = job.partitions.iter().cloned().collect();
        for partition in targets {
            let entry = partitions
                .entry(partition.clone())
                .or_insert_with(|| PartitionOverview {
                    name: partition.clone(),
                    state: "UNKNOWN".to_string(),
                    total_nodes: 0,
                    total_cpus: None,
                    total_mem_mb: None,
                    total_gpus: None,
                    node_state_counts: BTreeMap::new(),
                    mine: UsageStats::default(),
                    others: UsageStats::default(),
                });

            if job.is_mine {
                entry.mine.observe(job);
            } else {
                entry.others.observe(job);
            }
        }
    }

    partitions.into_values().collect()
}

fn derive_capabilities(
    specs: &[PartitionSpec],
    partitions: &[PartitionOverview],
    jobs: &[crate::model::types::JobRecord],
) -> Capabilities {
    Capabilities {
        account: jobs.iter().any(|job| job.account.is_some()),
        submit_time: jobs.iter().any(|job| job.submit_time.is_some()),
        priority: jobs.iter().any(|job| job.priority.is_some()),
        job_cpus: jobs.iter().any(|job| job.cpus.is_some()),
        job_gpus: jobs.iter().any(|job| job.requested_gpus.is_some()),
        req_tres: jobs.iter().any(|job| job.req_tres.is_some()),
        alloc_tres: jobs.iter().any(|job| job.alloc_tres.is_some()),
        partition_cpu_totals: partitions
            .iter()
            .any(|partition| partition.total_cpus.is_some()),
        partition_gpu_totals: partitions
            .iter()
            .any(|partition| partition.total_gpus.is_some()),
        partition_static: !specs.is_empty(),
        sacct_history: false,
        job_detail: false,
    }
}

pub fn aggregate_users(
    jobs: &[crate::model::types::JobRecord],
    current_user: &str,
) -> Vec<UserUsage> {
    // User View is built from the same normalized active-job records as Overview and queue pages
    // so Mine / Others and user-level ownership always share one source of truth.
    let mut users = BTreeMap::<String, UserUsage>::new();

    for job in jobs.iter().filter(|job| job.active) {
        let entry = users.entry(job.user.clone()).or_insert_with(|| UserUsage {
            user: job.user.clone(),
            is_current_user: job.user == current_user,
            jobs: UsageStats::default(),
            partitions: BTreeMap::new(),
        });
        entry.jobs.observe(job);

        let partition_key = job.primary_partition().to_string();
        entry
            .partitions
            .entry(partition_key)
            .or_default()
            .observe(job);
    }

    users.into_values().collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::collector::RawSnapshot;
    use crate::collector::command::{CommandCapture, CommandStatus};

    use super::{aggregate_users, build_snapshot};

    fn ok_capture(program: &str, stdout: &str) -> CommandCapture {
        CommandCapture {
            program: program.to_string(),
            args: Vec::new(),
            status: CommandStatus::Success,
            exit_code: Some(0),
            stdout: stdout.to_string(),
            stderr: String::new(),
            duration_ms: 10,
        }
    }

    fn failed_capture(program: &str, stderr: &str) -> CommandCapture {
        CommandCapture {
            program: program.to_string(),
            args: Vec::new(),
            status: CommandStatus::Failed,
            exit_code: Some(1),
            stdout: String::new(),
            stderr: stderr.to_string(),
            duration_ms: 10,
        }
    }

    #[test]
    fn aggregates_my_vs_other_usage() {
        let raw = RawSnapshot {
            collected_at: Utc::now(),
            local_time: "2026-04-12 12:00:00".to_string(),
            hostname: "login04".to_string(),
            current_user: "myli".to_string(),
            sample_duration_ms: 20,
            stale_partition_static: false,
            degraded_notes: Vec::new(),
            nodes: ok_capture(
                "sinfo",
                "gpu_l48\x1falloc\x1fc56b01n01\x1f52\x1f515200\x1fgpu:l40:8",
            ),
            jobs: ok_capture(
                "squeue",
                "1\x1fmyli\x1fdemo\x1fgpu_l48\x1ftrain\x1fRUNNING\x1f1:00:00\x1f2:00:00\x1f1\x1f8\x1fN/A\x1f2026-04-12T12:00:00\x1f1\x1fc56b01n01\n2\x1fother\x1fdemo\x1fgpu_l48\x1fpending\x1fPENDING\x1f0:00\x1f2:00:00\x1f1\x1f4\x1fN/A\x1f2026-04-12T12:01:00\x1f1\x1f(Resources)",
            ),
            partition_static: Some(ok_capture(
                "scontrol",
                "PartitionName=gpu_l48 State=UP TotalCPUs=104 TotalNodes=2 TRES=cpu=104,mem=1030400M,node=2,billing=104,gres/gpu=16",
            )),
        };

        let snapshot = build_snapshot(&raw, "myli");
        assert_eq!(snapshot.partitions.len(), 1);
        let partition = &snapshot.partitions[0];
        assert_eq!(partition.mine.running_jobs, 1);
        assert_eq!(partition.others.pending_jobs, 1);
        assert_eq!(partition.total_gpus, Some(16));
    }

    #[test]
    fn aggregates_active_usage_per_user() {
        let raw = RawSnapshot {
            collected_at: Utc::now(),
            local_time: "2026-04-12 12:00:00".to_string(),
            hostname: "login04".to_string(),
            current_user: "myli".to_string(),
            sample_duration_ms: 20,
            stale_partition_static: false,
            degraded_notes: Vec::new(),
            nodes: ok_capture("sinfo", ""),
            jobs: ok_capture(
                "squeue",
                "1\x1fmyli\x1fdemo\x1fgpu_l48\x1ftrain\x1fRUNNING\x1f1:00:00\x1f2:00:00\x1f1\x1f8\x1fgres:gpu:1\x1f2026-04-12T12:00:00\x1f1\x1fc56b01n01\n2\x1fother\x1fdemo\x1fgpu_l48\x1fpending\x1fPENDING\x1f0:00\x1f2:00:00\x1f1\x1f4\x1fN/A\x1f2026-04-12T12:01:00\x1f1\x1f(Resources)\n3\x1fother\x1fdemo\x1fcn_long\x1fserve\x1fRUNNING\x1f0:15:00\x1f4:00:00\x1f2\x1f32\x1fN/A\x1f2026-04-12T12:05:00\x1f1\x1fc57b01n02",
            ),
            partition_static: None,
        };
        let snapshot = build_snapshot(&raw, "myli");
        let mut users = aggregate_users(&snapshot.jobs, "myli");
        users.sort_by(|left, right| left.user.cmp(&right.user));

        assert_eq!(users.len(), 2);
        assert!(users[0].is_current_user);
        assert_eq!(users[0].jobs.running_jobs, 1);
        assert_eq!(users[0].jobs.running_gpus, 1);
        assert_eq!(users[1].jobs.running_jobs, 1);
        assert_eq!(users[1].jobs.pending_jobs, 1);
        assert_eq!(
            users[1].top_partitions_summary(2),
            "cn_long (Running: 1, Pending: 0), gpu_l48 (Running: 0, Pending: 1)"
        );
    }

    #[test]
    fn degrades_gracefully_when_partition_static_is_missing() {
        let raw = RawSnapshot {
            collected_at: Utc::now(),
            local_time: "2026-04-12 12:00:00".to_string(),
            hostname: "login04".to_string(),
            current_user: "myli".to_string(),
            sample_duration_ms: 20,
            stale_partition_static: false,
            degraded_notes: vec!["partition static data unavailable".to_string()],
            nodes: ok_capture(
                "sinfo",
                "cn-long\x1fidle\x1fc59b01n01\x1f192\x1f386200\x1f(null)",
            ),
            jobs: ok_capture(
                "squeue",
                "1\x1fmyli\x1fdemo\x1fcn-long\x1ftrim\x1fRUNNING\x1f1:00:00\x1f2:00:00\x1f1\x1f8\x1fN/A\x1f2026-04-12T12:00:00\x1f1\x1fc59b01n01",
            ),
            partition_static: None,
        };

        let snapshot = build_snapshot(&raw, "myli");
        assert!(snapshot.degraded);
        assert!(snapshot.capabilities.partition_cpu_totals);
        assert!(!snapshot.capabilities.partition_gpu_totals);
    }

    #[test]
    fn surfaces_command_failures_without_crashing() {
        let raw = RawSnapshot {
            collected_at: Utc::now(),
            local_time: "2026-04-12 12:00:00".to_string(),
            hostname: "login04".to_string(),
            current_user: "myli".to_string(),
            sample_duration_ms: 20,
            stale_partition_static: false,
            degraded_notes: vec!["squeue failed: timeout".to_string()],
            nodes: ok_capture(
                "sinfo",
                "cn-long\x1fidle\x1fc59b01n01\x1f192\x1f386200\x1f(null)",
            ),
            jobs: failed_capture("squeue", "timeout"),
            partition_static: None,
        };

        let snapshot = build_snapshot(&raw, "myli");
        assert!(snapshot.degraded);
        assert!(snapshot.jobs.is_empty());
        assert!(!snapshot.notes.is_empty());
    }

    #[test]
    fn keeps_partial_job_output_when_squeue_times_out() {
        let raw = RawSnapshot {
            collected_at: Utc::now(),
            local_time: "2026-04-12 12:00:00".to_string(),
            hostname: "login04".to_string(),
            current_user: "myli".to_string(),
            sample_duration_ms: 20,
            stale_partition_static: false,
            degraded_notes: vec!["squeue failed".to_string()],
            nodes: ok_capture("sinfo", "gpu_l48\x1falloc\x1fc56b01n01\x1f52\x1f515200\x1fgpu:l40:8"),
            jobs: CommandCapture {
                program: "squeue".to_string(),
                args: Vec::new(),
                status: CommandStatus::TimedOut,
                exit_code: None,
                stdout: "1\x1fmyli\x1fdemo\x1fgpu_l48\x1ftrain\x1fRUNNING\x1f1:00:00\x1f2:00:00\x1f1\x1f8\x1fgres:gpu:1\x1f2026-04-12T12:00:00\x1f1\x1fc56b01n01".to_string(),
                stderr: String::new(),
                duration_ms: 5000,
            },
            partition_static: Some(ok_capture(
                "scontrol",
                "PartitionName=gpu_l48 State=UP TotalCPUs=104 TotalNodes=2 TRES=cpu=104,mem=1030400M,node=2,billing=104,gres/gpu=16",
            )),
        };

        let snapshot = build_snapshot(&raw, "myli");
        assert!(snapshot.degraded);
        assert_eq!(snapshot.jobs.len(), 1);
        assert_eq!(snapshot.partitions[0].mine.running_jobs, 1);
    }
}
