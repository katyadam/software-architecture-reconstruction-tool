#!/bin/bash
set -euo pipefail

PROJ=eventuate-tram-sagas-examples-customers-and-orders
BENCH=/mnt/c/Users/I749046/benchmarks/java/$PROJ
WORK=/tmp/sar-bench-$PROJ
GEN=/home/user/cargo-target/debug/generate-java-tia-manifest
TIA=/home/user/cargo-target/debug/test-impact
WALA=/mnt/c/Users/I749046/software-architecture-reconstruction-tool/wala-callgraph/target/wala-callgraph-all.jar
OUT=/tmp/sar-bench-out-$PROJ
STRIP=/mnt/c/Users/I749046/software-architecture-reconstruction-tool/scripts/strip_manifest.py

# Change in order-service-sagas (same module as CreateOrderSagaTest) to enable WALA analysis.
# CreateOrderSagaTest.shouldRejectCreateOrder() calls data.getRejectionReason() directly.
SAGA_DATA=order-service/order-service-sagas/src/main/java/io/eventuate/examples/tram/sagas/customersandorders/orders/sagas/CreateOrderSagaData.java

echo "=== benchmark: $PROJ (Gradle, synthetic commit pair) ==="
echo "change: CreateOrderSagaData.getRejectionReason() -- returns null (baseline), correct (candidate)"
echo "module: order-service/order-service-sagas (same module as CreateOrderSagaTest)"
echo "expected: CreateOrderSagaTest.shouldRejectCreateOrder selected (direct call to getRejectionReason)"
echo ""

git -C "$BENCH" worktree remove --force "$WORK" 2>/dev/null || true
rm -rf "$WORK" "$OUT"
mkdir -p "$OUT"

git -C "$BENCH" worktree add --detach "$WORK" HEAD
git -C "$WORK" config user.email bench@example.com
git -C "$WORK" config user.name bench

# Remove the toolchain block so Gradle uses the ambient JDK 21 (project requests 17)
sed -i '/toolchain {/,/}/d' "$WORK/build.gradle"

# baseline: break getRejectionReason() to always return null
awk '
  /public RejectionReason getRejectionReason\(\)/ { in_method=1 }
  in_method && /return rejectionReason;/ { sub(/return rejectionReason;/, "return null;") }
  /^  \}/ && in_method { in_method=0 }
  { print }
' "$WORK/$SAGA_DATA" > /tmp/_sgd_patched.java && mv /tmp/_sgd_patched.java "$WORK/$SAGA_DATA"
git -C "$WORK" commit -qam "benchmark-baseline: break CreateOrderSagaData.getRejectionReason() -> null"
BASELINE=$(git -C "$WORK" rev-parse HEAD)

# candidate: restore getRejectionReason()
awk '
  /public RejectionReason getRejectionReason\(\)/ { in_method=1 }
  in_method && /return null;/ { sub(/return null;/, "return rejectionReason;") }
  /^  \}/ && in_method { in_method=0 }
  { print }
' "$WORK/$SAGA_DATA" > /tmp/_sgd_patched.java && mv /tmp/_sgd_patched.java "$WORK/$SAGA_DATA"
git -C "$WORK" commit -qam "benchmark-candidate: restore CreateOrderSagaData.getRejectionReason()"
CANDIDATE=$(git -C "$WORK" rev-parse HEAD)

echo "baseline:  $BASELINE"
echo "candidate: $CANDIDATE"
echo "diff:"
git -C "$WORK" diff "$BASELINE" "$CANDIDATE" -- "$SAGA_DATA"
echo ""

echo "=== compiling at candidate ==="
t0=$(date +%s%3N)
(cd "$WORK" && ./gradlew \
  :order-service:order-service-domain:classes \
  :order-service:order-service-sagas:classes \
  :order-service:order-service-sagas:testClasses \
  :order-service:order-service-restapi:classes \
  :order-service:order-service-restapi:testClasses \
  :customer-service:customer-service-restapi:classes \
  :customer-service:customer-service-restapi:testClasses \
  -q --no-daemon 2>&1 | tail -5)
t1=$(date +%s%3N)
echo "  compile: $((t1 - t0))ms"

echo ""
echo "=== generating manifest (tree-sitter test discovery) ==="
t0=$(date +%s%3N)
"$GEN" \
  --project-dir "$WORK" \
  --output "$OUT/tia-artifacts.json" \
  --build-tool gradle \
  --java-version 17
t1=$(date +%s%3N)
echo "  manifest generation: $((t1 - t0))ms"

# Remove modules with tests but no compiled class dirs (e.g. migration-tests standalone project).
# Sets module_inventory_complete=false so TIA uses conservative fallback for those.
python3 "$STRIP" "$OUT/tia-artifacts.json"

echo ""
echo "tests discovered:"
python3 -c "
import json
m = json.load(open('$OUT/tia-artifacts.json'))
for mod in m['modules']:
    for t in mod['tests']:
        print(f\"  [{mod['root'] or '.'}] {t['class_name'].split('/')[-1]}.{t['method_name']}\")
print(f\"  total: {sum(len(mod['tests']) for mod in m['modules'])} tests across {len(m['modules'])} modules\")
"

python3 -c "
import json
config = {
  'service_descriptions': [{
    'name': 'eventuate-orders',
    'base_dir_path': '.',
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
# shouldRejectCreateOrder calls data.getRejectionReason() directly -- WALA should trace this
target_precise = 'shouldRejectCreateOrder'
# shouldCreateOrder is in the same module -- at minimum module_fallback should select it
target_module  = 'shouldCreateOrder'

if target_precise in sel_names:
    rtype = [list(t['reasons'][0].keys())[0] for t in r['selected_tests'] if t['test']['method_name']==target_precise][0]
    print(f'PASS: {target_precise} selected (reason: {rtype})')
    if rtype in ('call_graph', 'changed_callable'):
        print('PRECISE: call-graph reachability confirmed')
    else:
        print('CONSERVATIVE: module-level fallback (correct but imprecise)')
else:
    print(f'FAIL: {target_precise} not selected -- FALSE NEGATIVE')
    ok = False

if target_module in sel_names:
    rtype = [list(t['reasons'][0].keys())[0] for t in r['selected_tests'] if t['test']['method_name']==target_module][0]
    print(f'PASS: {target_module} selected (reason: {rtype})')
else:
    print(f'INFO: {target_module} not selected (expected if WALA-precise, OK if other module tests skipped)')

total = len(sel_names) + len(unsel_names)
skipped = len(unsel_names)
print(f'precision: {skipped}/{total} tests skipped ({100*skipped//total if total else 0}%)')
sys.exit(0 if ok else 1)
"
