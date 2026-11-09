#!/usr/bin/env python3
"""Remove modules that have tests but no test_class_dirs (were not compiled).

Modules with tests but no class dirs trigger a validation error in the TIA
engine. Modules that cannot be compiled (e.g. standalone Gradle sub-projects
not included in the main settings.gradle) should be removed entirely so the
engine does not try to validate them. When modules are removed, mark
module_inventory_complete=false so TIA doesn't assume the inventory is complete.

Pass --expected-absent <root1> <root2> ... to list module roots that are
intentionally excluded (e.g. integration test modules needing external
infrastructure). These are removed without setting module_inventory_complete=false.
"""
import json, sys

args = sys.argv[1:]
expected_absent = set()
if '--expected-absent' in args:
    idx = args.index('--expected-absent')
    args.pop(idx)
    while idx < len(args) and not args[idx].startswith('-'):
        expected_absent.add(args.pop(idx))

path = args[0]
m = json.load(open(path))
before_modules = len(m["modules"])
before_tests = sum(len(mod["tests"]) for mod in m["modules"])

kept = []
removed_unexpected = 0
for mod in m["modules"]:
    root = mod["root"] or "."
    has_tests = bool(mod["tests"])
    has_class_dirs = bool(mod["test_class_dirs"]) or bool(mod["application_class_dirs"])
    if root in expected_absent:
        print(f"  [exclude] {root}: {len(mod['tests'])} tests, no test_class_dirs (expected absent)")
        # drop entirely — don't keep even as an empty module
    elif has_tests and not mod["test_class_dirs"]:
        print(f"  [remove] {root}: {len(mod['tests'])} tests, no test_class_dirs (module not compiled)")
        removed_unexpected += 1
    else:
        kept.append(mod)

m["modules"] = kept
after_tests = sum(len(mod["tests"]) for mod in m["modules"])

if len(kept) < before_modules:
    if removed_unexpected > 0:
        m["module_inventory_complete"] = False
        print(f"  set module_inventory_complete=false ({removed_unexpected} unexpected removals)")
    json.dump(m, open(path, "w"), indent=2)
    print(f"  removed {before_modules - len(kept)} modules, {after_tests}/{before_tests} tests remain")
