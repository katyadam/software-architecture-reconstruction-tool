# Conservative Java TIA Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Select a conservative set of JUnit tests affected by differences between a Git baseline revision and the checked-out Maven Java candidate revision.

**Architecture:** A new impact-analysis layer converts Git zero-context diff ranges into callable-, class-, or module-level changed elements using Tree-sitter Java. It discovers JUnit tests, requests a WALA graph rooted at their compiled methods, and traverses that graph backwards from changed callables. Missing evidence always escalates selection to the complete affected-module test inventory.

**Tech Stack:** Rust 2024, Tree-sitter Java, Clap, Git CLI, Maven, Java 17, WALA 1.8.0, JUnit 5.

**Spec:** `docs/superpowers/specs/2026-10-03-java-tia-baseline-design.md`

## Global Constraints

- The candidate revision is the `HEAD` checked out under `--project-dir`; baseline is any Git revision accepted by `git diff`.
- Automate Maven only; Gradle remains explicit-input support in the WALA adapter.
- Support JUnit 4 `@Test` and JUnit 5 `@Test`, `@ParameterizedTest`, `@RepeatedTest`, and `@TestFactory`.
- Select module-wide tests for deleted/renamed/resource/POM/configuration/unmapped changes and all analysis uncertainty.
- Do not add Spring DI, proxy, endpoint, listener, reflection, or cross-service inference in this milestone.
- Preserve provider-neutral call-graph JSON schema compatibility.
- Add purpose comments/Javadoc for every new production function and type.
- Make one focused commit per completed task on `feat/java-tia-baseline`.

## Review Focus

- A deleted Java source file must select its prior Maven module’s complete test inventory; Task 2 owns this test.
- A diff hunk outside a method body must become a class-level change rather than disappear; Task 2 owns this test.
- An overloaded test method must be identified by class plus descriptor, not by method name alone; Task 3 owns this test.
- A test-class compilation/classpath failure must select the module fallback and retain the failure diagnostic; Task 4 owns this test.
- A changed resource/POM or unresolved call-graph mapping must never return a narrowed empty test set; Task 5 owns these tests.

---

### Task 1: Add provider-neutral impact-analysis contracts

**Files:**
- Create: `models/src/impact.rs`
- Modify: `models/src/lib.rs`
- Test: `models/src/impact.rs`

**Interfaces:**
- Produces `ChangedElement`, `ChangedElementKind`, `JavaTestCase`, `SelectionReason`, `SelectedTest`, and `TestImpactResult` for Tasks 2–6.
- `ChangedElement` has `module_root: String`, `source_path: String`, `kind: ChangedElementKind`, and `callable_signature: Option<String>`.
- `JavaTestCase` has `module_root: String`, `class_name: String`, `method_name: String`, `descriptor: String`, `callable_signature: String`, and `source_path: String`.
- `SelectionReason` distinguishes `ChangedCallable`, `ChangedClass`, `ModuleFallback`, and `AnalysisFailure` and carries a diagnostic string.

- [ ] **Step 1: Write failing serialization and equality tests for a callable change and a fallback-selected test**

Assert a `TestImpactResult` round-trips through JSON and preserves the selected test’s descriptor and `ModuleFallback` reason.

- [ ] **Step 2: Run the model test to verify it fails**

Run: `cargo test -p models impact`

Expected: FAIL because the `impact` module and public types do not exist.

- [ ] **Step 3: Implement the contracts in `models/src/impact.rs` and re-export them from `models/src/lib.rs`**

Derive `Debug`, `Clone`, `PartialEq`, `Eq`, `Serialize`, and `Deserialize`; use `snake_case` serialization for enums.

- [ ] **Step 4: Run the model test to verify it passes**

Run: `cargo test -p models impact`

Expected: PASS.

- [ ] **Step 5: Commit the contract task**

```bash
git add models/src/impact.rs models/src/lib.rs
git commit -m "feat: add test impact result contracts"
```

### Task 2: Map Git changes to conservative Java impact elements

**Files:**
- Create: `extractor-runtime/src/impact/mod.rs`
- Create: `extractor-runtime/src/impact/git_diff.rs`
- Create: `extractor-runtime/src/impact/java_changes.rs`
- Modify: `extractor-runtime/src/lib.rs`
- Modify: `extractor-runtime/Cargo.toml`
- Test: `extractor-runtime/tests/impact_git_diff.rs`

**Interfaces:**
- Consumes `ChangedElement` from Task 1 and a checked-out Git project path.
- Produces `analyze_changes(project_root: &Path, baseline_revision: &str) -> Result<Vec<ChangedElement>, ImpactError>`.
- `git_diff` must combine `git diff --name-status --find-renames <baseline> HEAD` with `git diff --unified=0 --find-renames <baseline> HEAD` and preserve old/new paths and candidate line ranges.
- `java_changes` must map a candidate Java range to the smallest enclosing Tree-sitter `method_declaration`; a range outside a method maps to the enclosing class; all non-Java or non-localizable changes map to a Maven module.

- [ ] **Step 1: Write failing fixture tests for method, class, deleted-file, renamed-file, resource, and POM changes**

Use a temporary Git repository with a Maven module. Assert a method hunk yields `Callable`, a field/annotation hunk yields `Class`, and deleted/renamed/resource/POM paths yield `Module` for the expected module root.

- [ ] **Step 2: Run the impact diff test to verify it fails**

Run: `cargo test -p extractor-runtime --test impact_git_diff`

Expected: FAIL because `analyze_changes` does not exist.

- [ ] **Step 3: Implement `ImpactError`, Git diff parsing, module discovery, and Tree-sitter range ownership**

Add direct Tree-sitter dependencies to this crate. Read candidate Java source from the checked-out project only. Require candidate `HEAD` to be the analysis candidate; use deleted/old paths only to locate the owning Maven module before emitting `Module`.

- [ ] **Step 4: Run the impact diff test to verify it passes**

Run: `cargo test -p extractor-runtime --test impact_git_diff`

Expected: PASS.

- [ ] **Step 5: Commit the change-mapping task**

```bash
git add models extractor-runtime/src/impact extractor-runtime/src/lib.rs extractor-runtime/tests/impact_git_diff.rs
git commit -m "feat: map Git changes to Java impact elements"
```

### Task 3: Discover stable JUnit test entry points

**Files:**
- Create: `extractor-runtime/src/impact/java_tests.rs`
- Modify: `extractor-runtime/Cargo.toml`
- Test: `extractor-runtime/tests/impact_java_tests.rs`

**Interfaces:**
- Consumes a Maven module root and source files below `src/test/java`.
- Produces `discover_java_tests(module_root: &Path) -> Result<Vec<JavaTestCase>, ImpactError>`.
- Each `JavaTestCase.class_name` is the JVM internal name derived from its package declaration and enclosing classes. Its `descriptor` uses JVM method-descriptor syntax, derived from Tree-sitter parameter types; no-argument JUnit methods use `()V`.
- The WALA-facing selector is `class_name + "#" + method_name + descriptor`.

- [ ] **Step 1: Write failing discovery tests for JUnit 4, JUnit 5, parameterized, repeated, factory, helper, and overloaded methods**

Assert annotation-bearing methods are emitted, unannotated helpers are excluded, and overloaded methods preserve different descriptors.

- [ ] **Step 2: Run the test-discovery test to verify it fails**

Run: `cargo test -p extractor-runtime --test impact_java_tests`

Expected: FAIL because `discover_java_tests` does not exist.

- [ ] **Step 3: Implement Tree-sitter JUnit annotation and descriptor discovery**

Recognize fully-qualified and imported annotation spellings by terminal annotation name. Derive the JVM internal class name from the source package declaration. Emit diagnostics and no narrowed selection when a parameter type cannot be represented safely.

- [ ] **Step 4: Run the test-discovery test to verify it passes**

Run: `cargo test -p extractor-runtime --test impact_java_tests`

Expected: PASS.

- [ ] **Step 5: Commit the test-discovery task**

```bash
git add extractor-runtime/src/impact/java_tests.rs extractor-runtime/tests/impact_java_tests.rs
git commit -m "feat: discover Java test entry points"
```

### Task 4: Analyze Maven test roots with WALA

**Files:**
- Modify: `wala-callgraph/src/main/java/com/voyantclair/wala/MavenBytecodeArtifacts.java`
- Modify: `wala-callgraph/src/main/java/com/voyantclair/wala/BytecodeCallGraph.java`
- Modify: `wala-callgraph/src/main/java/com/voyantclair/wala/WalaCallGraphMain.java`
- Modify: `wala-callgraph/src/test/java/com/voyantclair/wala/MavenBytecodeArtifactsTest.java`
- Modify: `wala-callgraph/src/test/java/com/voyantclair/wala/BytecodeCallGraphTest.java`
- Modify: `extractor-runtime/src/call_graph/wala.rs`
- Modify: `extractor-runtime/tests/wala_provider.rs`

**Interfaces:**
- `MavenBytecodeArtifacts.PreparedArtifacts` gains `testClassDirs: List<Path>` and returns test-scope dependency JARs after Maven `test-compile`.
- Adapter CLI accepts repeatable `--test-classes-dir` and `--entrypoint <internal-class>#<method><descriptor>`.
- `BytecodeCallGraph.analyze(..., List<MethodSelector> entrypoints, Algorithm algorithm)` uses supplied selectors in addition to conventional `main` methods.
- Rust exposes `WalaJavaProvider::analyze_test_roots(source_root, &[JavaTestCase]) -> CallGraphOutcome` and treats adapter/preparation failure as an impact-layer fallback trigger.

- [ ] **Step 1: Write failing Java and Rust tests for a JUnit test calling an application method**

Compile a Maven fixture with `src/main/java` and `src/test/java`; assert test classes and test-scope JARs are prepared and the graph contains `ExampleTest.testMethod → Service.changedMethod`. Add a provider test proving selectors are serialized as repeated adapter arguments.

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `mvn -q -f wala-callgraph/pom.xml -Dtest=MavenBytecodeArtifactsTest,BytecodeCallGraphTest test`

Run: `cargo test -p extractor-runtime wala_provider`

Expected: FAIL because test bytecode and explicit WALA entrypoint selectors are unsupported.

- [ ] **Step 3: Implement Maven test preparation and explicit WALA method selectors**

Build the selected reactor module with `test-compile`, collect both class directory kinds, use Maven test scope for dependency classpath, and add only uniquely resolved supplied selectors as WALA entry points. Retain CHA as default and report unmatched selectors as diagnostics.

- [ ] **Step 4: Run the focused tests to verify they pass**

Run: `mvn -q -f wala-callgraph/pom.xml -Dtest=MavenBytecodeArtifactsTest,BytecodeCallGraphTest test`

Run: `cargo test -p extractor-runtime wala_provider`

Expected: PASS.

- [ ] **Step 5: Commit the WALA test-root task**

```bash
git add wala-callgraph extractor-runtime/src/call_graph/wala.rs extractor-runtime/tests/wala_provider.rs
git commit -m "feat: analyze Maven Java test roots with WALA"
```

### Task 5: Select affected tests with mandatory module fallback

**Files:**
- Create: `extractor-runtime/src/impact/selection.rs`
- Test: `extractor-runtime/tests/impact_selection.rs`

**Interfaces:**
- Consumes `Vec<ChangedElement>`, `Vec<JavaTestCase>`, `CallGraphOutcome`, and resolved call edges.
- Produces `select_tests(changes, tests, outcome, resolved_edges) -> TestImpactResult`.
- Callable changes traverse reverse edges to test callable signatures. Class changes use all module callables sharing the changed class. Module changes, failed/non-OK WALA outcomes, diagnostics for unresolved mapping, and missing test entrypoints select the complete module test inventory.

- [ ] **Step 1: Write failing selection tests for direct reachability and every fallback condition**

Assert only the reaching test is selected for a direct callable edge. Assert class, deleted/configuration/POM, failed WALA, and unresolved mapping each select all tests in the module with the correct reason.

- [ ] **Step 2: Run the selector test to verify it fails**

Run: `cargo test -p extractor-runtime --test impact_selection`

Expected: FAIL because `select_tests` does not exist.

- [ ] **Step 3: Implement reverse graph traversal and module-scoped fallback**

Deduplicate `SelectedTest` by its stable test identity, retain every applicable reason, and never emit an empty narrowed selection for a module containing discovered tests when analysis is uncertain.

- [ ] **Step 4: Run the selector test to verify it passes**

Run: `cargo test -p extractor-runtime --test impact_selection`

Expected: PASS.

- [ ] **Step 5: Commit the selection task**

```bash
git add extractor-runtime/src/impact/selection.rs extractor-runtime/tests/impact_selection.rs
git commit -m "feat: select Java tests conservatively"
```

### Task 6: Provide a standalone TIA CLI workflow

**Files:**
- Create: `cli/src/bin/test-impact.rs`
- Create: `cli/tests/test_impact_cli.rs`
- Modify: `cli/Cargo.toml`
- Modify: `docs/call_graph_providers.md`

**Interfaces:**
- CLI command: `cargo run -p cli --bin test-impact -- --project-dir <candidate-checkout> --baseline-revision <revision> --wala-adapter-jar <jar> --output <file> [--wala-timeout-seconds <seconds>]`.
- The command writes one pretty JSON `TestImpactResult` and returns nonzero only for invalid input or Git invocation failure; analysis uncertainty is represented in the result, not as an omitted output.

- [ ] **Step 1: Write a failing CLI integration test for a two-commit Maven fixture**

Create a temporary repository where one application method changes. Assert the output JSON selects the one reaching JUnit test and includes `ChangedCallable` as its reason.

- [ ] **Step 2: Run the CLI integration test to verify it fails**

Run: `cargo test -p cli --test test_impact_cli`

Expected: FAIL because the `test-impact` binary does not exist.

- [ ] **Step 3: Implement argument validation, orchestration, JSON output, and diagnostics forwarding**

Require `project_dir/.git`, validate baseline with `git rev-parse`, confirm the candidate is checked-out `HEAD`, discover Maven modules/tests, invoke WALA per affected module, and persist the result even when module fallback occurs.

- [ ] **Step 4: Run the CLI integration test to verify it passes**

Run: `cargo test -p cli --test test_impact_cli`

Expected: PASS.

- [ ] **Step 5: Commit the CLI task**

```bash
git add cli docs/call_graph_providers.md
git commit -m "feat: add conservative Java test impact CLI"
```

### Task 7: Verify the complete baseline and record benchmark metrics

**Files:**
- Create: `docs/benchmarks/java-tia-baseline.md`
- Modify: `docs/call_graph_providers.md`
- Test: `cli/tests/test_impact_cli.rs`

**Interfaces:**
- Consumes the Task 6 CLI and Maven fixture repository.
- Produces documented affected-test recall, selected/total test count, and elapsed analysis time for every controlled version pair.

- [ ] **Step 1: Add failing version-pair cases for a deletion and configuration change**

Extend the fixture repository so these cases must select the full module test set, demonstrating that non-callable changes cannot be silently narrowed.

- [ ] **Step 2: Run the complete TIA fixture suite to verify the new cases fail**

Run: `cargo test -p cli --test test_impact_cli`

Expected: FAIL until the fixture assertions and reporting cover the required fallback reasons.

- [ ] **Step 3: Implement benchmark report generation and user documentation**

Document fixture revision pairs, affected-test recall, selection reduction, WALA/Maven timing, and the explicit unsupported-framework limitations. Do not claim production-level recall from controlled fixtures.

- [ ] **Step 4: Run complete verification**

Run: `cargo fmt --check`

Run: `cargo test -p models`

Run: `cargo test -p extractor-runtime`

Run: `cargo test -p cli`

Run: `mvn -q -f wala-callgraph/pom.xml test`

Expected: all commands PASS.

- [ ] **Step 5: Commit the verification and benchmark task**

```bash
git add cli docs
git commit -m "docs: evaluate Java TIA baseline"
```
