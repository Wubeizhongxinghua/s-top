# Changelog

This project follows a simple release-oriented changelog. For packaging artifacts and binary downloads, see the GitHub Releases page.

## [0.2.3] - 2026-04-15

### Fixed

- Included `resv` and other non-standard node states in the partition detail node-state distribution instead of dropping them from the UI
- Kept partition node-state summaries and search text aligned with the expanded state distribution
- Disabled `o` and `e` log-viewer shortcuts while Job Detail is still loading, so stdout/stderr tailing only becomes available after the detail payload and file paths are ready

## [0.2.2] - 2026-04-14

### Fixed

- Made the Job Detail log viewer track carriage-return progress output more like `tail -f`
- Added live rendering for progress-style updates that rewrite the current line without waiting for `\n`
- Preserved normal newline log handling while improving follow-mode feedback for stdout and stderr viewers

## [0.2.1] - 2026-04-14

### Changed

- Synced package metadata for the next crates.io and conda publication cycle
- Aligned release versioning with the current `sqtop` package version

## [0.2.0] - 2026-04-14

### Changed

- Refined detail modal layout, visual hierarchy, and cancel-flow presentation
- Improved detail-page table geometry and semantic color separation

## [0.1.0] - 2026-04-12

### Added

- Initial public release of the Slurm TUI monitor
