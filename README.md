# sqtop

[English](./README.md) | [中文](./README.zh-CN.md)

![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust)
![TUI](https://img.shields.io/badge/TUI-ratatui%20%2B%20crossterm-4c8eda)
![Platform](https://img.shields.io/badge/Platform-Linux%20%2F%20Slurm-2f855a)
![Data Path](https://img.shields.io/badge/Data%20Path-text%20Slurm%20commands-805ad5)

`sqtop` is a terminal monitor for Slurm clusters. It is designed for ordinary cluster users who need a readable, continuously refreshed view of partitions, queues, users, and jobs without relying on `slurmrestd` or Slurm JSON output.

The project was previously published as `s-top`. The executable, crate, and conda package name are now `sqtop`.

![Overview](docs/screenshots/overview-hero.png)

## Overview

The project focuses on the scheduler view rather than low-level hardware telemetry. The main questions it aims to answer are:

- which partitions are busy
- how current usage is split between the current user and other users
- which jobs are running or pending
- which users occupy the most resources
- how queue pressure changes over time

The implementation assumes a typical HPC user environment:

- no root privileges
- no dependency on `slurmrestd`
- no requirement for `squeue --json` or `sinfo --json`
- graceful degradation when a site does not expose optional Slurm fields

## Main Features

- Full-screen TUI built with `ratatui` and `crossterm`
- Periodic refresh with cancellation and command timeouts
- Partition overview with pressure, ownership split, and trend history
- My Jobs and All Jobs views with search, filtering, sorting, and horizontal navigation
- User view with per-user queue and resource summaries
- Partition and node drill-down pages
- Structured job detail modal
- Conservative single-job and bulk `scancel` workflows
- Mouse support for tabs, rows, headers, modal actions, and basic navigation

### Overview

![Overview](docs/screenshots/overview-hero.png)

### My Jobs

![My Jobs](docs/screenshots/my-jobs.png)

### All Jobs

![All Jobs](docs/screenshots/all-jobs.png)

### Users

![Users](docs/screenshots/users.png)

### Partition Detail

![Partition Detail](docs/screenshots/partition-detail.png)

### Node Detail

![Node Detail](docs/screenshots/node-detail.png)

### Job Detail

![Job Detail](docs/screenshots/job-detail.png)

### Cancel Preview

![Cancel Preview](docs/screenshots/cancel-preview.png)

## Views

### Overview

The initial page shows cluster-wide partition pressure, ownership distribution, running versus pending counts, and a rolling trend.

### My Jobs

Shows the current user's active jobs. This page is intended for day-to-day queue inspection and job operations.

### All Jobs

Shows the active queue across all visible users. Rows belonging to the current user remain visually distinct.

### Users

Provides per-user summaries for running jobs, pending jobs, total jobs, resource footprint, and dominant partitions. The lower pane shows active jobs for the selected user.

### Partition Detail

Shows a selected partition in more detail, including node-state distribution, partition-local trends, nodes, and jobs in that partition.

### Node Detail

Shows jobs on a selected node together with interactive `user`, `state`, `where`, and `why` filters.

### Job Detail

Displays a structured modal for a selected job. Fields are grouped by purpose instead of being emitted as an unstructured text block.

## Installation

### Requirements

- Linux
- Rust stable
- Slurm client commands in `PATH`
- a terminal that supports full-screen TUIs

### Build From Source

```bash
cargo build --release
```

### Install Locally

```bash
cargo install --path .
```

### Install from crates.io


```bash
cargo install sqtop
```

### Install with conda


```bash
conda install -c wubeizhongxinghua sqtop
```

If you want `conda install sqtop` to work without `-c`, add the channel once:

```bash
conda config --add channels wubeizhongxinghua
conda install sqtop
```

## Usage

### Run

```bash
./target/release/sqtop
```

### Common Options

```bash
./target/release/sqtop --interval 2
./target/release/sqtop --once
./target/release/sqtop --debug-dump
```

### CLI Options

| Flag | Description |
| --- | --- |
| `--interval <seconds>` | Refresh interval. Default: `2.0` |
| `--user <name>` | Override the identity used for Mine / Others |
| `--all` | Start on the All Jobs page |
| `--no-all-jobs` | Disable the All Jobs page |
| `--theme <auto\|dark\|light>` | Select the UI theme |
| `--advanced-resources` | Force advanced resource columns on |
| `--no-advanced-resources` | Hide advanced resource columns |
| `--debug-dump` | Print raw and parsed data, then exit |
| `--once` | Collect once, print a summary, then exit |
| `--compact` | Use a denser layout |
| `--no-color` | Disable color output |

## Keybindings and Mouse Interaction

### Keyboard

| Key | Action | Scope |
| --- | --- | --- |
| `q` | Quit | Global |
| `Tab` / `Shift-Tab` | Switch top-level pages | Global |
| `j` / `k` / Up / Down | Move selection | Lists |
| `Enter` | Open detail | Overview and job lists |
| `b` / `Esc` | Go back or close modal | Detail views and modals |
| `/` | Start live search | Global |
| `s` | Cycle sort key | Overview, Users, job lists |
| `f` | Cycle queue-state filter | Job lists |
| `m` | Toggle mine-only mode | Shared views |
| `g` | Cycle metric mode | Overview and Partition Detail |
| `p` | Pin or unpin the current partition | Overview and job views |
| `[` / `]` | Move selected node | Partition Detail |
| `n` | Open selected node | Partition Detail |
| `u` | Cycle node user filter | Node Detail |
| `w` | Edit node `where` filter | Node Detail |
| `y` | Edit node `why` filter | Node Detail |
| `c` | Clear node filters | Node Detail |
| `i` | Open job detail | Job lists |
| `x` | Cancel the selected job | Job lists |
| `X` | Preview bulk cancel | Job lists |
| `Left` / `Right` | Move the horizontal column window | Wide tables |

### Mouse

| Interaction | Result |
| --- | --- |
| Click a tab | Switch page |
| Click a row | Select row |
| Double-click a row | Open detail |
| Click a sortable header | Sort by that column |
| Click the same header again | Reverse sort direction |
| Mouse wheel | Scroll the active list |
| Click a footer action | Trigger the action |
| Click outside the job-detail modal | Close the modal |

## Data Collection and Compatibility

The fast path is based on text-oriented Slurm commands:

- `sinfo`
- `squeue`
- `scontrol show partition`
- `scontrol show node`
- `scontrol show job`

`sacct` is used only where historical or detail enrichment is appropriate, and it is not required for the main live views.

Parsing rules are intentionally conservative:

- explicit field separators are used instead of whitespace-based parsing
- every external command has a timeout
- command failures degrade the corresponding panel instead of crashing the UI
- optional fields remain optional throughout the model layer

## Project Structure

| Path | Responsibility |
| --- | --- |
| `src/collector/` | Slurm command execution, timeout handling, cancellation, raw collection |
| `src/model/` | Parsers, normalized data structures, aggregation |
| `src/app.rs` | Application state, refresh orchestration, filtering, sorting, event handling |
| `src/ui/` | Rendering, theme definitions, view composition, mouse hit testing |
| `src/cli.rs` | CLI parsing and current-user resolution |
| `src/config.rs` | Optional configuration support |
| `recipe/` | Conda recipe and build scripts |
| `.github/workflows/` | Release packaging and registry publishing automation |
| `config.example.toml` | Example configuration |

## Limitations

- Availability of `ReqTRES`, `AllocTRES`, `GRES`, memory, and GPU fields depends on site configuration
- Pending jobs eligible for multiple partitions may appear in more than one partition-level pending aggregation
- Narrow terminals still require horizontal navigation on wide tables
- Trend rendering depends on terminal font support for Unicode symbols
- Conda packaging is currently prepared for Linux `x86_64`; additional conda targets can be added later if needed
- The conda package currently targets a Linux `glibc` baseline of `2.17`

## License

This project is distributed under the MIT license. See [LICENSE](LICENSE).
