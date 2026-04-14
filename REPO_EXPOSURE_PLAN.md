# Repository Exposure Plan

Date: 2026-04-14  
Repository: `Wubeizhongxinghua/s-top`

## Recommended GitHub repo description

Candidate 1:

`Interactive terminal monitor for Slurm clusters, with partition pressure, queue views, user summaries, and structured job inspection.`

Candidate 2:

`Scheduler-focused Slurm TUI for partitions, jobs, users, and nodes. Works in ordinary HPC user environments without Slurm JSON APIs.`

Candidate 3:

`Rust TUI for Slurm queue monitoring: partition overview, job drill-down, user summaries, and safe cancel previews for shared HPC clusters.`

Recommended default:

`Scheduler-focused Slurm TUI for partitions, jobs, users, and nodes. Works in ordinary HPC user environments without Slurm JSON APIs.`

## Recommended topics

Priority order:

1. `slurm`
2. `hpc`
3. `tui`
4. `rust`
5. `ratatui`
6. `cluster-monitoring`
7. `terminal-ui`
8. `scheduler`
9. `queue-monitoring`
10. `squeue`
11. `sinfo`
12. `job-monitoring`

## Homepage recommendation

Recommendation: do not set a GitHub repository homepage yet.

Reason:

- no dedicated docs site or landing page is present
- pointing GitHub’s homepage field back to the repository adds little value

Alternative once ready:

- GitHub Pages docs
- project documentation site
- a release landing page if a dedicated docs domain is introduced

## Suggested social preview concept

### Goal

Make the project understandable in one glance when the repository link is shared in Slack, WeChat, X, or email.

### Visual composition

- Background: cropped `docs/screenshots/overview-hero.png`
- Title text: `sqtop`
- Subtitle: `A Slurm TUI for partitions, queues, users, and jobs`
- Supporting line: `Readable queue monitoring for ordinary HPC users`
- Keep the layout dark, minimal, and terminal-like

### Text on image

Recommended text:

- `sqtop`
- `Interactive Slurm cluster monitor`

Avoid:

- long bullet lists
- installation commands
- tiny unreadable screenshots

## Profile pin suggestion

Short pin text:

`sqtop: a Rust TUI for Slurm clusters that makes partitions, queues, users, and jobs readable from the terminal.`

## Release title template

Template:

`vX.Y.Z: <short release focus>`

Examples:

- `v0.2.1: packaging and registry release fix`
- `v0.2.0: detail modal and cancel-flow polish`

## Release notes structure

Recommended structure:

1. `Highlights`
2. `Installation`
3. `UI and workflow changes`
4. `Packaging / compatibility`
5. `Notes for cluster environments`

Suggested release notes skeleton:

```md
## Highlights

- ...

## Installation

- `cargo install sqtop`
- `conda install -c wubeizhongxinghua sqtop`

## UI and workflow changes

- ...

## Packaging and compatibility

- ...

## Notes

- ...
```

## Zenodo DOI recommendation

Recommended steps:

1. Sign in to <https://zenodo.org> with GitHub
2. Enable the repository in Zenodo’s GitHub integration
3. Create the next GitHub release
4. Let Zenodo archive that release automatically
5. Add the generated DOI badge to the README only after it exists

## Social sharing blurb (100–150 Chinese characters)

`sqtop 是一个面向 Slurm 集群的终端监控工具，重点解决分区压力、队列占用、用户资源使用和任务详情查看的问题。它不依赖 slurmrestd，也不要求 JSON 输出，适合普通 HPC 用户直接在登录节点使用。`

## “Why star” one-liner

`Star this repository if you want a readable Slurm queue dashboard that works in ordinary HPC user environments.`

## English short pitch

`sqtop is a scheduler-focused Slurm TUI for users who need a clearer view of partitions, jobs, users, and nodes without relying on Slurm JSON APIs.`

## GitHub metadata commands

The following commands were the planned low-risk metadata updates. They are kept here as a reusable record.

Replace the description with your preferred candidate and run:

```bash
gh repo edit Wubeizhongxinghua/s-top \
  --description "Scheduler-focused Slurm TUI for partitions, jobs, users, and nodes. Works in ordinary HPC user environments without Slurm JSON APIs."
```

Add topics:

```bash
gh repo edit Wubeizhongxinghua/s-top \
  --add-topic slurm \
  --add-topic hpc \
  --add-topic tui \
  --add-topic rust \
  --add-topic ratatui \
  --add-topic cluster-monitoring \
  --add-topic terminal-ui \
  --add-topic scheduler \
  --add-topic queue-monitoring \
  --add-topic squeue \
  --add-topic sinfo \
  --add-topic job-monitoring
```

Enable Discussions:

```bash
gh repo edit Wubeizhongxinghua/s-top --enable-discussions
```

Optional future homepage update once a docs site exists:

```bash
gh repo edit Wubeizhongxinghua/s-top --homepage "https://<your-docs-site>"
```

## Execution status

- Automatic GitHub metadata update: completed partially
- Executed changes:
  - updated repository description
  - added recommended topics
  - enabled GitHub Discussions
- Not changed automatically:
  - homepage URL remains unset by design
- Current environment status:
  - `gh` is installed
  - `gh` is authenticated for `github.com`


## Applied metadata

- Description:
  - `Scheduler-focused Slurm TUI for partitions, jobs, users, and nodes. Works in ordinary HPC user environments without Slurm JSON APIs.`
- Homepage:
  - left unset intentionally
- Discussions:
  - enabled
- Topics:
  - `slurm`
  - `hpc`
  - `tui`
  - `rust`
  - `ratatui`
  - `cluster-monitoring`
  - `terminal-ui`
  - `scheduler`
  - `queue-monitoring`
  - `squeue`
  - `sinfo`
  - `job-monitoring`
