# Go Package Selection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add explicitly scoped, opt-in whole-Go-test-binary selection using independently validated package dependencies, preserving conservative execution by default.

**Architecture:** The native provider supplies a separate graph-free package capability. Rust validates that capability against committed source and ordinary discovery, then selects complete execution packages. CHA remains positive diagnostic evidence, not exclusion authority; Java providers remain separate.

**Tech Stack:** Existing Go 1.26 module and pinned golang.org/x/tools/go/packages; Rust, Serde, Clap; shell/Python benchmark controls using prepared dependencies.

**Spec:** `docs/superpowers/specs/2026-10-18-go-package-selection-design.md`

## Global Constraints

- Preserve `analyze_go_impact(&GoImpactOptions)`, its callers, default report schema 1, and ordinary native inventory schema 1.
- Use `--selection-policy conservative|packages`, default `conservative`; accept `--test-scope linked-binary` only with `packages`, where it is required.
- Package-policy reports use schema 2, including fallback/no-op; exclusions concern only the caller-declared linked-binary contract.
- Only wholly mapped, nonempty, existing-declaration body edits qualify. Existing broad changes fall back; unsupported repository layouts remain input errors.
- Keep clean-root, committed-source, offline scratch-module, Unix process supervision and atomic-publication protections.
- Package inventory gets an independent deadline equal to the configured per-stage timeout. Never erase ordinary discovery after metadata failure.
- No dependency installation during analysis; no benchmark-source modifications; no Java changes, default-policy switch, or per-test exclusion inside an affected binary.
- Implement natively on the existing isolated `feat/go-callgraph-tia` branch after plan approval; preserve `.idea/` and other user changes. Each task has a review gate before the next task.

## Review Focus

1. A dependency leaves and re-enters the main module: traverse external nodes rather than truncate imports (Tasks 1, 4).
2. An example/fuzz-only binary or external-test sibling has no ordinary TestX entry: retain complete execution grouping (Tasks 1, 3, 4).
3. Metadata disappears, times out, or observes a different toolchain/source set: retain basic discovery but require a whole-module run (Tasks 3, 5).
4. Process-launch aliases/function values, generated harness files, or external local replacements masquerade as ordinary sources: block exclusions without guessing package IDs (Tasks 1–3).
5. Zero selected binaries or a directory containing Go metapattern syntax: never publish an accidental bare `go test` instruction (Tasks 3, 6, 7).

---

## File structure and ownership

| Files | Responsibility |
| --- | --- |
| `go-callgraph/load_context.go`, existing `loading.go` | Shared protected loader setup and effective driver context; preserve ordinary loading behavior. |
| `go-callgraph/package_protocol.go`, `package_inventory.go` | Separate capability DTOs and complete package-instance/import/binary ownership facts. |
| `go-callgraph/package_blockers.go`, existing `main.go` | Local-source escape scan and independently supervised native mode. |
| `go-callgraph/package_inventory_test.go`, `package_blockers_test.go`, existing `main_test.go` | Native evidence and mode regression tests. |
| `models/src/go_package_impact.rs`, existing `lib.rs`, `go_impact.rs` | New evidence/report types and additive selection/reason variants; no new required fields in old DTOs. |
| `extractor-runtime/src/call_graph/go_native/packages.rs`, existing `go_native.rs`, `go_native/inventory.rs` | Package-mode transport and structural/cross-inventory validation. |
| `extractor-runtime/src/impact/go_package_selection.rs`, existing `mod.rs` | Pure package closure and whole-binary decisions. |
| `extractor-runtime/src/impact/go_package.rs`, existing `go.rs`, `go_selection.rs`, `go_git.rs` | Opt-in orchestration and shared existing safety/positive-evidence checks. |
| `cli/src/bin/go-test-impact.rs`, `cli/src/go_impact_output.rs` | Policy/scope validation and schema-agnostic atomic serialization. |
| `models/tests/go_package_impact.rs`, `extractor-runtime/tests/go_package_inventory.rs`, `go_package_selection.rs`, `go_package_impact.rs`, `cli/tests/go_package_impact_cli.rs` | Layered protocol, validation, policy, orchestration and CLI tests. |
| `scripts/run-go-package-selection-benchmarks.sh`, `scripts/evaluate-go-package-selection.py`, `scripts/test-go-package-selection-benchmarks.sh` | One-command isolated real-project mutation replay and retained artifacts. |
| `docs/go-package-selection.md` | Scope, exact execution data, preparation and reproducible controls. |

## Shared contract decisions

Task 1 defines Go wire structs; Task 3 mirrors their snake_case fields in Rust:

- `EffectiveBuildContext` / `GoEffectiveBuildContext`: `go_version`, `goos`, `goarch`, `cgo_enabled` (strings), `architecture_features` (string map), `goexperiment` (string), `build_tags` (normalized string), `environment` (string map), `build_flags` (string array). Version is the loading driver's `GOVERSION`, not `runtime.Version()`.
- `PackageNode` / `GoPackageNode`: `id`, `import_path`, `imports` (opaque ID array), `provenance`, `source_files` (canonical active local files), `test_files` (active local test-file subset), `for_test` (loader value, not an ID parser).
- `PackageProvenance` / `GoPackageProvenance`: tagged `kind` = `verified_module`, `module_cache`, `standard_library`, `generated_test_harness`, or `local_replacement`; include `directory` and optional `module_path`, `module_version`. Local replacements outside the verified root are blockers, never module-cache evidence.
- `ExecutionPackage` / `GoExecutionPackage`: `import_path`, `relative_directory`, `roots` (opaque node ID array). Root directory is `"."`; other directories are slash-separated module-relative paths.
- `PackageBlocker` / `GoPackageBlocker`: `code`, optional `source_path`, `diagnostic`. Codes are exactly `test_main`, `dynamic_loading`, `process_execution`, `unsafe_linkage`, `cgo_or_assembly`, `outside_local_replacement`, `uninspectable_local_sources`.
- `PackageInventoryOutcome` / `GoPackageInventoryOutcome`: `schema_version: 1`, `provider_id: "go-native"`, `status`, `source_root`, optional `scope`, optional `effective_build_context`, `complete` (boolean), `packages`, `execution_packages`, `blockers`, `diagnostics`. Missing required evidence never defaults to complete. Failure has `complete: false`.
- Rust `GoPackageTestScope::LinkedBinary` serializes `linked-binary`; `GoSelectionPolicy::{Conservative, Packages}` serializes lowercase. CLI names follow the same strings.
- `GoPackageImpactReport` flattens the existing `GoImpactReport` into schema 2 and adds `requested_policy`, `declared_test_scope`, optional `effective_build_context`, `package_evidence: GoStageReport`, `execution_inventory_complete`, `discovered_execution_packages`, `selected_execution_packages`, `unselected_execution_packages`. Selected/unselected entries are `GoExecutionPackageDecision { package: GoExecutionPackage, argument: String, changed_owners: Vec<String>, dependency_intersection: Vec<String>, reason: String }`.
- Add `GoSelectionMode::ConservativePackages` and `GoSelectionReason::PackageDependency { execution_package: String, changed_owners: Vec<String> }` without changing serialization of existing variants. Unselected TestX entries retain the existing `GoTestCase` shape.
- Exact arguments are `.` or `./<relative_directory>`; reject absolute paths, traversal, non-normal components, backslashes, and `...` anywhere. Do not interpolate arguments into shell text.

### Task 1: Independent native package graph and effective context

**Files:** Create `go-callgraph/load_context.go`, `package_protocol.go`, `package_inventory.go`, `package_inventory_test.go`; modify `go-callgraph/loading.go`.

**Interfaces:**
- Consumes existing `Options`, `Scope`, ordinary `Discover(context.Context, Options) InventoryOutcome`, and existing `testModule` fixture helper.
- Produces `DiscoverPackages(ctx context.Context, opts Options) PackageInventoryOutcome`.
- Produces private `prepareLoad(ctx context.Context, opts Options) (*loadContext, error)` and `(*loadContext).Close()`; `loadContext` owns `Root string`, `Config packages.Config`, `Effective EffectiveBuildContext` and scratch-directory cleanup. Both loaders use it.

- [ ] **Step 1: Add native fixture tests before implementation.** Extend the existing `testModule` pattern with a local temporary dependency module for the external-bridge fixture; keep all dependency preparation inside test setup. Add tests named `TestPackageInventoryDirectTransitiveAndExternalBridge`, `TestPackageInventoryVariantsAndInit`, `TestPackageInventoryExampleFuzzAndBenchmarkOnly`, `TestPackageInventoryDoesNotLoadDependencySyntax`, `TestPackageInventoryEffectiveDriverContext`, and `TestPackageInventoryGeneratedHarnessProvenance`. Assert the wire facts, not bracket/suffix-derived IDs:

  ```go
  // In the direct fixture find nodes by exact recorded source-file ownership.
  if out.SchemaVersion != 1 || out.ProviderID != "go-native" || !out.Complete { t.Fatal(out) }
  // Assert app -> external bridge -> local helper IDs exist as direct edges.
  // Assert internal/external test variants share one execution package's roots.
  // Assert example/fuzz/benchmark-only fixture enumerates three binaries and zero TestX.
  // Assert normal and augmented nodes both own production.go; normal owns no helper_test.go.
  // Assert dependency Syntax is empty using a package-loading test seam, not timing.
  // Compare effective version to `go env GOVERSION` under the loader's protected env.
  ```

- [ ] **Step 2: Run `cd go-callgraph && go test ./... -run TestPackageInventory -count=1`.** Expected FAIL because the new operation/types do not exist.
- [ ] **Step 3: Extract protected loader setup into `prepareLoad` without changing ordinary inventory or graph load modes.** Resolve the actual driver context with supervised `go env -json` using the same environment as loading. Record applicable architecture keys (`GO386`, `GOAMD64`, `GOARM`, `GOARM64`, `GOMIPS`, `GOMIPS64`, `GOPPC64`, `GORISCV64`, `GOWASM`), CGO, GOEXPERIMENT, explicit tags and protected execution environment. Normalize tags once. Record semantic build flags (`-mod=readonly`, optional tags), not ephemeral scratch `-modfile` paths. Do not change the old inventory wire scope to smuggle new fields into schema 1.
- [ ] **Step 4: Implement `DiscoverPackages` using metadata-only `NeedName|NeedFiles|NeedCompiledGoFiles|NeedImports|NeedDeps|NeedModule|NeedForTest`, `Tests: true`.** Traverse every loaded dependency by opaque ID, sort/deduplicate output, and fail on loader errors or unknown provenance. No `NeedSyntax`, `NeedTypes`, SSA or call graph for dependencies. Determine local ownership from canonical actual file instances and loader module records. Standard library provenance requires GOROOT-backed facts; generated harness provenance requires actual generated/cache-backed main source and imports into recorded test instances, not a `.test` name guess. Group binary roots using loader test association plus source-backed internal/external instances, including test-file groups without TestX. Keep helper-only `_test.go` ownership distinct from production variants. Retain full imports through nonlocal intermediaries.
- [ ] **Step 5: Run `cd go-callgraph && go test ./... -count=1`.** Expected PASS, including old inventory/graph tests and unchanged ordinary JSON contracts.
- [ ] **Step 6: Review gate.** Check scratch lifetime, metadata-only mode, complete external traversal, actual driver version, generated harness classification and test-file grouping against the spec; fix findings before committing.
- [ ] **Step 7: Commit only Task 1 files:** `git add -- go-callgraph/load_context.go go-callgraph/loading.go go-callgraph/package_protocol.go go-callgraph/package_inventory.go go-callgraph/package_inventory_test.go` then `git commit -m "feat: expose graph-free Go package dependency evidence"`.

### Task 2: Local escape blockers and native command supervision

**Files:** Create `go-callgraph/package_blockers.go`, `package_blockers_test.go`; modify `go-callgraph/package_inventory.go`, `main.go`, `main_test.go`.

**Interfaces:**
- Consumes Task 1 DTOs and `DiscoverPackages`.
- Produces private `scanPackageBlockers(ctx context.Context, root string, packages []*packages.Package) ([]PackageBlocker, error)`; loading calls it before reporting complete evidence.
- Produces `--mode package-inventory` with the same root/tags/timeout CLI conventions as ordinary inventory; no targets required.

- [ ] **Step 1: Add table-driven `TestPackageBlockers` covering all seven exact blocker codes.** Include aliased/dot imports, `f := os.StartProcess`, an aliased syscall launch function stored then called, `go:linkname`, cgo, local assembly, TestMain-only input, external local replacement and unreadable/invalid local input. Include negative controls for ordinary reflection, sort callbacks and syscall signal constants. Assert a blocker prevents exclusion eligibility even if graph metadata is complete; scan failure sets `complete == false`. Add `TestPackageInventoryModeTimeout` and `TestPackageInventoryModeNeedsRootNotTargets` in `main_test.go` using the existing command test pattern.
- [ ] **Step 2: Run `cd go-callgraph && go test ./... -run 'TestPackageBlockers|TestPackageInventoryMode' -count=1`.** Expected FAIL for unsupported mode/missing safeguards.
- [ ] **Step 3: Implement `scanPackageBlockers`.** Parse only active verified-local files; resolve imports/aliases when inspecting selectors and dot-import references, including references used as function values. Conservatively flag `plugin`, `os/exec`, `unsafe`, `os.StartProcess`, syscall launch entrypoints (`Exec`, `ForkExec`, `StartProcess`) and low-level syscall function references that could launch processes. Do not block merely importing syscall for signal constants. Inspect active cgo/assembly metadata and raw directives. Deduplicate blockers by code/source; preserve explanatory diagnostics. A failed local scan cannot be marked complete.
- [ ] **Step 4: Wire the new mode through `main.go`'s worker/deadline/error-output branches.** Emit package-specific schema-1 failure JSON with `complete: false`; keep existing modes and timeout defaults unchanged.
- [ ] **Step 5: Run `cd go-callgraph && go test -race ./... && go vet ./...`.** Expected PASS; confirm old modes still produce their previous envelopes.
- [ ] **Step 6: Review gate.** Trace each blocker through active-source discovery, alias handling and failure serialization; check negative controls do not silently broaden the restriction.
- [ ] **Step 7: Commit Task 2 files:** `git add -- go-callgraph/package_blockers.go go-callgraph/package_blockers_test.go go-callgraph/package_inventory.go go-callgraph/main.go go-callgraph/main_test.go` then `git commit -m "feat: guard Go package evidence against local execution escapes"`.

### Task 3: Rust package contracts, supervised adapter and fail-closed validation

**Files:** Create `models/src/go_package_impact.rs`, `models/tests/go_package_impact.rs`, `extractor-runtime/src/call_graph/go_native/packages.rs`, `extractor-runtime/tests/go_package_inventory.rs`; modify `models/src/lib.rs`, `models/src/go_impact.rs`, `extractor-runtime/src/call_graph/go_native.rs`, `go_native/inventory.rs`.

**Interfaces:**
- Consumes Task 1 wire contracts and Task 2 native mode.
- Produces `GoNativeProvider::discover_packages(&self, root: &Path) -> GoPackageInventoryOutcome`.
- Produces public `validate_package_inventory(root: &Path, inventory: &GoInventoryOutcome, evidence: &GoPackageInventoryOutcome) -> Result<ValidatedGoPackageInventory, String>` in `go_native/packages.rs`, re-exported from `go_native.rs`. `ValidatedGoPackageInventory` exposes immutable accessors `evidence(&self) -> &GoPackageInventoryOutcome` and `owners(&self) -> &BTreeMap<String, BTreeSet<String>>`; construction remains private to the validator.
- Produces report/policy DTOs from Shared contract decisions. Reuse `GoStageStatus`, `GoStageReport`, `GoAnalysisScope`, `GoImpactReport`, not duplicate versions.

- [ ] **Step 1: Add DTO roundtrip/snapshot tests and adapter mocks that return real wire JSON.** Test unknown version/provider, missing completeness/provenance/context, duplicate IDs/binary identities, dangling imports, root not found, foreign/symlinked local paths, contradictory ownership, missing ordinary active files, missing TestX-owning roots, omitted example-only groups and malformed exact directories. Assert `validate_package_inventory(...).is_err()` for each. A genuinely complete empty inventory validates; absent evidence does not. Context mismatch tests independently vary actual version, tags, GOOS/GOARCH, CGO, architecture features and source sets where ordinary evidence supplies those facts; additional effective facts remain explicitly recorded, not invented in old inventory.
- [ ] **Step 2: Run `cargo test -p models --test go_package_impact` and `cargo test -p extractor-runtime --test go_package_inventory`.** Expected FAIL for missing module/types/methods.
- [ ] **Step 3: Implement strict DTOs and supervised transport.** Expose existing metadata transport to sibling modules with `pub(super)` rather than duplicating process handling. Parse legacy unsupported-mode/nonzero/timeout/malformed responses into failed, incomplete package outcomes with diagnostic tails. Never mutate/replace ordinary inventory.
- [ ] **Step 4: Implement `validate_package_inventory`.** Validate envelope/root/scope against ordinary discovery and requested tags, require complete success and no blockers, validate all graph references and provenance, and build exact source-to-all-owner IDs. Verify every inventory source has owners; additional committed local files such as assembly must be accounted for, not silently ignored. Check test-file groups cover each execution package and each active test-file group is represented; TestX source-owning test variants must be roots of its execution binary. Reject generated harnesses posing as committed sources, external replacements and nonrepresentable exact arguments. Derive membership from recorded facts, never parse IDs. Reject scope version disagreement with the actual driver rather than treating the provider's compiled version as proof.
- [ ] **Step 5: Run the two Task 3 test commands plus `cargo test -p models --test go_impact` and `cargo test -p extractor-runtime --test go_source_inventory`.** Expected PASS with unchanged schema-1 snapshots and legacy inventory support.
- [ ] **Step 6: Review gate.** Audit every path granting `ValidatedGoPackageInventory`; malformed/partial/blocked evidence must have no path into selection. Check wire names against Go DTOs.
- [ ] **Step 7: Commit only the Task 3 file list:** `git commit -m "feat: validate independent Go package selection evidence"` after explicit-path staging.

### Task 4: Pure whole-binary dependency policy

**Files:** Create `extractor-runtime/src/impact/go_package_selection.rs`, `extractor-runtime/tests/go_package_selection.rs`; modify `extractor-runtime/src/impact/mod.rs`, `go_selection.rs`.

**Interfaces:**
- Consumes validated evidence/ownership from Task 3 and existing `GoChangedElement`, `GoInventoryOutcome`, `GoAnalysisResult`, `GoSelectionDecision`.
- Produces `select_go_packages(changes: &[GoChangedElement], inventory: &GoInventoryOutcome, evidence: &ValidatedGoPackageInventory, graph: Option<&GoAnalysisResult>) -> GoPackageSelectionDecision`.
- `GoPackageSelectionDecision` contains `tests: GoSelectionDecision`, `selected_execution_packages: Vec<GoExecutionPackageDecision>`, `unselected_execution_packages: Vec<GoExecutionPackageDecision>`, `execution_inventory_complete: bool`.
- Extract `validated_graph_suggestions(changes: &[GoChangedElement], inventory: &GoInventoryOutcome, graph: Option<&GoAnalysisResult>) -> Option<Vec<GoSuggestedTest>>` from existing graph checks; both policies call it without weakening those checks.

- [ ] **Step 1: Write multi-binary table tests with exact expected identities.** The fixture is main helper H, direct binary A, independent B, and C importing H through an external node. H edit selects `["example/A", "example/C"]`, excludes `["example/B"]`. A's internal/external TestX siblings are all selected. A `_test.go` helper edit selects only A; a helper used solely by initialization still selects its importer. Test two production owner instances, example/fuzz-only binaries, all-affected and none-affected cases. Assert `run_all_tests` respectively false, true for nonempty all-selected, and false for empty selection. Missing/failed graph preserves package decisions and yields no suggestions. Valid positive suggestion into B forces conservative-all; invalid graph is unavailable rather than exclusion evidence.
- [ ] **Step 2: Run `cargo test -p extractor-runtime --test go_package_selection`.** Expected FAIL for missing package policy.
- [ ] **Step 3: Implement graph suggestion extraction and package traversal.** C is the union of every exact owner of each changed source, not only a declaration's single package. Traverse union roots with a visited-ID set through all graph nodes, including roots. Partition entire binaries and then TestX entries by execution identity; populate changed owners/intersection/reasons. Any change not wholly callable/body-only, absent owner, unknown execution identity or valid positive contradiction returns the existing conservative decision and marks the reason explicitly. Use sorted deterministic sets; do not remove evidence limitations.
- [ ] **Step 4: Run `cargo test -p extractor-runtime --test go_package_selection` and `cargo test -p extractor-runtime --test go_impact_selection`.** Expected PASS; old conservative selection still selects every discovered TestX.
- [ ] **Step 5: Review gate.** Manually trace the external bridge, test-only helper and none-affected cases; check closure intersection and whole-binary TestX partition agree.
- [ ] **Step 6: Commit Task 4 files with explicit-path staging:** `git commit -m "feat: select whole Go test binaries by verified imports"`.

### Task 5: Opt-in orchestration, committed evidence and honest fallback reports

**Files:** Create `extractor-runtime/src/impact/go_package.rs`, `extractor-runtime/tests/go_package_impact.rs`; modify `extractor-runtime/src/impact/go.rs`, `go_git.rs`, `mod.rs`.

**Interfaces:**
- Preserve `analyze_go_impact(options: &GoImpactOptions) -> Result<GoImpactReport, GoImpactError>` unchanged.
- Produce separate `analyze_go_package_impact(options: &GoImpactOptions, scope: GoPackageTestScope) -> Result<GoPackageImpactReport, GoImpactError>` in `go_package.rs`, re-exported by `impact::go`.
- Extract internal `analyze_go_impact_core(options: &GoImpactOptions, package_scope: Option<GoPackageTestScope>) -> Result<(GoImpactReport, Option<GoPackageImpactReport>), GoImpactError>` to share one discovery/mapping/graph flow; conservative entrypoint returns the first report. The optional report is present exactly when package scope was requested. No second discovery or graph invocation.
- Extract `verify_committed_sources(diff: &GoGitDiff, files: &[String]) -> Result<(), GoImpactError>` into `go_git.rs`; use for ordinary and expanded package files.

- [ ] **Step 1: Add instrumented provider-mock orchestration tests.** Assert call order ordinary inventory -> describe -> package inventory -> graph for eligible package requests; default makes zero package calls. Metadata sleeps beyond its deadline while ordinary discovery succeeds: discovered TestX retained, package stage failed, execution completeness false, whole-module run required. Graph failure with valid metadata still allows a subset. Empty diff calls no provider modes and reports schema 2/no_changes/false/incomplete/skipped/null context. Broad configuration/signature/import/init/deletion changes never invoke package mode. Preserve nonzero input errors for nested module/workspace/dirty HEAD and active source absent from candidate commit.
- [ ] **Step 2: Run `cargo test -p extractor-runtime --test go_package_impact`.** Expected FAIL for missing opt-in runtime entrypoint.
- [ ] **Step 3: Refactor existing orchestration into the shared core and invoke new metadata only after independent discovery and wholly body-only mapping succeed.** Time/log `go_impact_phase=package_inventory` separately. Validate package evidence and expanded committed files before using Task 4. Check source identity/build agreement before and after relevant work and retain the existing final clean/revision check. Each native invocation gets its own configured timeout; metadata failure never resets discovery or falls through to empty success. Do not run graphs for empty diffs or otherwise bypass existing graph eligibility safeguards.
- [ ] **Step 4: Assemble schema-2 reports on every package-request path.** Populate effective context only from validated successful evidence. For invalid/unavailable metadata, execution lists are incomplete and module fallback is explicit; for validated evidence, list all known binaries even if a later graph contradiction causes broad fallback. Keep all affected binaries selected when no reduction exists. No-op is distinct from a validated empty execution inventory. Preserve base identities/stages/graph suggestions; do not present discarded conservative run-all reasons as package-exclusion reasons.
- [ ] **Step 5: Run `cargo test -p extractor-runtime --test go_package_impact`, `cargo test -p extractor-runtime --test go_impact_git`, and all prior Task 3–4 commands.** Expected PASS, including late source drift and legacy provider cases.
- [ ] **Step 6: Review gate.** Inspect every early return for honest completeness/context/run-all fields and input-error semantics; verify ordinary callers cannot accidentally enable metadata.
- [ ] **Step 7: Commit Task 5 files with explicit-path staging:** `git commit -m "feat: orchestrate opt-in Go package impact reports"`.

### Task 6: CLI opt-in, schema-2 publication and execution documentation

**Files:** Modify `cli/src/bin/go-test-impact.rs`, `cli/src/go_impact_output.rs`; create `cli/tests/go_package_impact_cli.rs`, `docs/go-package-selection.md`.

**Interfaces:**
- Consumes Task 5 public entrypoints and Task 3 policy/scope enums.
- Changes writer to `write_report_atomic<T: serde::Serialize>(output: &Path, report: &T) -> anyhow::Result<()>`; keep path validation/publication behavior unchanged.
- Adds `--selection-policy` and `--test-scope`; runtime options stay unchanged. Validate the flag combination before invoking providers.

- [ ] **Step 1: Add parser and subprocess tests.** Default -> schema 1/no metadata; packages without scope -> input error; conservative with scope -> input error; unknown scope/policy -> error; packages plus linked-binary -> schema 2. Assert successful subset arguments equal `["./service/a"]`, a root binary argument is `"."`, eligible none-selected equals `[]` and `run_all_tests == false`, and missing evidence gives incomplete inventory plus `run_all_tests == true`. Invalid `../x`, `a/...`, absolute or symlink-escaping directories cannot become exact execution instructions. Re-run existing atomic-output and deadline protections with both report shapes.
- [ ] **Step 2: Run `cargo test -p cli --test go_package_impact_cli`.** Expected FAIL for unsupported flags/schema 2.
- [ ] **Step 3: Implement Clap flag parsing and explicit dispatch to old/new entrypoints.** Match enum strings in Shared contract decisions, leave default timeout 300 seconds and positive-only validation unchanged. Serialize reports through the existing atomic writer; add no test execution to the CLI.
- [ ] **Step 4: Document scope exclusions, local scan limitations, unchanged default, whole-binary execution including examples/fuzz seeds, exact argument arrays and empty-set skip semantics in `docs/go-package-selection.md`.** Include single-line invocations, not fragile shell continuations. Explain that race/coverage/different build contexts are not certified and graph suggestions are not execution lists. Distinguish input rejection, broad fallback and valid all-selected package decisions.
- [ ] **Step 5: Run `cargo test -p cli --test go_package_impact_cli`, `cargo test -p cli --test go_test_impact_cli`, and `cargo test -p cli --bin go-test-impact`.** Expected PASS without loosening existing timeouts to hide fixture failures.
- [ ] **Step 6: Review gate.** Inspect actual default/v2 JSON and help output; confirm no interpolated command string, implicit scope or fabricated empty inventory.
- [ ] **Step 7: Commit Task 6 files:** `git add -- cli/src/bin/go-test-impact.rs cli/src/go_impact_output.rs cli/tests/go_package_impact_cli.rs docs/go-package-selection.md` then `git commit -m "feat: expose scoped Go package selection in CLI"`.

### Task 7: One-command real-project controls and final compatibility gate

**Files:** Create `scripts/run-go-package-selection-benchmarks.sh`, `scripts/evaluate-go-package-selection.py`, `scripts/test-go-package-selection-benchmarks.sh`; update `docs/go-package-selection.md` with verified results/commands.

**Interfaces:**
- Runner command: `scripts/run-go-package-selection-benchmarks.sh`.
- Inputs: optional `GO_BENCHMARK_ROOT` (default resolved `../../../benchmarks/go`), `TIA_BENCHMARK_OUTPUT_DIR` (default unique temporary directory), `GO_CALLGRAPH_BINARY` and `GO_ANALYSIS_TIMEOUT_SECONDS` (default 300). Build local tool/CLI if no executable override is supplied; never download benchmark dependencies automatically.
- Evaluator invocation: `python3 scripts/evaluate-go-package-selection.py --benchmark-root ROOT --output-dir OUTPUT --go-callgraph-binary BINARY --go-test-impact-binary BINARY --analysis-timeout-seconds N`.
- Outputs: `summary.json`, per-case baseline/candidate IDs, committed diffs, independent package inventory, analyzer JSON/stderr, baseline/mutant Go test JSON, selected/excluded binary identities, observed failing binaries and omitted-failing-binary count. Preserve failed-case artifacts; return nonzero for omissions or setup failures.

- [ ] **Step 1: Add control-script tests before implementation.** Use temporary Git fixtures and stub executables to prove the runner never edits originals, rejects missing projects/dependencies with an actionable preparation command, retains failed artifacts, rejects incomplete execution inventory, skips execution on `[]`, and passes exact packages/build context as argument arrays. Add an evaluator test fixture in `scripts/test-go-package-selection-benchmarks.sh` where an excluded binary fails: expected nonzero and `omitted_failing_binaries == 1`. Include output paths with spaces to pin quoting.
- [ ] **Step 2: Run `bash scripts/test-go-package-selection-benchmarks.sh`.** Expected FAIL for missing runner/evaluator.
- [ ] **Step 3: Implement isolated fixture creation and replay.** Copy/clone repositories into unique output-owned directories, verify clean pinned baselines and prepared offline dependencies, commit body-only mutations there, and preserve user repositories. Start go-micro-services at `f80124996af7180bb28ad3e290a84e941980a45f` and go-coffeeshop at `22d33190a0cf81630878e47bafae2de7c0abb783`; reject a missing revision rather than silently changing baselines. Establish passing baseline tests before judging each mutant. Use recorded environment/semantic build flags for subsequent tests; reject cross-platform contexts that cannot execute locally. External local-replacement fixtures remain unit controls, not edits to original benchmarks.
- [ ] **Step 4: Implement case definitions and independent observations in the evaluator.** Locate a unique existing body and verify its original text before mutation; abort on source drift. Cases: microservices profile body edit, geo distance multiplier `2 -> 200`, rate append omission, frontend date format edit, frontend HTTP status mutation, augmented geo sort-comparator helper reversal, coffeeshop NewID returning `uuid.Nil`, and uncovered AggregateRoot event-helper body edit. Augmentation belongs in a committed baseline before the callback mutation. Add separate configuration, signature, deletion and explicit-init controls requiring fallback. Use `go list -deps -test -json` and ordinary `go list -json ./...` active test-file metadata as independently collected observations; do not infer binaries by parsing `.test` suffixes. Execute complete test binaries through exact package arguments, plus an independent full-module mutant run to detect failing excluded binaries. Keep TestX counts distinct from binary counts and preserve surviving mutants as observations, not alleged failures.
- [ ] **Step 5: Run the fake runner control and the real one-command runner.** Require zero observed omitted failing binaries. Verify rather than assume that independent service edits exclude independent binaries, frontend edits retain all three frontend TestX, coffeeshop keeps both shared-binary TestX, and the callback binary remains selected even without CHA suggestions. Investigate any deviation before changing expectations; unsupported real-project local blockers may require a documented fallback, not removal of safety checks. Report reduction separately from mutation capture; no general soundness claim.
- [ ] **Step 6: Run final verification:** `cd go-callgraph && go test -race ./... && go vet ./...`; `scripts/run-go-callgraph-control.sh`; `scripts/test-go-native-adapter.sh`; `scripts/run-go-impact-control.sh`; `cargo fmt --all -- --check`; `cargo test --workspace`; `git diff --check`. Expected PASS, with default control still **7 discovered / 5 suggested / 7 selected**. Record actual suite counts and any existing ignored tests; do not claim success from a scheduled or partial run.
- [ ] **Step 7: Whole-branch review gate.** Review the diff against all eight spec acceptance gates and the five Review Focus cases. Recheck default wire compatibility, exclusion authority, real artifact evidence and all fallback paths. Fix findings and rerun affected checks before declaring completion. Preserve native execution; obtain an independent reviewer only under applicable review-skill instructions/authorization.
- [ ] **Step 8: Commit Task 7 files with exact-path staging:** `git commit -m "test: validate Go package selection on isolated real projects"`. Do not merge or push without a separate request.

## Plan self-review and handoff

Coverage: native graph/context and variants (Task 1), blockers/deadlines (Task 2), strict capability validation and old-wire preservation (Task 3), closure/whole-binary policy (Task 4), opt-in orchestration and completeness (Task 5), CLI/report/exact execution data (Task 6), empirical real-project controls and complete compatibility checks (Task 7). The five Review Focus cases each have an owning test gate above.

Implementation begins only after plan review. Preserve the user's native execution preference. At each task gate report verified outcomes and substantive findings; do not substitute graph-suggestion reduction for actual execution-package reduction.
