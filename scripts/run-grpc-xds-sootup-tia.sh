#!/usr/bin/env bash
set -euo pipefail

# Evaluates SootUp on grpc-java's xDS build with the project property required to skip Android.
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
git_common_dir=$(git -C "$repo_root" rev-parse --path-format=absolute --git-common-dir)
tool_root=$(dirname "$git_common_dir")
project_name=grpc-java
benchmark_root=${JAVA_BENCHMARK_ROOT:-"$tool_root/../benchmarks/java"}
project_dir="$benchmark_root/$project_name"
output_dir=${TIA_GRPC_XDS_OUTPUT_DIR:-/tmp/sar-grpc-xds-sootup-tia}
runner=${TIA_GRPC_XDS_RUNNER:-"$repo_root/scripts/run-java-tia-benchmarks.sh"}

if ! git -C "$project_dir" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "grpc-java benchmark checkout is not a Git repository: $project_dir" >&2
  exit 2
fi

JAVA_BENCHMARK_ROOT="$benchmark_root" \
TIA_BENCHMARK_OUTPUT_DIR="$output_dir" \
WALA_TIMEOUT_SECONDS="${WALA_TIMEOUT_SECONDS:-300}" \
JAVA_CALLGRAPH_PROVIDER=sootup \
JAVA_CALLGRAPH_ADAPTER_JAR="${JAVA_CALLGRAPH_ADAPTER_JAR:-$repo_root/sootup-callgraph/target/sootup-callgraph-all.jar}" \
JAVA_CALLGRAPH_GRADLE_ARGUMENTS="${JAVA_CALLGRAPH_GRADLE_ARGUMENTS:--PskipAndroid=true}" \
"$runner" "$project_name"

printf 'Results: %s/%s.json\n' "$output_dir" "$project_name"
