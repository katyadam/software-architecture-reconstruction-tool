#!/usr/bin/env bash
set -euo pipefail

# No benchmark checkout is modified. Keep artifacts for inspection.
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
for tool in go jq perl; do
  command -v "$tool" >/dev/null || { echo "Required command not found: $tool" >&2; exit 1; }
done
result_dir="$(mktemp -d "${TMPDIR:-/tmp}/sar-go-callgraph-control.XXXXXX")"
echo "Go native call-graph control; results: $result_dir"
cp -R "$repo_root/go-callgraph/testdata/control" "$result_dir/source"
(
  cd "$repo_root/go-callgraph"
  GOWORK=off go build -mod=readonly -o "$result_dir/go-callgraph" .
)
(
  cd "$result_dir/source"
  GOWORK=off go test -count=1 -json ./...
) >"$result_dir/baseline-tests.jsonl"
"$result_dir/go-callgraph" --source-dir "$result_dir/source" --target 'example.com/tia#Changed' --target 'example.com/tia#NeverCalled' --target 'example.com/tia#Initialize' >"$result_dir/callgraph.json"

# Controlled mutation: only Changed's returned value changes.
perl -pi -e 's/(func Changed\(\) int\s*\{ return )1( \})/${1}2$2/' "$result_dir/source/service.go"
mutation_status=0
(
  cd "$result_dir/source"
  GOWORK=off go test -count=1 -json ./...
) >"$result_dir/mutation-tests.jsonl" || mutation_status=$?
if [[ "$mutation_status" -eq 0 ]]; then
  echo "Mutation unexpectedly passed" >&2
  exit 1
fi
jq -s '[.[] | select(.Action == "fail" and .Test != null) | {package: .Package, test: .Test}] | unique | sort_by(.package, .test)' "$result_dir/mutation-tests.jsonl" >"$result_dir/observed-failures.json"
# For this fixture, external tests belong to example.com/tia_test in SSA,
# but go test reports their execution under example.com/tia.
jq '[.reachability[] | select(.method.member_name == "Changed") | .entrypoints[] | {package: (if .declaring_type == "example.com/tia_test" then "example.com/tia" else .declaring_type end), test: .member_name}] | unique | sort_by(.package, .test)' "$result_dir/callgraph.json" >"$result_dir/reachable-tests.json"
jq -en --slurpfile actual "$result_dir/observed-failures.json" --slurpfile predicted "$result_dir/reachable-tests.json" '$actual[0] == $predicted[0] and ($actual[0] | length) == 5' >/dev/null
jq -e '(.tests | length) == 7 and ([.reachability[] | select(.method.member_name == "NeverCalled") | .entrypoints | length] == [0]) and ([.reachability[] | select(.method.member_name == "Initialize") | .entrypoints | length] == [6])' "$result_dir/callgraph.json" >/dev/null
echo "PASS: 5 reachable tests match 5 observed failures; 2 unrelated tests excluded."
echo "PASS: initialization reaches 6 importing tests; an uncalled target reaches none."
echo "Graph proof only: dependency bodies are opaque; production TIA is not integrated."
echo "Results: $result_dir/callgraph.json"
