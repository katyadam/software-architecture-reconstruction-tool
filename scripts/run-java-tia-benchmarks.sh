#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: run-java-tia-benchmarks.sh [PROJECT ...]

Runs local Java TIA checks from each project's HEAD~1 to HEAD. By default it
checks sample-message-driven-microservices and sample-spring-kafka-microservices.

Environment:
  JAVA_BENCHMARK_ROOT       Benchmark checkout root (default: sibling benchmarks/java)
  TIA_BINARY                Built test-impact binary (default: target/debug/test-impact)
  WALA_ADAPTER_JAR          WALA adapter JAR (default: wala-callgraph target JAR)
  TIA_BENCHMARK_OUTPUT_DIR  Directory for JSON results (default: a new /tmp directory)
  WALA_TIMEOUT_SECONDS      Per-module adapter timeout (default: 30)
EOF
}

if [[ ${1:-} == --help || ${1:-} == -h ]]; then
  usage
  exit 0
fi

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
git_common_dir=$(git -C "$repo_root" rev-parse --path-format=absolute --git-common-dir)
tool_root=$(dirname "$git_common_dir")
benchmark_root=${JAVA_BENCHMARK_ROOT:-"$tool_root/../benchmarks/java"}
tia_binary=${TIA_BINARY:-"$repo_root/target/debug/test-impact"}
wala_adapter_jar=${WALA_ADAPTER_JAR:-"$repo_root/wala-callgraph/target/wala-callgraph-all.jar"}
wala_timeout_seconds=${WALA_TIMEOUT_SECONDS:-30}
output_dir=${TIA_BENCHMARK_OUTPUT_DIR:-"$(mktemp -d /tmp/sar-java-tia-benchmark.XXXXXX)"}

if [[ ! -d $benchmark_root ]]; then
  echo "Benchmark root does not exist: $benchmark_root" >&2
  exit 2
fi

if [[ ! -x $tia_binary ]]; then
  cargo build -p cli --bin test-impact --manifest-path "$repo_root/Cargo.toml"
fi
if [[ ! -f $wala_adapter_jar ]]; then
  mvn -q -f "$repo_root/wala-callgraph/pom.xml" package -DskipTests
fi

mkdir -p "$output_dir"
if (($# == 0)); then
  set -- sample-message-driven-microservices sample-spring-kafka-microservices
fi

printf 'Java TIA benchmarks; results: %s\n' "$output_dir"
printf '%-44s %10s %12s %s\n' 'project' 'selected' 'unselected' 'timings'

status=0
for project_name in "$@"; do
  project_dir="$benchmark_root/$project_name"
  if [[ ! -d $project_dir/.git ]]; then
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
  if ! "$tia_binary" \
    --project-dir "$project_dir" \
    --baseline-revision "$baseline" \
    --wala-adapter-jar "$wala_adapter_jar" \
    --wala-timeout-seconds "$wala_timeout_seconds" \
    --output "$result"; then
    echo "TIA invocation failed: $project_name" >&2
    status=1
    continue
  fi
  if ! jq -e '.selected_tests | type == "array"' "$result" >/dev/null; then
    echo "TIA did not produce a valid result: $project_name" >&2
    status=1
    continue
  fi
  selected=$(jq '.selected_tests | length' "$result")
  unselected=$(jq '.unselected_tests | length' "$result")
  timings=$(jq -r '[.diagnostics[] | select(startswith("timing: "))] | join("; ")' "$result")
  printf '%-44s %10s %12s %s\n' "$project_name" "$selected" "$unselected" "$timings"
done

exit "$status"
