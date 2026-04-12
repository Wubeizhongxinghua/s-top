#!/usr/bin/env bash
set -euo pipefail

cargo install --locked --path . --root "${PREFIX}"
rm -f "${PREFIX}/.crates.toml" "${PREFIX}/.crates2.json"
