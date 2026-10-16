#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
git_common_dir=$(git -C "$repo_root" rev-parse --path-format=absolute --git-common-dir)
tool_root=$(dirname "$git_common_dir")
project_name=sample-message-driven-microservices
project_dir=${TIA_SOURCE_ONLY_PROJECT_DIR:-"$tool_root/../benchmarks/java/$project_name"}
candidate=${TIA_SOURCE_ONLY_CANDIDATE:-8d35afa22b2c917ac2712fb0ad76e881e601646d}
worktree_root=${TIA_SOURCE_ONLY_WORKTREE_ROOT:-/tmp/sar-java-source-only-benchmark}
worktree="$worktree_root/$project_name"
output_dir=${TIA_SOURCE_ONLY_OUTPUT_DIR:-/tmp/sar-message-source-only-tia}
runner=${TIA_SOURCE_ONLY_RUNNER:-"$repo_root/scripts/run-java-tia-benchmarks.sh"}

case "$worktree_root" in
  /tmp/*|"${TMPDIR%/}"/*) ;;
  *)
    echo "TIA_SOURCE_ONLY_WORKTREE_ROOT must be below /tmp or TMPDIR: $worktree_root" >&2
    exit 2
    ;;
esac

if [[ ! -d $project_dir/.git && ! -f $project_dir/.git ]]; then
  echo "Source benchmark is not a Git repository: $project_dir" >&2
  exit 2
fi

mkdir -p "$worktree_root"
git -C "$project_dir" worktree remove --force "$worktree" >/dev/null 2>&1 || true
git -C "$project_dir" worktree prune
git -C "$project_dir" worktree add --detach "$worktree" "$candidate^"
git -C "$project_dir" diff "$candidate^" "$candidate" -- '*.java' | git -C "$worktree" apply
if git -C "$worktree" diff --quiet; then
  echo "Candidate has no Java-source changes: $candidate" >&2
  exit 2
fi
git -C "$worktree" add -- '*.java'
git -C "$worktree" -c user.name=TIA -c user.email=tia@example.invalid commit -m 'Java-only TIA benchmark change'

rm -rf "$output_dir"
JAVA_BENCHMARK_ROOT="$worktree_root" \
TIA_BENCHMARK_OUTPUT_DIR="$output_dir" \
WALA_TIMEOUT_SECONDS="${WALA_TIMEOUT_SECONDS:-300}" \
"$runner" "$project_name"

printf 'Results: %s/%s.json\n' "$output_dir" "$project_name"
