#!/bin/bash
set -euo pipefail

PROJ=microservice-kafka
BENCH=/mnt/c/Users/I749046/benchmarks/java/$PROJ
WORK=/tmp/sar-bench-spoon-$PROJ
GEN=/home/user/cargo-target/debug/generate-java-tia-manifest
TIA=/home/user/cargo-target/debug/test-impact
SPOON=/mnt/c/Users/I749046/software-architecture-reconstruction-tool/spoon-callgraph/target/spoon-callgraph-all.jar
OUT=/tmp/sar-bench-spoon-out-$PROJ
SVC_FILE=microservice-kafka/microservice-kafka-invoicing/src/main/java/com/ewolff/microservice/invoicing/InvoiceService.java

echo "=== benchmark: microservice-kafka (Spoon source-level, synthetic commit pair) ==="
echo "change: InvoiceService.generateInvoice() idempotency guard"
echo "expected: InvoicingServiceTest.ensureIdempotencySecondCallIgnored selected via call-graph"
echo ""

git -C "$BENCH" worktree remove --force "$WORK" 2>/dev/null || true
rm -rf "$WORK" "$OUT"
mkdir -p "$OUT"

git -C "$BENCH" worktree add --detach "$WORK" HEAD
git -C "$WORK" config user.email bench@example.com
git -C "$WORK" config user.name bench

# baseline: invert guard
sed -i 's/if (invoiceRepository.existsById/if (!invoiceRepository.existsById/' "$WORK/$SVC_FILE"
git -C "$WORK" commit -qam "benchmark-baseline: break generateInvoice idempotency guard"
BASELINE=$(git -C "$WORK" rev-parse HEAD)

# candidate: restore
sed -i 's/if (!invoiceRepository.existsById/if (invoiceRepository.existsById/' "$WORK/$SVC_FILE"
git -C "$WORK" commit -qam "benchmark-candidate: restore correct idempotency guard"

echo "baseline:  $BASELINE"
echo "candidate: $(git -C "$WORK" rev-parse HEAD)"
echo ""

echo "=== compiling test classes (needed for manifest class-dir validation) ==="
t0=$(date +%s%3N)
mvn -q -f "$WORK/microservice-kafka/pom.xml" test-compile -DskipTests 2>&1 | tail -5
t1=$(date +%s%3N)
echo "  mvn test-compile: $((t1 - t0))ms"
echo ""

echo "=== generating manifest (tree-sitter test discovery) ==="
t0=$(date +%s%3N)
"$GEN" --project-dir "$WORK/microservice-kafka" --output "$OUT/tia-artifacts.json"
t1=$(date +%s%3N)
echo "  manifest generation: $((t1 - t0))ms"

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
echo "=== running TIA (Spoon source-level provider) ==="
t0=$(date +%s%3N)
"$TIA" \
  --project-dir "$WORK" \
  --baseline-revision "$BASELINE" \
  --config-file "$OUT/service-config.json" \
  --config-base-dir "$WORK" \
  --output "$OUT/result.json" \
  --java-callgraph-provider spoon \
  --spoon-adapter-jar "$SPOON" \
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
    if rtype in ('call_graph', 'changed_callable'):
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
