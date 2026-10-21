#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
for tool in go cargo git python3; do
  command -v "$tool" >/dev/null || { echo "Required command not found: $tool" >&2; exit 1; }
done
benchmark_root="${GO_BENCHMARK_ROOT:-$repo_root/../../../benchmarks/go}"
output_dir="${TIA_BENCHMARK_OUTPUT_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/sar-go-package-selection.XXXXXX")}"
native="${GO_CALLGRAPH_BINARY:-$repo_root/target/go-callgraph}"
cli="${GO_TEST_IMPACT_BINARY:-$repo_root/target/debug/go-test-impact}"
if [[ -z "${GO_TEST_IMPACT_BINARY:-}" ]]; then
  (cd "$repo_root" && cargo build --offline --locked -p cli --bin go-test-impact)
fi
if [[ -z "${GO_CALLGRAPH_BINARY:-}" ]]; then
  (cd "$repo_root/go-callgraph" && GOENV=off GOWORK=off GOTOOLCHAIN=local GOPROXY=off GOSUMDB=off go build -mod=readonly -o "$native" .)
fi
echo "Go package-selection controls; artifacts: $output_dir"
PYTHONDONTWRITEBYTECODE=1 python3 "$repo_root/scripts/evaluate-go-package-selection.py" --benchmark-root "$benchmark_root" --output-dir "$output_dir" --go-callgraph-binary "$native" --go-test-impact-binary "$cli" --analysis-timeout-seconds "${GO_ANALYSIS_TIMEOUT_SECONDS:-300}" --campaign "${GO_BENCHMARK_CAMPAIGN:-controls}"
