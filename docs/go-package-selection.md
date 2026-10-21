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

Final review also checked Windows process-launch/function-value safeguards and external-only test directories. Production initialization roots are mandatory when source-backed production exists; a directory containing only external test files does not acquire a source-less root. These cases have native scanner/grouping regressions, Rust rejection tests and real native-to-Rust fixtures.

Post-review verification on 2026-10-18: workspace tests **544 passed / 0 failed / 4 ignored**; native package integration explicitly **8 passed / 0 ignored** (including the two normally ignored real-provider tests); Go race tests and vet passed; ten benchmark-harness controls passed. Native graph, adapter and unchanged conservative controls passed. A fresh 12-case real replay again recorded zero omitted failing binaries and zero inconclusive cases.

## Broader mutation campaign

```sh
scripts/run-go-package-validation.sh
```

This opt-in extension preserves the original 12-case runner. It inspects **every immediate benchmark repository** in `GO_BENCHMARK_ROOT`, recording its revision, module layout and readiness result. Manifest projects are checked at their pinned replay revisions, not an unrelated current HEAD; other repositories are inventoried at their recorded HEAD. Root-module candidates with test files get offline package loading, a complete baseline test run and native package-evidence inspection in isolated clones. Nested layouts, absent tests, missing dependencies, blockers and baseline failures remain visible in `readiness/readiness.json`; they are not silently dropped or counted as successful projects. No dependencies are installed and original checkouts remain unchanged.

The fixed `manifest.json` includes the original controls plus additional body mutations covering comparisons, numeric constants, result accumulation, lookups, dates, formatting, identifiers and event collections. Cases are not chosen after observing whether their tests fail. Every case is a fresh isolated baseline/candidate pair, tested by both a full-module run and the selected whole binaries. Compilation failures are invalid mutants, not detected regressions. Mutants that pass the full suite are survivors (possibly equivalent), not detections.

The summary separates package selection, cases with actual binary reduction, whole-module fallback, failure-producing cases, survivors, invalid/inconclusive cases and omitted failing binaries. Each timed case records full-suite time and analysis-plus-selected-test time, including negative savings. These sequential warm-cache timings are descriptive, not randomized performance estimates; cloning, readiness and mutation setup are outside that comparison and have separate command records.

The coverage target is **30 successful body mutations across at least three independent projects**. `success` means the executed controls passed; **it does not mean the coverage target was met**. Check `coverage_target_met` and `limitations` separately. Multiple services in one repository are not counted as independent projects. All four broad-change controls remain in the campaign and must fall back rather than being forced into a reduction result.

### Observed campaign: 2026-10-19

The inspected set contained 13 repositories: two runnable projects, ten unsupported module layouts, and one repository without tests. The campaign ran **35 body mutations plus four broad-change controls** on the two runnable projects.

| Outcome | Cases |
| --- | ---: |
| Actual package reduction (1 of 5 binaries retained) | 26 |
| Valid package selection without reduction (coffeeshop's shared binary) | 7 |
| Intended broad-change fallback | 4 |
| Unexpected body-change fallback | 2 |
| Mutations/controls producing full-suite failures | 17 |
| Full-suite failing cases whose failures were retained | 17 |
| Full-suite survivors, including six fallback cases | 22 |
| Omitted failing binaries / invalid mutants / inconclusive cases | 0 / 0 / 0 |

The run deliberately exits nonzero: **37 expectations passed and two failed**, not 39/39. `profile-empty-loaded-map` and `rate-empty-loaded-map` triggered the documented positive-graph-contradiction safeguard and selected the whole module. These safe over-selections remain failing reduction expectations; their expected modes were not changed to manufacture a passing campaign. Failed evaluations remain in observed outcome and timing denominators. The three-project breadth target is explicitly unmet even though 33 body cases successfully used package selection.

Sequential warm-cache timing across the same 39 cases was **58.381s for full suites versus 81.087s for analysis plus selected tests**. None of these individual comparisons saved time after analysis overhead. These small suites demonstrate execution-scope reduction, not an end-to-end speedup. Do not extrapolate performance from this sample.

Artifacts: `/private/var/folders/3k/ksjl2jsn143fn68w_1hmjqkc0000gn/T/sar-go-package-selection.BNq6uW/`. `summary-verified.json` reaggregates the unchanged per-case observations with the final survivor-counting logic; the original run summary, all candidate patches, reports, test events and command durations remain preserved. The one-command runner uses that final logic for future runs.

Verification: 19 harness tests passed; workspace tests 544 passed, zero failed, four ignored; formatting, shell syntax and diff checks passed. Fresh read-only review found and prompted RED→GREEN fixes for biased success-only reporting denominators and HEAD-versus-pinned readiness. A further regression keeps unexpected-fallback survivors visible; no selection algorithm or safety guard was changed.

### Binary-scoped CHA traversal

Tracing both unexpected fallbacks exposed the same raw CHA path: a frontend test defers a `context.CancelFunc` (signature `func()`), which module-wide CHA connects to an unrelated executable's `main()`. That executable calls the profile/rate constructors and loaders. The executable is not linked into the frontend test binary, so this is not a possible in-binary call path.

Native graph reachability now keeps separate package closures, initialization roots and traversal visited sets for each test binary. Closures use the loader's actual package identities and test associations, including internal/external siblings and source-backed production initialization for external-only tests. A path cannot traverse a known package outside that binary, even when its final target is linked. Synthetic functions without a recoverable owner remain conservative; known owners are recovered from SSA packages, declaration objects or lexical parents. The raw CHA graph and sort callback summaries remain shared and unchanged.

This narrows impossible cross-binary graph suggestions; it does **not** authorize exclusions using absent graph paths. The independent package-policy validation and positive-evidence contradiction safeguard are unchanged. Signature-only overapproximation within a binary, opaque dependencies, reflection and general callback completeness remain limitations.

Post-fix replay on 2026-10-19 passed **39/39 expectations**: 35 package-selection cases, including **28 reductions** (one of five binaries selected) and seven shared-binary cases without reduction; only the four intended broad-change controls fell back. Both loader mutations now select one binary and exclude four. All 17 full-suite failure-producing cases retained their failing binary; 22 observations survived; zero failing binaries were omitted and no cases were invalid or inconclusive. The original failing replay above remains preserved as before/after evidence.

Post-fix artifacts: `/private/var/folders/3k/ksjl2jsn143fn68w_1hmjqkc0000gn/T/sar-go-package-selection.N8jKTx/summary.json`. The three-project target remains unmet (two eligible independent projects), and no general soundness or speedup claim follows from this replay.

Post-fix sequential timings remain descriptive: full suites totaled 62.611s versus 82.969s for analysis plus selected tests. Four individual cases saved time, but the aggregate still did not. These measurements were collected alongside compatibility verification and are not a controlled performance experiment.

Verification: native race tests and vet passed; native graph, Rust adapter and unchanged default-policy controls passed (7 discovered / 5 suggested / 7 selected); 19 harness tests passed; workspace 544 passed / 0 failed / 4 ignored. Focused native regressions first reproduced impossible paths through unlinked executables, including a linked final target, and missing external-only production initialization. They pass with function-value, bound-method and existing sort/initialization checks. A fresh read-only review reported no findings. No Rust policy, wire shape, benchmark expectation or contradiction safeguard was weakened.
