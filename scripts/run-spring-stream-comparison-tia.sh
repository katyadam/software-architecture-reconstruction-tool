#!/usr/bin/env bash
set -euo pipefail

# One controlled Java-only diff, measured with both independent providers.
# All generated source changes live in a fresh detached temporary worktree.
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
git_common_dir=$(git -C "$repo_root" rev-parse --path-format=absolute --git-common-dir)
tool_root=$(dirname "$git_common_dir")
project_dir=${TIA_SPRING_PROJECT_DIR:-"$tool_root/../benchmarks/java/sample-spring-cloud-stream-kafka"}
run_dir=$(mktemp -d /tmp/sar-spring-stream-comparison.XXXXXX)
worktree="$run_dir/project"
result_dir="$run_dir/results"
mkdir -p "$result_dir"
printf 'Comparison artifacts: %s\n' "$run_dir"
git -C "$project_dir" worktree add --detach "$worktree" HEAD

package_path=pl/piomin/samples/kafka/order
module="$worktree/order-service"
cp "$repo_root/scripts/fixtures/spring-stream-impact/OrderImpactFacade.java" "$module/src/main/java/$package_path/"
cp "$repo_root/scripts/fixtures/spring-stream-impact/OrderImpactControlTest.java" "$module/src/test/java/$package_path/"
git -C "$worktree" add -- "order-service/src/main/java/$package_path/OrderImpactFacade.java" "order-service/src/test/java/$package_path/OrderImpactControlTest.java"
git -C "$worktree" -c user.name=TIA -c user.email=tia@example.invalid commit -m 'Add direct, transitive and unrelated baseline controls'
baseline=$(git -C "$worktree" rev-parse HEAD)

mvn -q -f "$module/pom.xml" -Dtest=OrderImpactControlTest test 2>&1 | tee "$result_dir/baseline-tests.log"
report="target/surefire-reports/TEST-pl.piomin.samples.kafka.order.OrderImpactControlTest.xml"
cp "$module/$report" "$result_dir/baseline-tests.xml"

source_file="order-service/src/main/java/$package_path/model/Order.java"
perl -0pi -e 's/return amount;/return amount + 1;/' "$worktree/$source_file"
if git -C "$worktree" diff --quiet -- "$source_file"; then
  echo 'Expected Order.getAmount return statement was not found' >&2
  exit 1
fi
git -C "$worktree" add -- "$source_file"
git -C "$worktree" -c user.name=TIA -c user.email=tia@example.invalid commit -m 'Mutate Order.getAmount for provider comparison'
if mvn -q -f "$module/pom.xml" -Dtest=OrderImpactControlTest test > "$result_dir/candidate-tests.log" 2>&1; then
  echo 'Mutation unexpectedly passed all control tests' >&2
  exit 1
fi
cp "$module/$report" "$result_dir/candidate-tests.xml"
python3 "$repo_root/scripts/junit-failing-methods.py" "$result_dir/candidate-tests.xml" > "$result_dir/actual-failures.json"
jq -e '. == ["readsAmount", "readsAmountThroughFacade"]' "$result_dir/actual-failures.json" > /dev/null
printf 'Observed mutation failures: readsAmount, readsAmountThroughFacade\n'

# Build from the reactor so the shaded adapters contain current shared support.
mvn -q -f "$repo_root/java-callgraph-adapters/pom.xml" package -DskipTests
cargo build -q -p cli --bin test-impact --manifest-path "$repo_root/Cargo.toml"
for provider in sootup wala; do
  if [[ $provider == sootup ]]; then
    adapter_args=(--java-callgraph-provider sootup --java-callgraph-adapter-jar "$repo_root/sootup-callgraph/target/sootup-callgraph-all.jar")
  else
    adapter_args=(--wala-adapter-jar "$repo_root/wala-callgraph/target/wala-callgraph-all.jar")
  fi
  printf 'Running %s on the same candidate and baseline\n' "$provider"
  "$repo_root/target/debug/test-impact" --project-dir "$worktree" --baseline-revision "$baseline" "${adapter_args[@]}" --wala-timeout-seconds "${WALA_TIMEOUT_SECONDS:-300}" --quiet --output "$result_dir/$provider.json" 2>&1 | tee "$result_dir/$provider.log"
done
jq -n --slurpfile sootup "$result_dir/sootup.json" --slurpfile wala "$result_dir/wala.json" --slurpfile failures "$result_dir/actual-failures.json" '
  def summarize($r): {
    selected: [$r.selected_tests[].test.method_name] | sort,
    unselected: [$r.unselected_tests[].method_name] | sort,
    reasons: [$r.selected_tests[] | {test: .test.method_name, reasons}],
    diagnostics: $r.diagnostics,
    all_observed_failures_selected: (($failures[0] - [$r.selected_tests[].test.method_name]) | length == 0),
    unrelated_control_excluded: ([$r.unselected_tests[].method_name] | index("readsId") != null)
  };
  {observed_failures: $failures[0], sootup: summarize($sootup[0]), wala: summarize($wala[0])}
' | tee "$result_dir/comparison.json"
printf 'Comparison result: %s/comparison.json\n' "$result_dir"
