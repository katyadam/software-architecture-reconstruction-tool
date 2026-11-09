#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: run-python-tia-benchmarks.sh [PROJECT ...]

Runs local Python TIA checks from each project's HEAD~1 to HEAD. By default it
checks microservices-django-kafka.

Environment:
  PYTHON_BENCHMARK_ROOT      Benchmark checkout root (default: sibling benchmarks/python)
  PYTHON_TIA_BINARY          Built python-test-impact binary (default: target/debug/python-test-impact)
  TIA_BENCHMARK_OUTPUT_DIR   Directory for JSON results (default: a new /tmp directory)

Each project streams command output to the terminal and writes it to
<output-dir>/<project>.log.
EOF
}

if [[ ${1:-} == --help || ${1:-} == -h ]]; then
  usage
  exit 0
fi

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
git_common_dir=$(git -C "$repo_root" rev-parse --path-format=absolute --git-common-dir)
tool_root=$(dirname "$git_common_dir")
benchmark_root=${PYTHON_BENCHMARK_ROOT:-"$tool_root/../benchmarks/python"}
tia_binary=${PYTHON_TIA_BINARY:-"$repo_root/target/debug/python-test-impact"}
output_dir=${TIA_BENCHMARK_OUTPUT_DIR:-"$(mktemp -d /tmp/sar-python-tia-benchmark.XXXXXX)"}

if [[ ! -d $benchmark_root ]]; then
  echo "Benchmark root does not exist: $benchmark_root" >&2
  exit 2
fi

if [[ ! -x $tia_binary ]]; then
  cargo build -p cli --bin python-test-impact --manifest-path "$repo_root/Cargo.toml"
fi

mkdir -p "$output_dir"
if (($# == 0)); then
  set -- microservices-django-kafka
fi

printf 'Python TIA benchmarks; results: %s\n' "$output_dir"
printf '%-44s %10s %12s %s\n' 'project' 'selected' 'unselected' 'timings'

status=0
for project_name in "$@"; do
  project_dir="$benchmark_root/$project_name"
  if ! git -C "$project_dir" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo "Unknown Git benchmark project: $project_name" >&2
    status=1
    continue
  fi
  if ! baseline=$(git -C "$project_dir" rev-parse HEAD~1); then
    echo "Benchmark project has no HEAD~1 baseline: $project_name" >&2
    status=1
    continue
  fi
  result="$output_dir/$project_name.json"
  log="$output_dir/$project_name.log"
  if ! "$tia_binary" \
    --project-dir "$project_dir" \
    --baseline-revision "$baseline" \
    --output "$result" 2>&1 | tee "$log"; then
    echo "TIA invocation failed: $project_name (see $log)" >&2
    status=1
    continue
  fi
  if ! jq -e '.selected_tests | type == "array"' "$result" >/dev/null; then
    echo "TIA did not produce a valid result: $project_name (see $log)" >&2
    status=1
    continue
  fi
  selected=$(jq '.selected_tests | length' "$result")
  unselected=$(jq '.unselected_tests | length' "$result")
  timings=$(jq -r '[.diagnostics[] | select(startswith("timing: "))] | join("; ")' "$result")
  printf '%-44s %10s %12s %s\n' "$project_name" "$selected" "$unselected" "$timings"
done

exit "$status"
