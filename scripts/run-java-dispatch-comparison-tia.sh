#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
run_dir=$(mktemp -d /tmp/sar-java-dispatch-comparison.XXXXXX)
baseline_repo="$run_dir/baseline"
mkdir -p "$baseline_repo"
cp -R "$repo_root/scripts/fixtures/java-dispatch/." "$baseline_repo/"
build_tool=${TIA_DISPATCH_BUILD_TOOL:-maven}
case "$build_tool" in
  maven) report_path=target/surefire-reports ;;
  gradle)
    report_path=build/test-results/test
    for module in . library app independent; do
      rm "$baseline_repo/$module/pom.xml"
    done
    ;;
  *) echo 'TIA_DISPATCH_BUILD_TOOL must be maven or gradle' >&2; exit 2 ;;
esac
test_project() {
  if [[ $build_tool == maven ]]; then
    mvn -q -fae -f "$1/pom.xml" test
  else
    gradle --continue -p "$1" test
  fi
}
git -C "$baseline_repo" init -q
git -C "$baseline_repo" add -- .
git -C "$baseline_repo" -c user.name=TIA -c user.email=tia@example.invalid commit -qm 'Baseline dispatch, reactor and injection controls'
baseline=$(git -C "$baseline_repo" rev-parse HEAD)
printf 'Comparison artifacts: %s\n' "$run_dir"
test_project "$baseline_repo" 2>&1 | tee "$run_dir/baseline-tests.log"
mkdir -p "$run_dir/baseline-reports"
cp "$baseline_repo/app/$report_path"/TEST-*.xml "$run_dir/baseline-reports/"
cp "$baseline_repo/independent/$report_path"/TEST-*.xml "$run_dir/baseline-reports/"
mvn -q -f "$repo_root/java-callgraph-adapters/pom.xml" package -DskipTests
cargo build -q -p cli --bin test-impact --manifest-path "$repo_root/Cargo.toml"

for scenario in dispatch cross-module injection; do
  candidate="$run_dir/$scenario/project"
  results="$run_dir/$scenario/results"
  mkdir -p "$results"
  git -C "$baseline_repo" worktree add -q --detach "$candidate" "$baseline"
  case "$scenario" in
    dispatch) source=app/src/main/java/control/OverriddenQuote.java; expected=interfaceDispatch ;;
    cross-module) source=library/src/main/java/control/SharedPrice.java; expected=crossModule ;;
    injection) source=app/src/main/java/control/QuoteConfiguration.java; expected=springInjection ;;
  esac
  if [[ $scenario == injection ]]; then
    perl -0pi -e 's/new InjectedQuote\(10\)/new InjectedQuote(11)/' "$candidate/$source"
  else
    perl -0pi -e 's/return 10;/return 11;/' "$candidate/$source"
  fi
  if git -C "$candidate" diff --quiet; then
    echo "Mutation was not applied: $scenario" >&2
    exit 1
  fi
  git -C "$candidate" add -- "$source"
  git -C "$candidate" -c user.name=TIA -c user.email=tia@example.invalid commit -qm "Mutate $scenario control"
  printf 'Executing mutation: %s\n' "$scenario"
  if test_project "$candidate" > "$results/candidate-tests.log" 2>&1; then
    echo "Mutation unexpectedly passed: $scenario" >&2
    exit 1
  fi
  cp "$candidate/app/$report_path"/TEST-*.xml "$results/"
  cp "$candidate/independent/$report_path"/TEST-*.xml "$results/"
  # Gradle's JUnit XML appends () to these parameterless control method names.
  python3 "$repo_root/scripts/junit-failing-methods.py" "$results"/TEST-*.xml | jq 'map(sub("\\(\\)$"; ""))' > "$results/observed-failures.json"
  if ! jq -e --arg expected "$expected" '. == [$expected]' "$results/observed-failures.json" > /dev/null; then
    echo "Unexpected mutation failures for $scenario; expected $expected:" >&2
    cat "$results/observed-failures.json" >&2
    exit 1
  fi
  # Install this candidate's library so provider preparation cannot use a stale
  # baseline dependency from another scenario's local Maven artifact.
  if [[ $build_tool == maven ]]; then
    mvn -q -f "$candidate/pom.xml" install -DskipTests > "$results/install.log" 2>&1
  fi
  for provider in sootup wala; do
    if [[ $provider == sootup ]]; then
      args=(--java-callgraph-provider sootup --java-callgraph-adapter-jar "$repo_root/sootup-callgraph/target/sootup-callgraph-all.jar")
    else
      args=(--wala-adapter-jar "$repo_root/wala-callgraph/target/wala-callgraph-all.jar")
    fi
    printf 'Analyzing %s with %s\n' "$scenario" "$provider"
    "$repo_root/target/debug/test-impact" --project-dir "$candidate" --baseline-revision "$baseline" "${args[@]}" --wala-timeout-seconds "${WALA_TIMEOUT_SECONDS:-300}" --quiet --output "$results/$provider.json" 2>&1 | tee "$results/$provider.log"
  done
  jq -n --arg scenario "$scenario" --slurpfile failures "$results/observed-failures.json" --slurpfile sootup "$results/sootup.json" --slurpfile wala "$results/wala.json" '
    def summarize($r): {
      selected: [$r.selected_tests[].test.method_name] | sort,
      unselected: [$r.unselected_tests[].method_name] | sort,
      missed_observed_failures: ($failures[0] - [$r.selected_tests[].test.method_name]),
      observed_failures_not_discovered: ($failures[0] - ([$r.selected_tests[].test.method_name] + [$r.unselected_tests[].method_name])),
      reasons: [$r.selected_tests[] | {test: .test.method_name, reasons}],
      diagnostics: $r.diagnostics
    };
    {scenario: $scenario, observed_failures: $failures[0], sootup: summarize($sootup[0]), wala: summarize($wala[0])}
  ' > "$results/comparison.json"
done
jq -s '.' "$run_dir"/dispatch/results/comparison.json "$run_dir"/cross-module/results/comparison.json "$run_dir"/injection/results/comparison.json | tee "$run_dir/comparison.json"
printf 'Comparison result: %s/comparison.json\n' "$run_dir"
