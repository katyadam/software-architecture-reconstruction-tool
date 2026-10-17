#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
git_common_dir=$(git -C "$repo_root" rev-parse --path-format=absolute --git-common-dir)
tool_root=$(dirname "$git_common_dir")
project_name=kafka-microservices-with-saga
project_dir=${TIA_POSITIVE_PROJECT_DIR:-"$tool_root/../benchmarks/java/$project_name"}
worktree_root=${TIA_POSITIVE_WORKTREE_ROOT:-/tmp/sar-kafka-positive-benchmark}
worktree="$worktree_root/$project_name"
output_dir=${TIA_POSITIVE_OUTPUT_DIR:-/tmp/sar-kafka-positive-tia}
runner=${TIA_POSITIVE_RUNNER:-"$repo_root/scripts/run-java-tia-benchmarks.sh"}
source_file=order/src/main/java/com/sylleryum/order/util/OrderConverter.java

case "$worktree_root" in
  /tmp/*|"${TMPDIR%/}"/*) ;;
  *) echo "TIA_POSITIVE_WORKTREE_ROOT must be below /tmp or TMPDIR: $worktree_root" >&2; exit 2 ;;
esac
if [[ ! -d $project_dir/.git && ! -f $project_dir/.git ]]; then
  echo "Source benchmark is not a Git repository: $project_dir" >&2
  exit 2
fi

mkdir -p "$worktree_root"
git -C "$project_dir" worktree remove --force "$worktree" >/dev/null 2>&1 || true
git -C "$project_dir" worktree prune
git -C "$project_dir" worktree add --detach "$worktree" HEAD

perl -0pi -e 's/return orderKafka;/return orderKafka; \/\/ TIA positive control/' "$worktree/$source_file"
if git -C "$worktree" diff --quiet -- "$source_file"; then
  echo "Expected return statement was not found in $source_file" >&2
  exit 2
fi
git -C "$worktree" add -- "$source_file"
git -C "$worktree" -c user.name=TIA -c user.email=tia@example.invalid commit -m 'TIA positive-control OrderConverter change'

rm -rf "$output_dir"
JAVA_BENCHMARK_ROOT="$worktree_root" \
TIA_BENCHMARK_OUTPUT_DIR="$output_dir" \
WALA_TIMEOUT_SECONDS="${WALA_TIMEOUT_SECONDS:-300}" \
WALA_MAVEN_JAVA_HOME="${WALA_MAVEN_JAVA_HOME:-/Library/Java/JavaVirtualMachines/zulu-11.jdk/Contents/Home}" \
JAVA_CALLGRAPH_PROVIDER="${JAVA_CALLGRAPH_PROVIDER:-wala}" \
JAVA_CALLGRAPH_ADAPTER_JAR="${JAVA_CALLGRAPH_ADAPTER_JAR:-}" \
"$runner" "$project_name"

printf 'Results: %s/%s.json\n' "$output_dir" "$project_name"
