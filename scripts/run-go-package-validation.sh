#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
export GO_BENCHMARK_CAMPAIGN=expanded
exec "$repo_root/scripts/run-go-package-selection-benchmarks.sh" "$@"
