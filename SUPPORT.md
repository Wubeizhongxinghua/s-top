# Support

## Where to ask for help

Use GitHub issues for:

- reproducible bugs
- packaging problems
- installation failures
- regressions after an upgrade

Use feature requests for:

- new views
- collector improvements
- additional Slurm compatibility
- documentation improvements

## Before opening an issue

Please collect as much of the following as you can:

- `sqtop --once`
- `sqtop --debug-dump` when relevant
- `squeue --version`
- `sinfo --version` or Slurm version details
- terminal type and size
- operating system details
- screenshots for rendering issues

If your cluster output contains sensitive names, paths, or account details, redact them before posting.

## What to include

- what you expected to happen
- what actually happened
- whether the problem is reproducible
- a minimal set of commands or steps to reproduce it

## Security-sensitive issues

Do not open a public issue for vulnerabilities. See [SECURITY.md](SECURITY.md).
