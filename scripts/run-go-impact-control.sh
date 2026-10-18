#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
for tool in go cargo git jq perl; do
  command -v "$tool" >/dev/null || { echo "Required command not found: $tool" >&2; exit 1; }
done
artifact_dir="$(mktemp -d "${TMPDIR:-/tmp}/sar-go-impact-control.XXXXXX")"
source_dir="$artifact_dir/module"
echo "Go impact control; artifacts: $artifact_dir"
cp -R "$repo_root/go-callgraph/testdata/control" "$source_dir"
(
  cd "$repo_root/go-callgraph"
  GOWORK=off go build -mod=readonly -o "$artifact_dir/go-callgraph" .
)
(
  cd "$repo_root"
  cargo build -p cli --bin go-test-impact
)
cargo_target_dir="$(cd "$repo_root" && cargo metadata --no-deps --format-version 1 | jq -r '.target_directory')"
cp "$cargo_target_dir/debug/go-test-impact" "$artifact_dir/go-test-impact"
control_git() { git -C "$source_dir" -c user.name="Go TIA control" -c user.email=go-tia@example.test -c commit.gpgsign=false "$@"; }
control_git init -q
control_git add -- .
control_git commit -qm baseline
baseline_revision="$(control_git rev-parse HEAD)"
(
  cd "$source_dir"
  GOWORK=off go test -count=1 -json ./...
) >"$artifact_dir/baseline-tests.jsonl"
perl -pi -e 's/(func Changed\(\) int\s*\{ return )1( \})/${1}2$2/' "$source_dir/service.go"
control_git add -- service.go
control_git commit -qm "TIA Changed return-value mutation"
"$artifact_dir/go-test-impact" --project-dir "$source_dir" --baseline-revision "$baseline_revision" --go-callgraph-binary "$artifact_dir/go-callgraph" --output "$artifact_dir/report.json"
mutation_status=0
(
  cd "$source_dir"
  GOWORK=off go test -count=1 -json ./...
) >"$artifact_dir/mutation-tests.jsonl" || mutation_status=$?
if [[ "$mutation_status" -eq 0 ]]; then echo "Mutation unexpectedly passed" >&2; exit 1; fi
jq -s '[.[] | select(.Action == "fail" and .Test != null) | {package: .Package, test: .Test}] | unique | sort_by(.package,.test)' "$artifact_dir/mutation-tests.jsonl" >"$artifact_dir/observed-failures.json"
jq '[.graph_suggested_tests[] | {package: .test.execution_package, test: .test.method.member_name}] | unique | sort_by(.package,.test)' "$artifact_dir/report.json" >"$artifact_dir/suggested-tests.json"
jq -en --slurpfile actual "$artifact_dir/observed-failures.json" --slurpfile suggested "$artifact_dir/suggested-tests.json" '$actual[0] == $suggested[0] and ($actual[0] | length) == 5' >/dev/null
jq -e '(.discovered_tests|length)==7 and (.graph_suggested_tests|length)==5 and (.selected_tests|length)==7 and (.unselected_tests|length)==0 and .selection_mode=="conservative_all" and .run_all_tests==true and .test_inventory_complete==true and ([.selected_tests[] | select(.test.method.member_name=="TestUnrelated" or .test.method.member_name=="TestIndependent") | select(any(.reasons[]; has("conservative_fallback")))]|length)==2' "$artifact_dir/report.json" >/dev/null
echo "PASS: 7 discovered; 5 graph suggestions match 5 observed failures; 7 selected; 0 excluded."
echo "Selection remains conservative_all; run_all_tests=true."
echo "Report: $artifact_dir/report.json"
