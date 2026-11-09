#!/bin/bash
set -euo pipefail

# Benchmark: microservices-django-kafka (Python, real commit pair)
#
# Baseline:  0d4e412  "Fix formatting."  — only non-events files changed
# Candidate: 915e300  "Remove unnecessary function. Use dateutil.parser.isoparse."
#             — removes parse_datetime() from events/src/events/__init__.py
#
# Expected: all 9 test functions selected (conservative_all), 0 unselected.
# Python TIA has no call-graph; every source change selects the full test suite.

PROJ=microservices-django-kafka
BENCH=/mnt/c/Users/I749046/benchmarks/python/$PROJ
TIA=/home/user/cargo-target/debug/python-test-impact
OUT=/tmp/sar-bench-out-$PROJ

echo "=== benchmark: $PROJ (Python, real commit pair) ==="
echo "baseline:  0d4e412  (Fix formatting — events/*.py reformatted, no logic change)"
echo "candidate: 915e300  (Remove unnecessary function; events/__init__.py)"
echo "expected:  9 selected / 0 unselected  (conservative_all)"
echo ""

BASELINE=$(git -C "$BENCH" rev-parse 0d4e412)
CANDIDATE=$(git -C "$BENCH" rev-parse 915e300)

echo "baseline SHA:  $BASELINE"
echo "candidate SHA: $CANDIDATE"
echo ""

rm -rf "$OUT" && mkdir -p "$OUT"

echo "=== diff (candidate vs baseline) ==="
git -C "$BENCH" diff "$BASELINE" "$CANDIDATE" --stat
echo ""

echo "=== running Python TIA ==="
t0=$(date +%s%3N)
"$TIA" \
  --project-dir "$BENCH" \
  --baseline-revision "$BASELINE" \
  --output "$OUT/result.json" 2>&1
t1=$(date +%s%3N)
echo "  TIA total: $((t1 - t0))ms"
echo ""

echo "=== result ==="
python3 -c "
import json
r = json.load(open('$OUT/result.json'))
sel   = r.get('selected_tests', [])
unsel = r.get('unselected_tests', [])
print(f'mode: {r[\"selection_mode\"]}  run_all: {r[\"run_all_tests\"]}')
print(f'selected ({len(sel)}):')
for t in sel:
    reasons = t.get('reasons', [])
    rtype = list(reasons[0].keys())[0] if reasons else '?'
    print(f'  [SELECTED/{rtype}] {t[\"test\"][\"qualified_name\"]} ({t[\"test\"][\"source_path\"]})')
print(f'unselected ({len(unsel)}):')
for t in unsel:
    print(f'  [skipped]  {t[\"qualified_name\"]} ({t[\"source_path\"]})')
print()
for d in r.get('diagnostics', []):
    if d.startswith('timing:'):
        print(' ', d)
    else:
        print('diagnostic:', d)
"
echo ""

echo "=== verdict ==="
python3 -c "
import json, sys
r = json.load(open('$OUT/result.json'))
sel   = r.get('selected_tests', [])
unsel = r.get('unselected_tests', [])
total = len(sel) + len(unsel)

ok = True

if r['selection_mode'] != 'conservative_all':
    print(f'FAIL: expected conservative_all, got {r[\"selection_mode\"]}')
    ok = False
else:
    print(f'PASS: selection_mode == conservative_all')

if r['run_all_tests'] != True:
    print(f'FAIL: expected run_all_tests=true, got {r[\"run_all_tests\"]}')
    ok = False
else:
    print(f'PASS: run_all_tests == true')

if len(sel) != 9:
    print(f'FAIL: expected 9 selected tests, got {len(sel)}')
    ok = False
else:
    print(f'PASS: {len(sel)}/9 tests selected (full conservative recall)')

if len(unsel) != 0:
    print(f'FAIL: expected 0 unselected, got {len(unsel)}')
    ok = False
else:
    print(f'PASS: 0 tests unselected (no false negatives)')

print()
print(f'precision: 0/{total} tests skipped (0%) — conservative design as expected')
sys.exit(0 if ok else 1)
"
