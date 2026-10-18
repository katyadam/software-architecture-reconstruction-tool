# Opt-in Go package selection

The default Go policy remains conservative: graph reachability suggests tests but never authorizes exclusions. Its report stays schema 1. Java/WALA/SootUp behavior is unchanged.

The optional `packages` policy selects entire test binaries using independently validated imports, not absent CHA paths. It requires an explicit `linked-binary` scope declaration and publishes schema 2, including fallback and no-op reports.

## Build and analyze

Run each command as a single line from this repository:

```sh
(cd go-callgraph && go build -mod=readonly -o ../target/go-callgraph .)
cargo build -p cli --bin go-test-impact
target/debug/go-test-impact --project-dir /absolute/path/to/module --baseline-revision BASELINE_SHA --go-callgraph-binary "$PWD/target/go-callgraph" --selection-policy packages --test-scope linked-binary --output /tmp/go-package-impact.json
```

Use a clean committed Git root module and prepared dependencies. Analysis is offline and never installs dependencies or edits the project. The timeout defaults to 300 seconds **per native stage**, not a total-run deadline. Supply `--analysis-timeout-seconds 300` to state it explicitly. Progress goes to stderr; JSON is atomically written outside the analyzed checkout.

## What the scope declares

Tests consume only their compiled binary under the recorded build context. They must not read source/Git state or separately built artifacts as inputs, execute/build other checkout components, load source-derived plugins, depend on another binary's side effects/order, or exercise distributed/shared infrastructure whose behavior changes independently of their imports. Package imports do not prove these assumptions; callers must decide whether they hold. Use the conservative default otherwise.

Additional local safeguards block exclusions for TestMain, plugin/process/unsafe escape indicators, go:linkname, cgo/assembly, outside-checkout local replacements, and uninspectable active sources. This is a local scan, not proof of opaque dependencies' behavior. Reflection and callbacks inside an affected binary do not require per-callable completeness because that whole binary is retained.

Only existing-declaration body edits qualify. Configuration, imports, build tags, globals/types, signatures, explicit initialization edits, additions/deletions/renames and uncertain mappings require a whole-module run. Unsupported module/workspace layouts and dirty/uncommitted active inputs remain input errors.

## Consume execution data, not graph suggestions

- Check `selection_mode`, `run_all_tests`, `execution_inventory_complete` and `package_evidence` first. Missing metadata is not an empty successful inventory.
- `conservative_all` means run the whole module, **not** merely the known execution-package list. Metadata failure retains any completed ordinary TestX inventory but cannot certify example/fuzz-only binary discovery.
- `conservative_packages` means package evidence was validated. Selected binaries are in `selected_execution_packages`; exclusions are separately listed. It is valid for every binary to remain selected, with `run_all_tests: true` and no reduction.
- `no_changes` is a no-op: no discovery, unavailable effective context, incomplete/skipped execution inventory, `run_all_tests: false`.

Each execution decision contains `package.import_path`, `package.relative_directory`, an exact `argument` (`.` or `./relative-directory`), changed owner IDs and the dependency intersection/reason. IDs are opaque loader identities, not names to parse or invent. Never expand `...` or interpolate report content into a shell command.

For a valid subset, execute whole selected packages with an argument array and the report's `effective_build_context.environment` and `build_flags`; do not apply a TestX-only `-run` filter. Examples, normal fuzz seeds and initialization are part of the whole-binary decision. Run from the verified module root and require the recorded Go driver version. The CLI analyzes only; it never runs tests itself.

**If the selected argument array is empty, skip execution. Never turn it into bare `go test`.** An eligible empty selection differs from missing evidence or a skipped inventory.

Alternate race/coverage/compiler flags or a different toolchain/platform/tag context are outside this first policy's claim. Graph failure removes suggestions but does not invalidate independently complete package evidence. Contradictory valid positive graph evidence forces whole-module fallback.

## Verification

The default control remains `scripts/run-go-impact-control.sh` (7 discovered / 5 graph-suggested / 7 selected).

Run the real-project controls with one command:

```sh
scripts/run-go-package-selection-benchmarks.sh
```

It builds local tools offline, uses prepared dependencies, clones isolated copies of `../../../benchmarks/go/go-micro-services` and `go-coffeeshop`, and preserves all results in a newly created temporary directory. It never changes the original projects or downloads their dependencies. It prints phase logs and the final `summary.json` path. Output directories must be empty; prior results are not overwritten.

Optional environment variables are `GO_BENCHMARK_ROOT`, `TIA_BENCHMARK_OUTPUT_DIR`, `GO_ANALYSIS_TIMEOUT_SECONDS` (default 300), `GO_CALLGRAPH_BINARY` and `GO_TEST_IMPACT_BINARY` (prebuilt executable overrides). Missing dependencies yield a preparation instruction, not an automatic download; cache-permission errors are reported separately.

Local replay on 2026-10-18 passed all 12 controls with zero observed failing binaries excluded:

| Controls | Execution result |
| --- | --- |
| Profile, geo, rate, frontend date/status, sort callback | 1 of 5 binaries selected; 4 excluded in each case |
| Frontend edits | All 3 frontend TestX retained together |
| Sort callback, graph deliberately disabled | Geo binary retained without any CHA suggestions |
| Coffeeshop ID and uncovered helper | Both TestX retained in their single shared binary; no package reduction |
| Configuration, signature, deletion, initialization | Whole-module fallback; execution inventory explicitly unavailable |

All six failure-producing controls retained their observed failing binary. The two coffeeshop mutants survived the existing tests; they are recorded as survivors, not claimed detections. Each case also compares selected-run failures with an independent full-module run and compares package decisions with separately collected `go list -deps -test` metadata. Harness tests deliberately omit a failing binary and require the control to fail.

Artifacts include committed revision IDs, mutation patches, independent package observations, analyzer reports/stderr, baseline/full-mutant/selected Go test JSON, and per-case results. Binary counts are unavailable (not zero) for incomplete fallback inventories. This is empirical evidence under the declared scope, **not a general soundness proof**.
