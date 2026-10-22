#!/bin/bash
set -euo pipefail
# Comprehensive Python TIA benchmark across all available repos with tests
# Runs HEAD~1..HEAD for each repo and reports discovery + selection

BENCH_ROOT=/mnt/c/Users/I749046/benchmarks/python
TIA=/home/user/cargo-target/debug/python-test-impact
OUT=/tmp/sar-python-bench-full
rm -rf "$OUT" && mkdir -p "$OUT"

run_benchmark() {
  local proj="$1"
  local dir="$BENCH_ROOT/$proj"
  local baseline candidate

  if ! baseline=$(git -C "$dir" rev-parse HEAD~1 2>/dev/null); then
    echo "$proj: only 1 commit, skipping"
    return
  fi
  candidate=$(git -C "$dir" rev-parse HEAD)

  local wt="$OUT/$proj-wt"
  git -C "$dir" worktree add "$wt" HEAD 2>/dev/null

  local result="$OUT/$proj.json"
  local log="$OUT/$proj.log"

  if ! "$TIA" \
    --project-dir "$wt" \
    --baseline-revision "$baseline" \
    --output "$result" \
    --pyan3-timeout-secs 0 >"$log" 2>&1; then
    echo "$proj: TIA FAILED (see $log)"
    git -C "$dir" worktree remove --force "$wt" 2>/dev/null || true
    return
  fi

  git -C "$dir" worktree remove --force "$wt" 2>/dev/null || true

  python3 -c "
import json, sys
proj = sys.argv[1]
r = json.load(open(sys.argv[2]))
disc = len(r.get('discovered_tests', []))
sel  = len(r.get('selected_tests', []))
unsel = len(r.get('unselected_tests', []))
mode = r.get('selection_mode', '?')
changed = len(r.get('changed_elements', []))
print(f'{proj:42s}  mode={mode:16s}  disc={disc:3d}  sel={sel:3d}  unsel={unsel:3d}  changed_elements={changed}')
" "$proj" "$result" 2>/dev/null || echo "$proj: JSON parse error"
}

echo "Running Python TIA benchmarks (HEAD~1..HEAD for each repo)"
echo ""
printf '%-42s  %-18s  %4s  %4s  %5s  %s\n' 'project' 'mode' 'disc' 'sel' 'unsel' 'changed_elems'
echo "$(printf '=%.0s' {1..90})"

for proj in FastApi-GRPC-Todo-Microservice-App Rabbitmq-showcase-python event-driven-dms kafka-ml microservices-demo microservices-django-kafka microservices-python microservices-rabbitmq-django; do
  run_benchmark "$proj"
done

echo ""
echo "Results written to: $OUT"
