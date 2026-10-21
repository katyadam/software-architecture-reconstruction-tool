#!/usr/bin/env python3
import json, sys
path = sys.argv[1] if len(sys.argv) > 1 else "/tmp/sar-bench-out-ftgo-application/tia-artifacts.json"
m = json.load(open(path))
print(f"module_inventory_complete: {m['module_inventory_complete']}")
print()
for mod in m["modules"]:
    tic = mod.get("test_inventory_complete", "?")
    dic = mod.get("dependency_inventory_complete", "?")
    root = mod["root"] or "."
    marker = " <---" if not tic or not dic else ""
    print(f"  [{root}] tic={tic} dic={dic} tests={len(mod['tests'])}{marker}")
