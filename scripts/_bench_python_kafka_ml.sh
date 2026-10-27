#!/bin/bash
set -euo pipefail
# Benchmark: kafka-ml (Python, production-only change to models.py)
#
# Baseline: 5636347^ (parent of "fix: Added TELEGRAF_STR_JSON to choices at models.py")
# Candidate: 5636347  — only backend/automl/models.py changed, no test file edits
# Expected: all tests in test_views.py selected (conservative_all)
#   BUT: our discovery only finds test_*.py files, not tests.py
#   => we expect to see ~14 tests (test_views.py only), missing tests.py if any

PROJ=kafka-ml
BENCH=/mnt/c/Users/I749046/benchmarks/python/$PROJ
TIA=/home/user/cargo-target/debug/python-test-impact
OUT=/tmp/sar-bench-out-$PROJ

CANDIDATE=5636347
BASELINE=$(git -C "$BENCH" rev-parse "${CANDIDATE}^")

echo "=== benchmark: $PROJ (Python, production-only commit) ==="
echo "baseline:  $BASELINE"
echo "candidate: $CANDIDATE ($(git -C "$BENCH" log --oneline -1 $CANDIDATE))"
echo "changed files:"
git -C "$BENCH" diff --name-only "$BASELINE" "$CANDIDATE"
echo ""

rm -rf "$OUT" && mkdir -p "$OUT"

# Checkout candidate into a worktree
WT="$OUT/worktree"
git -C "$BENCH" worktree add "$WT" "$CANDIDATE" 2>&1 || true

echo "=== running Python TIA ==="
t0=$(date +%s%3N)
"$TIA" \
  --project-dir "$WT" \
  --baseline-revision "$BASELINE" \
  --output "$OUT/result.json" \
  --pyan3-timeout-secs 0 2>&1
t1=$(date +%s%3N)
echo "  TIA total: $((t1 - t0))ms"
echo ""

echo "=== result summary ==="
python3 -c "
import json
r = json.load(open('$OUT/result.json'))
sel   = r.get('selected_tests', [])
unsel = r.get('unselected_tests', [])
print(f'mode: {r[\"selection_mode\"]}  run_all: {r[\"run_all_tests\"]}')
print(f'discovered: {len(r[\"discovered_tests\"])}')
print(f'selected ({len(sel)}):')
for t in sel:
    reasons = t.get('reasons', [])
    rtype = list(reasons[0].keys())[0] if reasons else '?'
    print(f'  [{rtype}] {t[\"test\"][\"qualified_name\"]} ({t[\"test\"][\"source_path\"]})')
print(f'unselected ({len(unsel)}):')
for t in unsel:
    print(f'  [skipped] {t[\"qualified_name\"]} ({t[\"source_path\"]})')
print()
for d in r.get('diagnostics', []):
    print('  diag:', d)
"

# Cleanup worktree
git -C "$BENCH" worktree remove --force "$WT" 2>/dev/null || true
