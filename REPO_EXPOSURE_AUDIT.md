# Repository Exposure Audit

Date: 2026-04-14  
Repository: `Wubeizhongxinghua/s-top`  
Default branch: `main`

## Project summary

`sqtop` is a Rust TUI for Slurm clusters. It targets ordinary HPC users who need a scheduler-oriented view of partitions, queues, nodes, users, and jobs without depending on `slurmrestd` or JSON-based Slurm output.

Primary audiences:

- Slurm users on shared HPC clusters
- cluster operators who want a user-friendly queue dashboard
- Rust / TUI developers interested in terminal monitoring tools

Core value proposition:

- readable partition overview
- queue visibility across users
- structured job detail and cancel preview flows
- compatibility with ordinary Slurm user environments

## Current repository strengths

- Clear Rust packaging metadata in `Cargo.toml`
- MIT license already present
- Tagged releases and packaging workflows already exist
- Real screenshots are available under `docs/screenshots/`
- English and Chinese READMEs already exist
- Project has a concrete niche with obvious search terms: Slurm, TUI, cluster monitoring, HPC

## Current repository discoverability gaps

### README

Before this documentation pass, the README already contained useful information, but several exposure issues remained:

- the first screen did not emphasize the problem statement strongly enough
- installation and quick-start information could be surfaced earlier
- screenshots existed but were not organized as a concise entry-point gallery
- community and support entry points were missing
- there was no clear “why this exists” framing for first-time visitors

### Repository metadata

- Git remote points to GitHub: `git@github.com:Wubeizhongxinghua/s-top.git`
- The repository slug is still `s-top`, while the binary/crate/package name is `sqtop`
- No evidence of GitHub repo description, topics, homepage, or Discussions configuration was available locally
- `gh` is not installed or not available in this environment, so metadata cannot be updated automatically from here

### Community health files

Missing before this pass:

- `CONTRIBUTING.md`
- `CODE_OF_CONDUCT.md`
- `SECURITY.md`
- `SUPPORT.md`
- `CITATION.cff`
- `CHANGELOG.md`
- issue templates
- PR template

### Academic / professional reuse

- The project is software that could reasonably be cited in papers, cluster guides, or technical reports
- A `CITATION.cff` file was missing
- There was no short citation guidance, release note structure, or Zenodo recommendation

## Existing assets that can improve conversion

- `docs/screenshots/overview-hero.png`
- `docs/screenshots/my-jobs.png`
- `docs/screenshots/all-jobs.png`
- `docs/screenshots/users.png`
- `docs/screenshots/partition-detail.png`
- `docs/screenshots/node-detail.png`
- `docs/screenshots/job-detail.png`
- `docs/screenshots/cancel-preview.png`

These are sufficient for a stronger GitHub landing page without inventing visuals.

## Existing release and packaging signals

- Multiple version tags already exist through `v0.2.1`
- GitHub Actions workflows exist for:
  - GitHub release archives
  - crates.io publishing
  - Anaconda.org publishing

This is strong evidence that installation can be presented confidently.

## Homepage candidate assessment

Possible candidates:

- GitHub repository root
- GitHub Releases page
- future docs site or GitHub Pages site

Assessment:

- A dedicated external homepage is not currently evident
- Reusing the repo URL as the GitHub “homepage” setting would add little value
- Recommendation: do not set a repository homepage yet unless a dedicated docs or release landing page is created

## Topics candidate assessment

High-signal topic candidates based on current repository contents:

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

## Discussions assessment

This project is a good candidate for GitHub Discussions because it will likely attract:

- installation questions
- cluster compatibility questions
- packaging requests
- feature requests around Slurm environments

Recommendation:

- enable Discussions once the issue templates and support policy are in place

## Zenodo / citation assessment

This project is suitable for software citation:

- it has tagged releases
- it is a standalone tool
- it has a clear name, license, and installation path

Recommendation:

- add `CITATION.cff`
- consider enabling Zenodo release archiving for DOI generation

## Main exposure risks still worth addressing manually on GitHub

- repo slug mismatch (`s-top`) versus public package name (`sqtop`)
- missing repo description/topics/homepage/discussions configuration
- lack of social preview image customization
- lack of pinned-repo/profile coordination outside the repo itself

## Recommended next actions

1. Rewrite README for first-visit clarity and install conversion
2. Add community health files and templates
3. Add citation and changelog files
4. Set GitHub description and topics
5. Decide whether to keep the repo slug as `s-top` or rename it to `sqtop`
