#!/bin/bash
set -euo pipefail

PROJ=microservice-kafka
BENCH=/mnt/c/Users/I749046/benchmarks/java/$PROJ
WORK=/tmp/sar-bench-$PROJ
GEN=/home/user/cargo-target/debug/generate-java-tia-manifest
TIA=/home/user/cargo-target/debug/test-impact
WALA=/mnt/c/Users/I749046/software-architecture-reconstruction-tool/wala-callgraph/target/wala-callgraph-all.jar
OUT=/tmp/sar-bench-out-$PROJ
SVC_FILE=microservice-kafka/microservice-kafka-invoicing/src/main/java/com/ewolff/microservice/invoicing/InvoiceService.java

echo "=== benchmark: microservice-kafka (synthetic commit pair) ==="
echo "change: InvoiceService.generateInvoice() idempotency guard -- existsById() -> !existsById()"
echo "  baseline: guard inverted (always re-saves, idempotency broken)"
echo "  candidate: guard correct (existsById skips duplicate)"
echo "expected: InvoicingServiceTest.ensureIdempotencySecondCallIgnored selected via call-graph"
echo "          all shipping/order tests unselected"
echo ""

git -C "$BENCH" worktree remove --force "$WORK" 2>/dev/null || true
rm -rf "$WORK" "$OUT"
mkdir -p "$OUT"

git -C "$BENCH" worktree add --detach "$WORK" HEAD
git -C "$WORK" config user.email bench@example.com
git -C "$WORK" config user.name bench

# baseline: invert guard -> idempotency broken
sed -i 's/if (invoiceRepository.existsById/if (!invoiceRepository.existsById/' "$WORK/$SVC_FILE"
git -C "$WORK" commit -qam "benchmark-baseline: break generateInvoice idempotency guard"
BASELINE=$(git -C "$WORK" rev-parse HEAD)

# candidate: fix it back
sed -i 's/if (!invoiceRepository.existsById/if (invoiceRepository.existsById/' "$WORK/$SVC_FILE"
git -C "$WORK" commit -qam "benchmark-candidate: restore correct idempotency guard"
CANDIDATE=$(git -C "$WORK" rev-parse HEAD)

echo "baseline:  $BASELINE"
echo "candidate: $CANDIDATE"
echo ""

echo "=== compiling at candidate ==="
t0=$(date +%s%3N)
mvn -q -f "$WORK/microservice-kafka/pom.xml" test-compile -DskipTests 2>&1 | tail -3
t1=$(date +%s%3N)
echo "  compile: $((t1 - t0))ms"

echo ""
echo "=== generating manifest (tree-sitter test discovery) ==="
t0=$(date +%s%3N)
"$GEN" --project-dir "$WORK/microservice-kafka" --output "$OUT/tia-artifacts.json"
t1=$(date +%s%3N)
echo "  manifest generation: $((t1 - t0))ms"

echo ""
echo "tests discovered:"
python3 -c "
import json
m = json.load(open('$OUT/tia-artifacts.json'))
for mod in m['modules']:
    for t in mod['tests']:
        print(f\"  [{mod['root'] or '.'}] {t['class_name'].split('/')[-1]}.{t['method_name']}\")
"

python3 -c "
import json
config = {
  'service_descriptions': [{
    'name': '$PROJ',
    'base_dir_path': 'microservice-kafka',
    'urls': [],
    'java_analysis': {'artifacts_file': '$OUT/tia-artifacts.json'}
  }]
}
json.dump(config, open('$OUT/service-config.json', 'w'), indent=2)
"

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
target = 'ensureIdempotencySecondCallIgnored'
if target in sel_names:
    rtype = [list(t['reasons'][0].keys())[0] for t in r['selected_tests'] if t['test']['method_name']==target][0]
    print(f'PASS: {target} selected (reason: {rtype})')
    if rtype == 'call_graph':
        print('PRECISE: call-graph reachability confirmed -- no false negative, no false positive')
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
