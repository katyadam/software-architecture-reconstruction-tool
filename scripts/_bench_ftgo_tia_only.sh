#!/bin/bash
set -euo pipefail

TIA=/home/user/cargo-target/debug/test-impact
WALA=/mnt/c/Users/I749046/software-architecture-reconstruction-tool/wala-callgraph/target/wala-callgraph-all.jar
OUT=/tmp/sar-bench-out-ftgo-application
WORK=/tmp/sar-bench-ftgo-application
STRIP=/mnt/c/Users/I749046/software-architecture-reconstruction-tool/scripts/strip_manifest.py

CANDIDATE=$(git -C "$WORK" rev-parse HEAD)
BASELINE=$(git -C "$WORK" rev-parse HEAD~1)

echo "baseline:  $BASELINE"
echo "candidate: $CANDIDATE"

echo ""
echo "=== running TIA ==="
t0=$(date +%s%3N)
"$TIA" \
  --project-dir "$WORK" \
  --baseline-revision "$BASELINE" \
  --config-file "$OUT/service-config.json" \
  --config-base-dir "$WORK" \
  --output "$OUT/result.json" \
  --wala-adapter-jar "$WALA" \
  --quiet
t1=$(date +%s%3N)
echo "  TIA total: $((t1 - t0))ms"

echo ""
echo "=== result ==="
python3 -c "
import json
r = json.load(open('$OUT/result.json'))
sel   = r.get('selected_tests', [])
unsel = r.get('unselected_tests', [])
print(f'selected ({len(sel)}):')
for t in sel:
    rtype = list(t['reasons'][0].keys())[0] if t.get('reasons') else '?'
    print(f\"  [SELECTED/{rtype}] {t['test']['class_name'].split('/')[-1]}.{t['test']['method_name']}\")
print(f'unselected ({len(unsel)}):')
for t in unsel:
    print(f\"  [skipped]  {t['class_name'].split('/')[-1]}.{t['method_name']}\")
print()
print('requires_full_run:', r.get('requires_full_test_run', False))
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
sel_names   = {t['test']['method_name'] for t in r.get('selected_tests', [])}
unsel_names = {t['method_name'] for t in r.get('unselected_tests', [])}

ok = True
target = 'shouldCreateDelivery'

if target in sel_names:
    rtype = [list(t['reasons'][0].keys())[0] for t in r['selected_tests'] if t['test']['method_name']==target][0]
    print(f'PASS: {target} selected (reason: {rtype})')
    if rtype in ('call_graph', 'changed_callable'):
        print('PRECISE: call-graph reachability confirmed')
    else:
        print('CONSERVATIVE: module-level fallback (correct but imprecise)')
else:
    print(f'FAIL: {target} not selected -- FALSE NEGATIVE')
    ok = False

total = len(sel_names) + len(unsel_names)
skipped = len(unsel_names)
print(f'precision: {skipped}/{total} tests skipped ({100*skipped//total if total else 0}%)')
sys.exit(0 if ok else 1)
"
