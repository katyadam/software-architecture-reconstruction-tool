#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
for tool in go cargo; do
  command -v "$tool" >/dev/null || { echo "Required command not found: $tool" >&2; exit 1; }
done
artifact_dir="$(mktemp -d "${TMPDIR:-/tmp}/sar-go-native-adapter.XXXXXX")"
echo "Go native Rust adapter control; artifacts: $artifact_dir"
(
  cd "$repo_root/go-callgraph"
  GOWORK=off go build -mod=readonly -o "$artifact_dir/go-callgraph" .
)
cd "$repo_root"
cargo test -p extractor-runtime --test go_native_provider
GO_CALLGRAPH_TEST_BINARY="$artifact_dir/go-callgraph" cargo test -p extractor-runtime --test go_native_provider real_go_executable_preserves_seven_tests_and_five_reachable_roots -- --ignored --nocapture
echo "PASS: Rust retains native Go tests and scope, and refuses unsafe exclusion."
