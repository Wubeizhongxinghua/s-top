# Contributing to sqtop

Thank you for considering a contribution.

## Before you start

Please check existing issues and pull requests before opening a new one. For questions about usage or installation, see [SUPPORT.md](SUPPORT.md).

## Development basics

This repository uses Rust stable.

Typical local checks:

```bash
cargo fmt
cargo test
cargo build --release
```

If your change affects packaging or documentation, update the relevant files in the same pull request.

## Scope expectations

Useful contributions include:

- Slurm compatibility fixes
- UI clarity and interaction improvements
- parser hardening for real cluster output
- documentation improvements
- packaging and release polish

Avoid unrelated refactors in the same pull request unless they are required for the change.

## Pull request guidelines

- Keep the change focused
- Explain the motivation and user-visible impact
- Mention how you tested the change
- Call out any Slurm environment assumptions
- Update docs when behavior, installation, or usage changes

## Reporting bugs

The most useful bug reports include:

- `sqtop --once` output or a redacted excerpt
- `sqtop --debug-dump` output when relevant
- Slurm command versions
- terminal environment information
- screenshots if the issue is visual
- steps to reproduce

Issue templates in `.github/ISSUE_TEMPLATE/` are preferred.
