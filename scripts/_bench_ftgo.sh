#!/bin/bash
set -euo pipefail

PROJ=ftgo-application
BENCH=/mnt/c/Users/I749046/benchmarks/java/$PROJ
WORK=/tmp/sar-bench-$PROJ
GEN=/home/user/cargo-target/debug/generate-java-tia-manifest
TIA=/home/user/cargo-target/debug/test-impact
WALA=/mnt/c/Users/I749046/software-architecture-reconstruction-tool/wala-callgraph/target/wala-callgraph-all.jar
OUT=/tmp/sar-bench-out-$PROJ
STRIP=/mnt/c/Users/I749046/software-architecture-reconstruction-tool/scripts/strip_manifest.py

# Gradle 6.9.1 bundled Groovy (2.5.x) only supports up to Java 16 (class version 60).
# JDK 11 and JDK 17 both fail; JDK 11 is the sweet spot for Gradle 6.x compatibility.
# Temurin JDK 11 is installed at /opt/jdk-11.
export JAVA_HOME="/opt/jdk-11"
export PATH="$JAVA_HOME/bin:$PATH"
echo "Java: $(java -version 2>&1 | head -1)"

# Synthetic change: DeliveryService.createDelivery() passes deliveryAddress as pickup (baseline)
# instead of restaurant.getAddress() (candidate).
# DeliveryServiceTest.shouldCreateDelivery asserts delivery.getPickupAddress() == PICKUP_ADDRESS.
# Both DeliveryService and DeliveryServiceTest are in ftgo-delivery-service -- same module.
DELIVERY_SERVICE=ftgo-delivery-service/src/main/java/net/chrisrichardson/ftgo/deliveryservice/domain/DeliveryService.java

echo "=== benchmark: $PROJ (Gradle, synthetic commit pair) ==="
echo "change: DeliveryService.createDelivery() -- passes wrong pickup address (baseline), correct (candidate)"
echo "module: ftgo-delivery-service (same module as DeliveryServiceTest)"
echo "expected: DeliveryServiceTest.shouldCreateDelivery selected via call_graph"
echo ""

git -C "$BENCH" worktree remove --force "$WORK" 2>/dev/null || true
rm -rf "$WORK" "$OUT"
mkdir -p "$OUT"

git -C "$BENCH" worktree add --detach "$WORK" HEAD
git -C "$WORK" config user.email bench@example.com
git -C "$WORK" config user.name bench

# baseline: pass deliveryAddress as pickup (wrong)
awk '
  /deliveryRepository\.save\(Delivery\.create\(orderId, restaurantId, restaurant\.getAddress\(\), deliveryAddress\)\)/ {
    sub(/restaurant\.getAddress\(\)/, "deliveryAddress")
  }
  { print }
' "$WORK/$DELIVERY_SERVICE" > /tmp/_ftgo_patched.java && mv /tmp/_ftgo_patched.java "$WORK/$DELIVERY_SERVICE"
git -C "$WORK" commit -qam "benchmark-baseline: DeliveryService.createDelivery uses deliveryAddress as pickup"
BASELINE=$(git -C "$WORK" rev-parse HEAD)

# candidate: restore correct restaurant.getAddress() as pickup
awk '
  /deliveryRepository\.save\(Delivery\.create\(orderId, restaurantId, deliveryAddress, deliveryAddress\)\)/ {
    sub(/deliveryAddress, deliveryAddress/, "restaurant.getAddress(), deliveryAddress")
  }
  { print }
' "$WORK/$DELIVERY_SERVICE" > /tmp/_ftgo_patched.java && mv /tmp/_ftgo_patched.java "$WORK/$DELIVERY_SERVICE"
git -C "$WORK" commit -qam "benchmark-candidate: DeliveryService.createDelivery restored correct pickup address"
CANDIDATE=$(git -C "$WORK" rev-parse HEAD)

echo "baseline:  $BASELINE"
echo "candidate: $CANDIDATE"
echo "diff:"
git -C "$WORK" diff "$BASELINE" "$CANDIDATE" -- "$DELIVERY_SERVICE"
echo ""

echo "=== compiling at candidate ==="
t0=$(date +%s%3N)
(cd "$WORK" && ./gradlew \
  :ftgo-order-service-contracts:build \
  :ftgo-consumer-service-contracts:build \
  :ftgo-kitchen-service-contracts:build \
  :ftgo-accounting-service-contracts:build \
  :ftgo-restaurant-service-contracts:build \
  :ftgo-common:classes :ftgo-common:testClasses \
  :ftgo-common-jpa:classes \
  :ftgo-delivery-service-api:classes \
  :ftgo-kitchen-service-api:classes \
  :ftgo-restaurant-service-api:classes \
  :ftgo-order-service-api:classes \
  :ftgo-consumer-service-api:classes \
  :ftgo-accounting-service-api:classes \
  :ftgo-delivery-service:classes :ftgo-delivery-service:testClasses \
  :ftgo-order-service:classes :ftgo-order-service:testClasses \
  :ftgo-kitchen-service:classes :ftgo-kitchen-service:testClasses \
  :ftgo-restaurant-service:classes :ftgo-restaurant-service:testClasses \
  :ftgo-consumer-service:classes :ftgo-consumer-service:testClasses \
  :ftgo-order-history-service:classes :ftgo-order-history-service:testClasses \
  :ftgo-accounting-service:classes :ftgo-accounting-service:testClasses \
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
  --java-version 8
t1=$(date +%s%3N)
echo "  manifest generation: $((t1 - t0))ms"

# Remove modules with tests but no compiled class dirs.
# ftgo-api-gateway and ftgo-end-to-end-tests need contract stubs published to build/repo
# (Spring Cloud Contract plugin), which requires the full publish pipeline -- skip them
# as expected-absent so module_inventory_complete stays true.
python3 "$STRIP" "$OUT/tia-artifacts.json" \
  --expected-absent ftgo-api-gateway ftgo-end-to-end-tests

echo ""
echo "tests discovered:"
python3 -c "
import json
m = json.load(open('$OUT/tia-artifacts.json'))
for mod in m['modules']:
    for t in mod['tests']:
        print(f\"  [{mod['root'] or '.'}] {t['class_name'].split('/')[-1]}.{t['method_name']}\")
print(f\"  total: {sum(len(mod['tests']) for mod in m['modules'])} tests across {len(m['modules'])} modules\")
print(f\"  module_inventory_complete: {m['module_inventory_complete']}\")
"

python3 -c "
import json
config = {
  'service_descriptions': [{
    'name': 'ftgo',
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
# shouldCreateDelivery checks delivery.getPickupAddress() -- direct call through createDelivery
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
