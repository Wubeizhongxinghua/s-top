use std::collections::BTreeMap;

use crate::collector::FIELD_SEP;
use crate::model::types::{
    HistoryRecord, JobDetail, JobRecord, NodeDetail, NodeRecord, PartitionSpec,
};

pub fn parse_nodes(stdout: &str) -> (Vec<NodeRecord>, Vec<String>) {
    let mut nodes = Vec::new();
    let mut notes = Vec::new();

    for (index, line) in stdout.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let fields: Vec<&str> = line.split(FIELD_SEP).collect();
        if fields.len() != 6 {
            notes.push(format!(
                "ignored sinfo node line {} with {} fields",
                index + 1,
                fields.len()
            ));
            continue;
        }

        let partition = normalize_partition(fields[0]);
        let state = normalize_node_state(fields[1]);
        let node_name = fields[2].trim().to_string();
        let cpus = parse_u32(fields[3]);
        let memory_mb = parse_u64(fields[4]);
        let gres = clean_optional(fields[5]);
        let gpus = gres.as_deref().and_then(parse_gpu_count);

        nodes.push(NodeRecord {
            partition,
            state,
            node_name,
            cpus,
            memory_mb,
            gpus,
            gres,
        });
    }

    (nodes, notes)
}

pub fn parse_jobs(stdout: &str, current_user: &str) -> (Vec<JobRecord>, Vec<String>) {
    let mut jobs = Vec::new();
    let mut notes = Vec::new();

    for (index, line) in stdout.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let fields: Vec<&str> = line.split(FIELD_SEP).collect();
        if fields.len() != 14 {
            notes.push(format!(
                "ignored squeue line {} with {} fields",
                index + 1,
                fields.len()
            ));
            continue;
        }

        let partition_raw = fields[3].trim().to_string();
        let partitions = split_partitions(&partition_raw);
        let state = fields[5].trim().to_uppercase();
        let gres = clean_optional(fields[10]);
        let job = JobRecord {
            job_id: fields[0].trim().to_string(),
            user: fields[1].trim().to_string(),
            account: clean_optional(fields[2]),
            partition_raw,
            partitions,
            name: fields[4].trim().to_string(),
            state: state.clone(),
            runtime_raw: fields[6].trim().to_string(),
            time_limit_raw: fields[7].trim().to_string(),
            runtime_secs: parse_slurm_duration(fields[6]),
            time_limit_secs: parse_slurm_duration(fields[7]),
            nodes: parse_u32(fields[8]).unwrap_or(0),
            cpus: parse_u32(fields[9]),
            memory_mb: None,
            requested_gpus: gres.as_deref().and_then(parse_gpu_count),
            gres,
            req_tres: None,
            alloc_tres: None,
            location_or_reason: fields[13].trim().to_string(),
            submit_time: clean_optional(fields[11]),
            priority: parse_u64(fields[12]),
            is_mine: fields[1].trim() == current_user,
            active: matches!(
                state.as_str(),
                "RUNNING" | "PENDING" | "CONFIGURING" | "COMPLETING" | "SUSPENDED"
            ),
            running: matches!(state.as_str(), "RUNNING" | "COMPLETING"),
            pending: matches!(state.as_str(), "PENDING" | "CONFIGURING"),
        };

        jobs.push(job);
    }

    (jobs, notes)
}

pub fn parse_partition_specs(stdout: &str) -> (Vec<PartitionSpec>, Vec<String>) {
    let mut specs = Vec::new();
    let mut notes = Vec::new();

    for block in stdout.split("\n\n") {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }

        let mut values = BTreeMap::new();
        for token in block.split_whitespace() {
            if let Some((key, value)) = token.split_once('=') {
                values.insert(key.to_string(), value.to_string());
            }
        }

        let Some(name) = values.get("PartitionName").cloned() else {
            notes.push("ignored partition block without PartitionName".to_string());
            continue;
        };

        let state = values
            .get("State")
            .cloned()
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let total_nodes = values.get("TotalNodes").and_then(|value| parse_u32(value));
        let total_cpus = values.get("TotalCPUs").and_then(|value| parse_u64(value));
        let tres = values.get("TRES").cloned().unwrap_or_default();
        let tres_map = parse_resource_map(&tres);
        let total_mem_mb = tres_map.get("mem").copied();
        let total_gpus = tres_map
            .get("gres/gpu")
            .copied()
            .or_else(|| tres_map.get("gpu").copied());

        specs.push(PartitionSpec {
            name,
            state,
            total_nodes,
            total_cpus,
            total_mem_mb,
            total_gpus,
        });
    }

    (specs, notes)
}

#[allow(dead_code)]
pub fn parse_history(stdout: &str, current_user: &str) -> (Vec<HistoryRecord>, Vec<String>) {
    let mut rows = Vec::new();
    let mut notes = Vec::new();

    for (index, line) in stdout.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let fields: Vec<&str> = line.split('|').collect();
        if fields.len() != 11 {
            notes.push(format!(
                "ignored sacct line {} with {} fields",
                index + 1,
                fields.len()
            ));
            continue;
        }

        let job_id = fields[0].trim().to_string();
        if job_id.contains('.') {
            continue;
        }

        let alloc_tres = clean_optional(fields[9]);
        let req_tres = clean_optional(fields[10]);
        let alloc_map = alloc_tres
            .as_deref()
            .map(parse_resource_map)
            .unwrap_or_default();

        rows.push(HistoryRecord {
            job_id,
            name: fields[1].trim().to_string(),
            user: fields[2].trim().to_string(),
            partition: clean_optional(fields[3]),
            state: fields[4].trim().to_string(),
            exit_code: fields[5].trim().to_string(),
            elapsed_raw: fields[6].trim().to_string(),
            elapsed_secs: parse_slurm_duration(fields[6]),
            start: clean_optional(fields[7]),
            end: clean_optional(fields[8]),
            alloc_tres,
            req_tres,
            alloc_cpus: alloc_map.get("cpu").copied().map(|value| value as u32),
            alloc_gpus: alloc_map
                .get("gres/gpu")
                .copied()
                .or_else(|| alloc_map.get("gpu").copied())
                .map(|value| value as u32),
            alloc_mem_mb: alloc_map.get("mem").copied(),
            is_mine: fields[2].trim() == current_user,
        });
    }

    (rows, notes)
}

pub fn parse_scontrol_job(stdout: &str, current_user: &str) -> (Option<JobDetail>, Vec<String>) {
    let line = stdout.trim();
    if line.is_empty() {
        return (None, vec!["empty scontrol job output".to_string()]);
    }

    let map = parse_key_value_line(line);
    let Some(job_id) = map.get("JobId").cloned() else {
        return (
            None,
            vec!["missing JobId in scontrol show job output".to_string()],
        );
    };

    let user = map
        .get("UserId")
        .map(|value| value.split('(').next().unwrap_or(value).to_string());
    let req_tres = map.get("TRES").cloned();
    let req_tres_map = req_tres
        .as_deref()
        .map(parse_resource_map)
        .unwrap_or_default();
    let tres_per_node = map.get("TresPerNode").cloned();
    let gpu_count = tres_per_node
        .as_deref()
        .and_then(parse_gpu_count)
        .or_else(|| {
            req_tres_map
                .get("gres/gpu")
                .copied()
                .map(|value| value as u32)
        });

    (
        Some(JobDetail {
            job_id,
            name: map.get("JobName").cloned(),
            user: user.clone(),
            account: map.get("Account").cloned(),
            partition: map.get("Partition").cloned(),
            state: map.get("JobState").cloned(),
            reason: map.get("Reason").cloned(),
            exit_code: map.get("ExitCode").cloned(),
            nodes: map.get("NumNodes").and_then(|value| parse_u32(value)),
            n_tasks: map.get("NumTasks").and_then(|value| parse_u32(value)),
            cpus: map.get("NumCPUs").and_then(|value| parse_u32(value)),
            memory_mb: req_tres_map.get("mem").copied(),
            requested_gpus: gpu_count,
            gres: tres_per_node,
            req_tres: req_tres.clone(),
            alloc_tres: None,
            runtime_raw: map.get("RunTime").cloned(),
            time_limit_raw: map.get("TimeLimit").cloned(),
            submit_time: map.get("SubmitTime").cloned(),
            start_time: map.get("StartTime").cloned(),
            end_time: map.get("EndTime").cloned(),
            node_list: map
                .get("NodeList")
                .cloned()
                .or_else(|| map.get("ReqNodeList").cloned()),
            work_dir: map.get("WorkDir").cloned(),
            command: map.get("Command").cloned(),
            stdout_path: map.get("StdOut").cloned(),
            stderr_path: map.get("StdErr").cloned(),
            source_notes: Vec::new(),
            active: map.get("JobState").is_some_and(|state| {
                matches!(
                    state.as_str(),
                    "RUNNING" | "PENDING" | "CONFIGURING" | "COMPLETING" | "SUSPENDED"
                )
            }),
            is_mine: user.as_deref() == Some(current_user),
        }),
        Vec::new(),
    )
}

pub fn parse_scontrol_node(stdout: &str) -> (Option<NodeDetail>, Vec<String>) {
    let line = stdout.trim();
    if line.is_empty() {
        return (None, vec!["empty scontrol node output".to_string()]);
    }

    let map = parse_key_value_line(line);
    let Some(node_name) = map.get("NodeName").cloned() else {
        return (
            None,
            vec!["missing NodeName in scontrol show node output".to_string()],
        );
    };

    let cfg_tres = map.get("CfgTRES").cloned();
    let alloc_tres = map.get("AllocTRES").cloned();
    let cfg_map = cfg_tres
        .as_deref()
        .map(parse_resource_map)
        .unwrap_or_default();
    let alloc_map = alloc_tres
        .as_deref()
        .map(parse_resource_map)
        .unwrap_or_default();

    (
        Some(NodeDetail {
            node_name,
            partition: map
                .get("Partitions")
                .map(|value| value.split(',').next().unwrap_or(value).to_string()),
            state: map.get("State").cloned(),
            reason: map.get("Reason").cloned(),
            cpu_alloc: map.get("CPUAlloc").and_then(|value| parse_u32(value)),
            cpu_total: map.get("CPUTot").and_then(|value| parse_u32(value)),
            mem_alloc_mb: map.get("AllocMem").and_then(|value| parse_u64(value)),
            mem_total_mb: map.get("RealMemory").and_then(|value| parse_u64(value)),
            gpu_alloc: alloc_map
                .get("gres/gpu")
                .copied()
                .or_else(|| alloc_map.get("gpu").copied())
                .map(|value| value as u32),
            gpu_total: cfg_map
                .get("gres/gpu")
                .copied()
                .or_else(|| cfg_map.get("gpu").copied())
                .map(|value| value as u32),
            gres: map.get("Gres").cloned(),
            alloc_tres,
            cfg_tres,
            source_notes: Vec::new(),
        }),
        Vec::new(),
    )
}

pub fn parse_history_detail(stdout: &str, current_user: &str) -> (Option<JobDetail>, Vec<String>) {
    let (rows, notes) = parse_history_detail_rows(stdout);
    let Some(row) = rows.into_iter().next() else {
        return (None, notes);
    };

    let alloc_map = row
        .alloc_tres
        .as_deref()
        .map(parse_resource_map)
        .unwrap_or_default();
    let req_map = row
        .req_tres
        .as_deref()
        .map(parse_resource_map)
        .unwrap_or_default();

    (
        Some(JobDetail {
            job_id: row.job_id.clone(),
            name: Some(row.name.clone()),
            user: Some(row.user.clone()),
            account: row.account.clone(),
            partition: row.partition.clone(),
            state: Some(row.state.clone()),
            reason: None,
            exit_code: Some(row.exit_code.clone()),
            nodes: row.nodes,
            n_tasks: row.n_tasks,
            cpus: alloc_map
                .get("cpu")
                .copied()
                .or_else(|| req_map.get("cpu").copied())
                .map(|value| value as u32),
            memory_mb: alloc_map
                .get("mem")
                .copied()
                .or_else(|| req_map.get("mem").copied()),
            requested_gpus: alloc_map
                .get("gres/gpu")
                .copied()
                .or_else(|| req_map.get("gres/gpu").copied())
                .map(|value| value as u32),
            gres: row.alloc_tres.clone(),
            req_tres: row.req_tres.clone(),
            alloc_tres: row.alloc_tres.clone(),
            runtime_raw: Some(row.elapsed_raw.clone()),
            time_limit_raw: None,
            submit_time: row.submit.clone(),
            start_time: row.start.clone(),
            end_time: row.end.clone(),
            node_list: row.node_list.clone(),
            work_dir: None,
            command: None,
            stdout_path: None,
            stderr_path: None,
            source_notes: notes.clone(),
            active: row.end.as_deref() == Some("Unknown") || row.state == "RUNNING",
            is_mine: row.user == current_user,
        }),
        notes,
    )
}

pub fn parse_slurm_duration(raw: &str) -> Option<u64> {
    let raw = raw.trim();
    if raw.is_empty() || matches!(raw, "UNLIMITED" | "N/A" | "Partition_Limit") {
        return None;
    }

    let (days, rest) = if let Some((days, rest)) = raw.split_once('-') {
        (days.parse::<u64>().ok()?, rest)
    } else {
        (0, raw)
    };

    let parts: Vec<&str> = rest.split(':').collect();
    let seconds = match parts.as_slice() {
        [minutes, seconds] => minutes.parse::<u64>().ok()? * 60 + seconds.parse::<u64>().ok()?,
        [hours, minutes, seconds] => {
            hours.parse::<u64>().ok()? * 3600
                + minutes.parse::<u64>().ok()? * 60
                + seconds.parse::<u64>().ok()?
        }
        _ => return None,
    };

    Some(days * 86_400 + seconds)
}

pub fn parse_gpu_count(raw: &str) -> Option<u32> {
    let raw = raw.trim();
    if raw.is_empty() || matches!(raw, "N/A" | "(null)") {
        return None;
    }

    let mut total = 0_u32;
    let mut found = false;

    for segment in raw.split(',') {
        let segment = segment.trim();
        if !segment.contains("gpu") {
            continue;
        }
        if let Some(value) = segment.rsplit([':', '=']).next().and_then(parse_u32) {
            total += value;
            found = true;
        } else {
            total += 1;
            found = true;
        }
    }

    found.then_some(total)
}

pub fn parse_resource_map(raw: &str) -> BTreeMap<String, u64> {
    let mut values = BTreeMap::new();
    for item in raw.split(',') {
        let Some((key, value)) = item.split_once('=') else {
            continue;
        };
        if let Some(parsed) = parse_tres_value(value) {
            values.insert(key.to_string(), parsed);
        }
    }
    values
}

pub fn parse_tres_value(raw: &str) -> Option<u64> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }

    let (number, unit) = match value.chars().last() {
        Some(last) if last.is_ascii_alphabetic() => (&value[..value.len() - 1], Some(last)),
        _ => (value, None),
    };
    let parsed = number.parse::<f64>().ok()?;
    let scaled = match unit.map(|unit| unit.to_ascii_uppercase()) {
        None => parsed,
        Some('K') => parsed / 1024.0,
        Some('M') => parsed,
        Some('G') => parsed * 1024.0,
        Some('T') => parsed * 1024.0 * 1024.0,
        Some('P') => parsed * 1024.0 * 1024.0 * 1024.0,
        _ => parsed,
    };
    Some(scaled.round() as u64)
}

fn parse_history_detail_rows(stdout: &str) -> (Vec<HistoryDetailRow>, Vec<String>) {
    let mut rows = Vec::new();
    let mut notes = Vec::new();

    for (index, line) in stdout.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let fields: Vec<&str> = line.split('|').collect();
        if fields.len() != 16 {
            notes.push(format!(
                "ignored sacct detail line {} with {} fields",
                index + 1,
                fields.len()
            ));
            continue;
        }

        let job_id = fields[0].trim().to_string();
        if job_id.contains('.') {
            continue;
        }

        rows.push(HistoryDetailRow {
            job_id,
            name: fields[1].trim().to_string(),
            user: fields[2].trim().to_string(),
            account: clean_optional(fields[3]),
            partition: clean_optional(fields[4]),
            state: fields[5].trim().to_string(),
            exit_code: fields[6].trim().to_string(),
            elapsed_raw: fields[7].trim().to_string(),
            submit: clean_optional(fields[8]),
            start: clean_optional(fields[9]),
            end: clean_optional(fields[10]),
            alloc_tres: clean_optional(fields[11]),
            req_tres: clean_optional(fields[12]),
            nodes: parse_u32(fields[13]),
            n_tasks: parse_u32(fields[14]),
            node_list: clean_optional(fields[15]),
        });
    }

    (rows, notes)
}

fn parse_key_value_line(raw: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut current_key = None::<String>;
    let mut current_value = String::new();

    for token in raw.split_whitespace() {
        if let Some((key, value)) = token.split_once('=') {
            if let Some(key) = current_key.take() {
                map.insert(key, current_value.trim().to_string());
                current_value.clear();
            }
            current_key = Some(key.to_string());
            current_value.push_str(value);
        } else if current_key.is_some() {
            if !current_value.is_empty() {
                current_value.push(' ');
            }
            current_value.push_str(token);
        }
    }

    if let Some(key) = current_key {
        map.insert(key, current_value.trim().to_string());
    }

    map
}

#[derive(Debug)]
struct HistoryDetailRow {
    job_id: String,
    name: String,
    user: String,
    account: Option<String>,
    partition: Option<String>,
    state: String,
    exit_code: String,
    elapsed_raw: String,
    submit: Option<String>,
    start: Option<String>,
    end: Option<String>,
    alloc_tres: Option<String>,
    req_tres: Option<String>,
    nodes: Option<u32>,
    n_tasks: Option<u32>,
    node_list: Option<String>,
}

fn split_partitions(raw: &str) -> Vec<String> {
    let values: Vec<String> = raw
        .split(',')
        .map(normalize_partition)
        .filter(|value| !value.is_empty())
        .collect();

    if values.is_empty() {
        vec!["unknown".to_string()]
    } else {
        values
    }
}

fn normalize_partition(raw: &str) -> String {
    raw.trim().trim_end_matches('*').to_string()
}

fn normalize_node_state(raw: &str) -> String {
    raw.trim()
        .trim_end_matches(|character: char| !character.is_ascii_alphabetic())
        .to_ascii_lowercase()
}

fn parse_u32(raw: &str) -> Option<u32> {
    raw.trim().parse::<u32>().ok()
}

fn parse_u64(raw: &str) -> Option<u64> {
    raw.trim().parse::<u64>().ok()
}

fn clean_optional(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() || matches!(value, "N/A" | "(null)" | "Unknown") {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_history, parse_scontrol_node, parse_tres_value};
    use super::{parse_jobs, parse_nodes, parse_partition_specs, parse_slurm_duration};

    #[test]
    fn parses_squeue_line_with_unit_separator() {
        let input = "123\x1fmyli\x1fdemo\x1fgpu_l48\x1ftrain\x1fRUNNING\x1f1:02:03\x1f2-00:00:00\x1f1\x1f8\x1fN/A\x1f2026-04-12T12:00:00\x1f42\x1fc57b01n01";
        let (jobs, notes) = parse_jobs(input, "myli");
        assert!(notes.is_empty());
        assert_eq!(jobs.len(), 1);
        let job = &jobs[0];
        assert_eq!(job.job_id, "123");
        assert!(job.is_mine);
        assert_eq!(job.runtime_secs, Some(3723));
        assert_eq!(job.time_limit_secs, Some(172800));
        assert_eq!(job.cpus, Some(8));
    }

    #[test]
    fn parses_sinfo_node_lines_and_gpu_counts() {
        let input = "gpu_l48\x1falloc\x1fc56b01n01\x1f52\x1f515200\x1fgpu:l40:8";
        let (nodes, notes) = parse_nodes(input);
        assert!(notes.is_empty());
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].partition, "gpu_l48");
        assert_eq!(nodes[0].state, "alloc");
        assert_eq!(nodes[0].gpus, Some(8));
    }

    #[test]
    fn parses_partition_tres() {
        let input = "PartitionName=gpu_l48\nState=UP TotalCPUs=104 TotalNodes=2 TRES=cpu=104,mem=1030400M,node=2,billing=104,gres/gpu=16\n";
        let (specs, notes) = parse_partition_specs(input);
        assert!(notes.is_empty());
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name, "gpu_l48");
        assert_eq!(specs[0].total_gpus, Some(16));
    }

    #[test]
    fn parses_slurm_durations() {
        assert_eq!(parse_slurm_duration("1:02:03"), Some(3723));
        assert_eq!(parse_slurm_duration("2-00:00:00"), Some(172800));
        assert_eq!(parse_slurm_duration("0:00"), Some(0));
        assert_eq!(parse_slurm_duration("UNLIMITED"), None);
    }

    #[test]
    fn parses_sacct_history_lines() {
        let input = "687508|2FBXO11|myli|gpu_l48|RUNNING|0:0|1-20:43:19|2026-04-10T20:23:52|Unknown|billing=20,cpu=20,gres/gpu=2,mem=40000M,node=1|billing=20,cpu=20,gres/gpu=2,mem=40000M,node=1";
        let (rows, notes) = parse_history(input, "myli");
        assert!(notes.is_empty());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].alloc_gpus, Some(2));
        assert_eq!(rows[0].alloc_cpus, Some(20));
        assert!(rows[0].is_mine);
    }

    #[test]
    fn parses_tres_values_with_units() {
        assert_eq!(parse_tres_value("40000M"), Some(40000));
        assert_eq!(parse_tres_value("62.50G"), Some(64000));
        assert_eq!(parse_tres_value("1T"), Some(1024 * 1024));
    }

    #[test]
    fn degrades_when_history_tres_fields_are_missing() {
        let input = "687508|2FBXO11|myli|gpu_l48|COMPLETED|0:0|00:10:00|2026-04-10T20:23:52|2026-04-10T20:33:52||";
        let (rows, notes) = parse_history(input, "myli");
        assert!(notes.is_empty());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].alloc_tres, None);
        assert_eq!(rows[0].req_tres, None);
        assert_eq!(rows[0].alloc_cpus, None);
        assert_eq!(rows[0].alloc_gpus, None);
        assert_eq!(rows[0].resources_summary(), "n/a");
    }

    #[test]
    fn parses_scontrol_node_summary() {
        let input = "NodeName=c56b01n01 CPUAlloc=52 CPUTot=52 Gres=gpu:l40:8 State=ALLOCATED Partitions=gpu_l48 RealMemory=515200 AllocMem=104000 CfgTRES=cpu=52,mem=515200M,gres/gpu=8 AllocTRES=cpu=52,mem=104000M,gres/gpu=8";
        let (detail, notes) = parse_scontrol_node(input);
        assert!(notes.is_empty());
        let detail = detail.expect("node detail");
        assert_eq!(detail.node_name, "c56b01n01");
        assert_eq!(detail.partition.as_deref(), Some("gpu_l48"));
        assert_eq!(detail.cpu_alloc, Some(52));
        assert_eq!(detail.gpu_total, Some(8));
        assert_eq!(detail.gpu_alloc, Some(8));
    }
}
