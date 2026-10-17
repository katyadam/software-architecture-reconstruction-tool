#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
help=$(bash "$repo_root/scripts/run-java-tia-benchmarks.sh" --help)

grep -Fq 'Usage: run-java-tia-benchmarks.sh' <<<"$help"
grep -Fq 'JAVA_BENCHMARK_ROOT' <<<"$help"

temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT
repo="$temp_dir/repo"
benchmark_root="$temp_dir/benchmarks"
fake_tia="$temp_dir/test-impact"

git init -q "$repo"
git -C "$repo" config user.email test@example.com
git -C "$repo" config user.name test
touch "$repo/first"
git -C "$repo" add first
git -C "$repo" commit -qm first
touch "$repo/second"
git -C "$repo" add second
git -C "$repo" commit -qm second
mkdir -p "$benchmark_root"
git -C "$repo" worktree add -q --detach "$benchmark_root/linked-worktree" HEAD

cat >"$fake_tia" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
while (($#)); do
  if [[ $1 == --output ]]; then
    printf '{"selected_tests":[],"unselected_tests":[],"diagnostics":[]}' >"$2"
    exit 0
  fi
  shift
done
exit 1
EOF
chmod +x "$fake_tia"

JAVA_BENCHMARK_ROOT="$benchmark_root" \
TIA_BINARY="$fake_tia" \
WALA_ADAPTER_JAR="$temp_dir/unused.jar" \
TIA_BENCHMARK_OUTPUT_DIR="$temp_dir/results" \
bash "$repo_root/scripts/run-java-tia-benchmarks.sh" linked-worktree

source_repo="$temp_dir/source-repo"
source_worktrees="$temp_dir/source-worktrees"
source_output="$temp_dir/source-output"
fake_runner="$temp_dir/fake-runner"
git init -q "$source_repo"
git -C "$source_repo" config user.email test@example.com
git -C "$source_repo" config user.name test
mkdir -p "$source_repo/src/main/java/example"
printf 'class Example {}\n' >"$source_repo/src/main/java/example/Example.java"
git -C "$source_repo" add .
git -C "$source_repo" commit -qm base
base_commit=$(git -C "$source_repo" rev-parse HEAD)
printf 'class Example { int changed; }\n' >"$source_repo/src/main/java/example/Example.java"
printf 'documentation\n' >"$source_repo/readme.md"
git -C "$source_repo" add .
git -C "$source_repo" commit -qm candidate
candidate_commit=$(git -C "$source_repo" rev-parse HEAD)

cat >"$fake_runner" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
test "${JAVA_BENCHMARK_ROOT:?}" = "${EXPECTED_BENCHMARK_ROOT:?}"
test "${WALA_TIMEOUT_SECONDS:?}" = 300
if [[ -n ${TIA_EXPECTED_PROVIDER:-} ]]; then
  test "${JAVA_CALLGRAPH_PROVIDER:?}" = "$TIA_EXPECTED_PROVIDER"
  test "${JAVA_CALLGRAPH_ADAPTER_JAR:?}" = "${TIA_EXPECTED_ADAPTER_JAR:?}"
fi
if [[ -n ${TIA_EXPECTED_GRADLE_ARGUMENTS:-} ]]; then
  test "${JAVA_CALLGRAPH_GRADLE_ARGUMENTS:?}" = "$TIA_EXPECTED_GRADLE_ARGUMENTS"
fi
if [[ -n ${TIA_EXPECTED_MAVEN_JAVA_HOME:-} ]]; then
  test "${WALA_MAVEN_JAVA_HOME:?}" = "$TIA_EXPECTED_MAVEN_JAVA_HOME"
fi
project="${TIA_EXPECTED_PROJECT:-sample-message-driven-microservices}"
test "$(git -C "$JAVA_BENCHMARK_ROOT/$project" diff --name-only HEAD~1 HEAD)" = "${TIA_EXPECTED_CHANGED_PATH:-src/main/java/example/Example.java}"
mkdir -p "$TIA_BENCHMARK_OUTPUT_DIR"
printf '{"selected_tests":[],"unselected_tests":[],"diagnostics":[]}' >"$TIA_BENCHMARK_OUTPUT_DIR/$project.json"
EOF
chmod +x "$fake_runner"

EXPECTED_BENCHMARK_ROOT="$source_worktrees" \
TIA_SOURCE_ONLY_PROJECT_DIR="$source_repo" \
TIA_SOURCE_ONLY_CANDIDATE="$candidate_commit" \
TIA_SOURCE_ONLY_WORKTREE_ROOT="$source_worktrees" \
TIA_SOURCE_ONLY_OUTPUT_DIR="$source_output" \
TIA_SOURCE_ONLY_RUNNER="$fake_runner" \
bash "$repo_root/scripts/run-message-driven-source-only-tia.sh"

rm -rf "$source_worktrees/sample-message-driven-microservices"
EXPECTED_BENCHMARK_ROOT="$source_worktrees" \
TIA_SOURCE_ONLY_PROJECT_DIR="$source_repo" \
TIA_SOURCE_ONLY_CANDIDATE="$candidate_commit" \
TIA_SOURCE_ONLY_WORKTREE_ROOT="$source_worktrees" \
TIA_SOURCE_ONLY_OUTPUT_DIR="$source_output" \
TIA_SOURCE_ONLY_RUNNER="$fake_runner" \
bash "$repo_root/scripts/run-message-driven-source-only-tia.sh"

test "$(git -C "$source_repo" rev-parse "$candidate_commit^")" = "$base_commit"

positive_repo="$temp_dir/positive-repo"
positive_worktrees="$temp_dir/positive-worktrees"
positive_output="$temp_dir/positive-output"
git init -q "$positive_repo"
git -C "$positive_repo" config user.email test@example.com
git -C "$positive_repo" config user.name test
mkdir -p "$positive_repo/order/src/main/java/com/sylleryum/order/util"
printf 'class OrderConverter { Object convert() { return orderKafka; } }\n' >"$positive_repo/order/src/main/java/com/sylleryum/order/util/OrderConverter.java"
git -C "$positive_repo" add .
git -C "$positive_repo" commit -qm base

EXPECTED_BENCHMARK_ROOT="$positive_worktrees" \
TIA_EXPECTED_PROJECT=kafka-microservices-with-saga \
TIA_EXPECTED_CHANGED_PATH=order/src/main/java/com/sylleryum/order/util/OrderConverter.java \
TIA_EXPECTED_MAVEN_JAVA_HOME=/Library/Java/JavaVirtualMachines/zulu-11.jdk/Contents/Home \
TIA_POSITIVE_PROJECT_DIR="$positive_repo" \
TIA_POSITIVE_WORKTREE_ROOT="$positive_worktrees" \
TIA_POSITIVE_OUTPUT_DIR="$positive_output" \
TIA_POSITIVE_RUNNER="$fake_runner" \
bash "$repo_root/scripts/run-kafka-positive-tia.sh"

TIA_EXPECTED_PROVIDER=sootup \
TIA_EXPECTED_ADAPTER_JAR="$temp_dir/sootup.jar" \
EXPECTED_BENCHMARK_ROOT="$positive_worktrees" \
TIA_EXPECTED_PROJECT=kafka-microservices-with-saga \
TIA_EXPECTED_CHANGED_PATH=order/src/main/java/com/sylleryum/order/util/OrderConverter.java \
TIA_POSITIVE_PROJECT_DIR="$positive_repo" \
TIA_POSITIVE_WORKTREE_ROOT="$positive_worktrees" \
TIA_POSITIVE_OUTPUT_DIR="$positive_output" \
TIA_POSITIVE_RUNNER="$fake_runner" \
JAVA_CALLGRAPH_ADAPTER_JAR="$temp_dir/sootup.jar" \
bash "$repo_root/scripts/run-kafka-positive-sootup-tia.sh"

grpc_repo="$temp_dir/grpc-java"
git init -q "$grpc_repo"
git -C "$grpc_repo" config user.email test@example.com
git -C "$grpc_repo" config user.name test
touch "$grpc_repo/base"
git -C "$grpc_repo" add base
git -C "$grpc_repo" commit -qm base
touch "$grpc_repo/candidate"
git -C "$grpc_repo" add candidate
git -C "$grpc_repo" commit -qm candidate

TIA_EXPECTED_PROVIDER=sootup \
TIA_EXPECTED_ADAPTER_JAR="$temp_dir/sootup.jar" \
TIA_EXPECTED_GRADLE_ARGUMENTS=-PskipAndroid=true \
EXPECTED_BENCHMARK_ROOT="$temp_dir" \
TIA_EXPECTED_PROJECT=grpc-java \
TIA_EXPECTED_CHANGED_PATH=candidate \
TIA_GRPC_XDS_OUTPUT_DIR="$temp_dir/grpc-output" \
TIA_GRPC_XDS_RUNNER="$fake_runner" \
JAVA_BENCHMARK_ROOT="$temp_dir" \
JAVA_CALLGRAPH_ADAPTER_JAR="$temp_dir/sootup.jar" \
bash "$repo_root/scripts/run-grpc-xds-sootup-tia.sh"
