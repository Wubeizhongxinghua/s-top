# s-top

[English](./README.md) | [中文](./README.zh-CN.md)

> A modern Slurm TUI for people who need to understand queue pressure, partition usage, user ownership, and job detail at a glance.

![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust)
![TUI](https://img.shields.io/badge/TUI-ratatui%20%2B%20crossterm-4c8eda)
![Platform](https://img.shields.io/badge/Platform-Linux%20%2F%20Slurm-2f855a)
![Mode](https://img.shields.io/badge/Data%20Path-text%20Slurm%20commands-805ad5)
![Status](https://img.shields.io/badge/Status-active%20iteration-0ea5e9)

## What It Is

`s-top` is a full-screen terminal monitor for Slurm clusters. It focuses on the scheduler view:

- which partitions are busy
- how much is mine vs other users
- who is consuming resources
- which jobs are running or pending
- how queue pressure changes over time

It is intentionally built for ordinary HPC users:

- no root access required
- no `slurmrestd`
- no dependency on `squeue --json` or `sinfo --json`
- tolerant of partial Slurm capabilities and site-specific differences

![s-top overview hero](docs/screenshots/overview-hero.png)

## Why

Classic Slurm workflows are powerful, but they fragment context:

- `squeue` is precise, but not easy to scan at cluster scale
- `sinfo` shows node state, but not ownership or queue pressure
- `watch` helps with refresh, but not with structure, search, sorting, or drill-down

`s-top` tries to fill that gap with a scheduler-centric dashboard:

- **Overview first**: see pressure, ownership, and job state distribution immediately
- **Queue-first workflow**: inspect jobs, users, partitions, and nodes without leaving the TUI
- **Operational detail**: structured job detail, safe cancel flows, and mouse interaction
- **Trend awareness**: rolling running/pending history instead of a single snapshot

## Highlights

### Cluster-wide visibility

- **Overview dashboard** with pressure bars, ownership split, and rolling running/pending trends
- **Stable partition colors** across overview, jobs, detail pages, and user summaries
- **Fast refresh path** based on lightweight Slurm text commands

### Queue analysis

- **My Jobs** and **All Jobs** pages with search, filters, sort, and horizontal view movement
- **User View** to rank active users by jobs and resources, then inspect the selected user's jobs
- **Partition Detail** and **Node Detail** for focused drill-down

### Job operations

- **Structured job detail modal** with grouped sections instead of raw text blobs
- **Single-job and bulk `scancel`** with preview and conservative ownership checks
- **Click-outside-to-close** job detail modal for smoother mouse-driven workflows

### Usability

- **Incremental search** with live filtering
- **Clickable column headers** for mouse sorting
- **Horizontal scrolling** for wide queue tables and resource-heavy views
- **Fast exit** and cancellable Slurm subprocesses

## Feature Summary

| Area | What You Get |
| --- | --- |
| Overview | Partition pressure, Mine/Other ownership, running/pending split, rolling cluster trend |
| Jobs | My jobs and all jobs, filters, sorting, resource footprint bars, horizontal view movement |
| Users | Per-user running/pending counts, resource totals, top partitions, selected-user job list |
| Partition | Pressure summary, node-state distribution, node picker, partition-local trend |
| Node | User/state/where/why filters with node-local jobs |
| Job Detail | Grouped panels for metadata, resources, scheduling, placement, and paths |
| Mouse | Tab switching, row selection, double-click open, header sorting, footer actions, click-outside close |

## Preview

> The screenshot paths below are intentionally wired into the README so you can drop images into `docs/screenshots/` later without rewriting the document.

### Overview

- partition pressure
- Mine Running / Mine Pending / Others Running / Others Pending
- rolling cluster trend

### My Jobs

- live search, filters, sort, and horizontal view movement
- full `Name` visibility via horizontal scrolling
- compact `resource footprint` for Node / CPU / GPU

![My Jobs view](docs/screenshots/my-jobs.png)

### All Jobs

- cluster-wide active queue with mine highlighting
- `Where / Why` kept ahead of `Name` for faster triage
- wide-table browsing with horizontal movement

![All Jobs view](docs/screenshots/all-jobs.png)

### Users

- per-user running / pending totals
- resource footprint ranking
- selected-user drill-down with active jobs

![Users view](docs/screenshots/users.png)

### Partition Detail

- partition-local trend
- node-state breakdown
- selected partition job table

![Partition detail](docs/screenshots/partition-detail.png)

### Node Detail

- per-node job list
- `user` / `state` / `where` / `why` interactive filters
- focused debugging view for a single node

![Node detail](docs/screenshots/node-detail.png)

### Queue Pages

- resource footprint bars with values aligned to the right of bars
- resource footprint focuses on Node / CPU / GPU
- `Where / Why` before `Name`
- `Left` / `Right` horizontal view movement for wide tables

### Job Detail

- grouped modal layout instead of plain text
- click outside to close
- keeps the underlying page state intact

![Job detail modal](docs/screenshots/job-detail.png)

### Cancel Preview

- single-job and bulk cancel flows use a separate confirmation surface
- preview clearly shows what will and will not be cancelled

![Cancel preview modal](docs/screenshots/cancel-preview.png)

### Optional GIF / Demo Clips

If you later want animated demos, these are good candidates:

- `docs/screenshots/demo-overview.gif`
- `docs/screenshots/demo-search-and-sort.gif`
- `docs/screenshots/demo-job-detail-and-cancel.gif`
- `docs/screenshots/demo-node-filtering.gif`

## Data Sources

Primary commands used by the fast path:

- `sinfo -Nh -o '%P<sep>%t<sep>%N<sep>%c<sep>%m<sep>%G'`
- `squeue -h -t PENDING,RUNNING,CONFIGURING,COMPLETING,SUSPENDED -o '%i<sep>%u<sep>%a<sep>%P<sep>%j<sep>%T<sep>%M<sep>%l<sep>%D<sep>%C<sep>%b<sep>%V<sep>%Q<sep>%R'`
- `scontrol show partition`
- `scontrol show node -o <node>`
- `squeue -h -w <node> ...`
- `scontrol show job -o <jobid>`
- `sacct -n -P -X -j <jobid> ...` for job detail enrichment

Design notes:

- `s-top` treats Slurm JSON output as optional, not required
- parsing uses an explicit separator (`\x1f`) instead of whitespace guessing
- all external commands run with timeout and cancellation
- the UI never blocks on shell commands directly

## Installation

### Requirements

- Linux
- Rust stable toolchain
- Slurm client commands available in `PATH`
- a terminal that supports fullscreen TUIs

### Build From Source

```bash
cargo build --release
```

### Install Locally

```bash
cargo install --path .
```

### Download Prebuilt Binaries

Tagged GitHub releases publish ready-to-run archives for:

- Linux `x86_64`
- macOS `x86_64`
- macOS `aarch64` / Apple Silicon
- Windows `x86_64`

Each archive includes:

- the `s-top` binary
- `README.md`
- `README.zh-CN.md`
- `config.example.toml`

### Run

```bash
./target/release/s-top
```

Override refresh interval:

```bash
cargo run --release -- --interval 2
```

Single collection summary:

```bash
./target/release/s-top --once
```

Debug dump:

```bash
./target/release/s-top --debug-dump
```

## Quick Start

1. Build the binary.
2. Run `./target/release/s-top`.
3. Start on **Overview** to see partition pressure and ownership.
4. Use `Tab` to switch to **My Jobs**, **Users**, or **All Jobs`.
5. Use `/` for live search, `s` for sort, `f` for queue-state filter, and `Enter` for detail.
6. On wide job tables, use `Left` / `Right` to move the visible column window.

## CLI

| Flag | Description |
| --- | --- |
| `--interval <seconds>` | Refresh interval, default `2.0` |
| `--user <name>` | Override the identity used for Mine / Others |
| `--all` | Start on the All Jobs page |
| `--no-all-jobs` | Disable the All Jobs page |
| `--theme <auto|dark|light>` | Theme selection |
| `--advanced-resources` | Force advanced resource columns on |
| `--no-advanced-resources` | Hide advanced resource columns |
| `--debug-dump` | Print raw + parsed data as JSON and exit |
| `--once` | Print a plain-text summary and exit |
| `--compact` | Tighter layout |
| `--no-color` | Disable colors |

## Keybindings

| Key | Action | Views |
| --- | --- | --- |
| `q` | Quit | Global |
| `Tab` / `Shift-Tab` | Switch top-level pages | Global |
| `j` / `k` / arrows | Move selection | Lists |
| `Enter` | Open detail | Overview / queue pages |
| `b` / `Esc` | Go back | Detail pages / modal |
| `/` | Start live search | Global |
| `s` | Cycle sort key | Overview / Users / queue pages |
| `f` | Cycle queue-state filter | Queue pages |
| `m` | Toggle mine-only view | Shared views |
| `g` | Cycle metric mode | Overview / Partition |
| `p` | Pin or unpin partition | Overview / queue pages |
| `[` / `]` | Move selected node | Partition Detail |
| `n` | Open selected node | Partition Detail |
| `u` | Cycle node user filter | Node Detail |
| `w` | Edit node `where` filter | Node Detail |
| `y` | Edit node `why` filter | Node Detail |
| `c` | Clear node filters | Node Detail |
| `i` | Open job detail | Queue pages |
| `x` | Cancel selected job | Queue pages |
| `X` | Preview bulk cancel | Queue pages |
| `Left` / `Right` | Move horizontal table view | Wide queue tables |

## Mouse Controls

| Interaction | Result |
| --- | --- |
| Click top tabs | Switch page |
| Click a row | Select row |
| Double-click a row | Open detail |
| Click a sortable header | Sort by that column |
| Click the same header again | Reverse sort direction |
| Wheel scroll | Move the current list |
| Click footer actions | Trigger action |
| Click outside job detail modal | Close modal |
| Click `← Overview (b)` | Return to Overview |

## Views

### Overview

The first screen is the monitoring dashboard:

- partition pressure
- Mine vs Others ownership
- running vs pending split
- rolling cluster-wide trend

### My Jobs

Focused view of the current user's active jobs with filters, sorting, resource bars, and detail.

### Users

Ranks active users and shows:

- running jobs
- pending jobs
- total jobs
- resource totals
- main partitions
- selected user's active jobs

### All Jobs

Shared queue view with highlighting for the current user's jobs.

### Partition Detail

Combines summary bars, node-state distribution, a partition-local trend, and partition jobs.

### Node Detail

Shows node-local jobs and interactive filters for:

- user
- state
- where
- why

### Job Detail

Structured modal with grouped sections:

- Basic
- Resources
- Scheduling
- Placement / Reason
- Paths / Extra

## Project Structure

| Path | Responsibility |
| --- | --- |
| `src/collector/` | Slurm command execution, timeout handling, cancellation, cached raw collection |
| `src/model/` | Parsers, normalized structs, partition aggregation, user aggregation |
| `src/app.rs` | App state, refresh orchestration, search/filter/sort state, event loop integration |
| `src/ui/` | Rendering, theme rules, widgets, modal layout, mouse hit-testing |
| `src/cli.rs` | CLI parsing and current-user resolution |
| `src/config.rs` | Optional config-file support |
| `config.example.toml` | Example configuration |

## Configuration

Optional config file:

```text
~/.config/s-top/config.toml
```

Example:

```toml
interval = 2.0
all_jobs_enabled = true
start_in_all_jobs = false
show_advanced_resources = true
compact = false
no_color = false
theme = "dark"
```

Current configuration surface is intentionally small:

- refresh interval
- user identity override
- page availability
- theme
- compact mode
- color on/off
- advanced resource columns on/off

## Notes And Troubleshooting

### No JSON support on your cluster

That is expected. `s-top` is designed around plain-text Slurm commands first.

### Queue pages feel wide

Use `Left` / `Right` to move the visible column window. `Where / Why` appears before `Name` on purpose so scheduling context stays visible sooner.

### Job detail fields are missing

Some fields depend on site-specific `scontrol` / `sacct` visibility. `s-top` degrades gracefully and shows `N/A` when the cluster does not expose a field.

### Trend dots look slightly different across terminals

The trend renderer uses Unicode dot glyphs. Most modern terminal fonts handle them well, but very old fonts may look uneven.

## Known Limitations

- `ReqTRES`, `AllocTRES`, `GRES`, memory, and GPU fields depend on cluster configuration
- multi-partition pending jobs are still counted per eligible partition in partition-level pending stats
- very narrow terminals will require horizontal view movement for queue-heavy pages
- dot-style trend rendering depends on terminal font quality
- the project currently keeps legacy history code in the tree, but history is not part of the default refresh path
- no license file is included yet; choose and add one before publishing the repository

## Roadmap

- richer node-level resource views when Slurm exposes more stable per-node totals
- deeper user drill-down beyond the current selected-user queue panel
- more reusable UI submodules as the page count grows
- optional screenshot/GIF automation for releases

## Contributing

Issues and pull requests are welcome.

Suggested local workflow:

```bash
cargo fmt
cargo test
cargo build --release
```

Please keep changes aligned with the current project direction:

- fast first paint
- non-blocking refresh
- text-command compatibility across real Slurm sites
- readable TUI over dense but ambiguous output

## License

This repository does not currently ship a `LICENSE` file. Add your preferred open-source license before publishing to GitHub.
