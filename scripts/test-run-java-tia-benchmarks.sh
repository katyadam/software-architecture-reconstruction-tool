#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
help=$(bash "$repo_root/scripts/run-java-tia-benchmarks.sh" --help)

grep -Fq 'Usage: run-java-tia-benchmarks.sh' <<<"$help"
grep -Fq 'JAVA_BENCHMARK_ROOT' <<<"$help"
