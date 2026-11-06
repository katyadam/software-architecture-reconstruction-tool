# Java TIA Module Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Use declared Maven and Gradle module metadata to map Java changes and discover descendant tests conservatively.

**Architecture:** Add a shared `ModuleCatalog` to `extractor-runtime::impact`. It exposes complete declared modules or an uncertainty state that preserves the current marker-recursive behavior and produces a result diagnostic. Git change mapping and Java test discovery both consume the same catalog, avoiding divergent ownership decisions.

**Tech Stack:** Rust 2024, `quick-xml` for Maven POM parsing, Tree-sitter Java, existing Git CLI integration, Rust integration tests with temporary fixtures.

**Spec:** `docs/superpowers/specs/2026-10-06-java-tia-module-metadata-design.md`

## Global Constraints

- Support literal Maven `<modules>` entries and Gradle `include`/literal `projectDir` declarations only.
- Do not invoke Maven or Gradle while constructing module metadata.
- Dynamic, malformed, missing, conflicting, or project-root-escaping metadata must broaden to the enclosing root and emit a diagnostic.
- CLI arguments and JSON schema remain unchanged; append diagnostics only.
- Preserve recursive marker discovery only for an uncertain catalog, never for a complete catalog.

## Review Focus

- Maven module paths containing `..` or escaping the project root must produce uncertainty; cover in Task 2.
- Gradle `projectDir` override must own changes and tests at its mapped directory; cover in Task 3.
- Unsupported Gradle interpolation or dynamic include must select root scope and record a diagnostic; cover in Task 3.
- A complete catalog must exclude an unlisted nested `pom.xml`/build file; cover in Task 4.
- Root plus declared child changes must still deduplicate child tests; cover in Task 4.

---

### Task 1: Shared module-catalog contract

**Files:**
- Create: `extractor-runtime/src/impact/java_modules.rs`
- Modify: `extractor-runtime/src/impact/mod.rs`
- Modify: `extractor-runtime/Cargo.toml`
- Modify: `Cargo.toml`
- Test: `extractor-runtime/src/impact/java_modules.rs`

**Interfaces:**
- Produces: `pub struct ModuleCatalog`, `pub fn load(project_root: &Path) -> ModuleCatalog`, `pub fn owner_for(&self, source_path: &str) -> PathBuf`, `pub fn descendants_of(&self, module_root: &Path) -> Vec<PathBuf>`, and `pub fn diagnostic(&self) -> Option<&str>`.
- Consumes: project-root paths; no callers change in this task.

- [ ] **Step 1: Write failing catalog tests for a single-module project and uncertain root fallback**

```rust
assert_eq!(catalog.owner_for("src/main/java/example/Service.java"), root);
assert_eq!(catalog.descendants_of(&root), vec![root]);
assert!(catalog.diagnostic().is_some());
```

- [ ] **Step 2: Run the new module-catalog tests to verify they fail**

Run: `cargo test -p extractor-runtime impact::java_modules::tests`

Expected: FAIL because `java_modules` and `ModuleCatalog` do not exist.

- [ ] **Step 3: Implement the `ModuleCatalog` complete/uncertain state and root-constrained path helpers**

Use normalized absolute paths. Add `quick-xml` as a workspace dependency now; Maven parsing is introduced in Task 2.

- [ ] **Step 4: Run the module-catalog tests to verify they pass**

Run: `cargo test -p extractor-runtime impact::java_modules::tests`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock extractor-runtime/Cargo.toml extractor-runtime/src/impact/mod.rs extractor-runtime/src/impact/java_modules.rs
git commit -m "feat: add Java TIA module catalog"
```

### Task 2: Maven reactor metadata parser

**Files:**
- Modify: `extractor-runtime/src/impact/java_modules.rs`
- Test: `extractor-runtime/src/impact/java_modules.rs`

**Interfaces:**
- Consumes: `ModuleCatalog::load(project_root)` from Task 1.
- Produces: complete catalog entries from recursively declared Maven reactor modules, or an uncertainty diagnostic.

- [ ] **Step 1: Write failing Maven catalog tests**

```rust
assert_eq!(catalog.owner_for("orders/src/main/java/example/Order.java"), root.join("orders"));
assert_eq!(catalog.descendants_of(&root), vec![root, root.join("orders")]);
assert!(catalog.owner_for("unlisted/src/main/java/example/Hidden.java") == root);
```

Add a malformed/property-expanded/escaping `<module>` fixture asserting `catalog.diagnostic().is_some()` and root ownership.

- [ ] **Step 2: Run the Maven catalog tests to verify they fail**

Run: `cargo test -p extractor-runtime impact::java_modules::tests::maven`

Expected: FAIL because Maven module declarations are not parsed.

- [ ] **Step 3: Implement literal Maven `<modules>` parsing and declared-child recursion**

Use `quick-xml`; accept only text-only literal module paths. Treat profiles, `${…}`, XML errors, missing directories, and paths outside `project_root` as uncertainty.

- [ ] **Step 4: Run the Maven catalog tests to verify they pass**

Run: `cargo test -p extractor-runtime impact::java_modules::tests::maven`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add extractor-runtime/src/impact/java_modules.rs Cargo.lock
git commit -m "feat: read Maven reactor modules for TIA"
```

### Task 3: Gradle settings metadata parser

**Files:**
- Modify: `extractor-runtime/src/impact/java_modules.rs`
- Test: `extractor-runtime/src/impact/java_modules.rs`

**Interfaces:**
- Consumes: `ModuleCatalog` complete state and normalized-path helpers from Tasks 1–2.
- Produces: complete Gradle project directory entries from literal `settings.gradle[.kts]` declarations, or uncertainty diagnostics.

- [ ] **Step 1: Write failing Gradle catalog tests**

```rust
assert_eq!(catalog.owner_for("services/orders/src/main/java/example/Order.java"), root.join("services/orders"));
assert_eq!(catalog.owner_for("custom/payment/src/main/java/example/Payment.java"), root.join("custom/payment"));
```

Cover Groovy and Kotlin literal `include` forms, a literal `projectDir` override, and a variable-interpolated include that asserts uncertainty/root ownership.

- [ ] **Step 2: Run the Gradle catalog tests to verify they fail**

Run: `cargo test -p extractor-runtime impact::java_modules::tests::gradle`

Expected: FAIL because Gradle settings are not parsed.

- [ ] **Step 3: Implement a literal-only settings parser**

Recognize only colon project paths in `include` calls and literal `project(...).projectDir = file(...)` assignments. Reject dynamic expressions, duplicate/conflicting project paths, and root-escaping mappings as uncertainty.

- [ ] **Step 4: Run the Gradle catalog tests to verify they pass**

Run: `cargo test -p extractor-runtime impact::java_modules::tests::gradle`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add extractor-runtime/src/impact/java_modules.rs
git commit -m "feat: read Gradle project metadata for TIA"
```

### Task 4: Integrate catalog ownership, discovery, and diagnostics

**Files:**
- Modify: `extractor-runtime/src/impact/git_diff.rs`
- Modify: `extractor-runtime/src/impact/java_changes.rs`
- Modify: `extractor-runtime/src/impact/java_tests.rs`
- Modify: `extractor-runtime/src/impact/mod.rs`
- Modify: `extractor-runtime/tests/impact_git_diff.rs`
- Modify: `extractor-runtime/tests/impact_java_tests.rs`
- Modify: `cli/src/bin/test-impact.rs`
- Modify: `cli/tests/test_impact_cli.rs`

**Interfaces:**
- Consumes: `ModuleCatalog` from Tasks 1–3.
- Produces: `ChangeAnalysis { changes: Vec<ChangedElement>, diagnostics: Vec<String>, catalog: ModuleCatalog }` from `analyze_changes`, and catalog-aware `discover_java_tests`.

- [ ] **Step 1: Write failing integration tests for declared-only descendants and uncertain diagnostics**

```rust
assert_eq!(changed[0].module_root, root.join("orders").to_string_lossy());
assert!(!tests.iter().any(|test| test.module_root.contains("unlisted")));
assert!(result.diagnostics.iter().any(|d| d.contains("module metadata")));
```

Extend the existing reactor CLI test with an unlisted nested module and verify root-plus-child changes still select each declared test once.

- [ ] **Step 2: Run the affected integration tests to verify they fail**

Run: `cargo test -p extractor-runtime --test impact_git_diff --test impact_java_tests && cargo test -p cli --test test_impact_cli`

Expected: FAIL because callers still use marker walking and diagnostics are unavailable.

- [ ] **Step 3: Thread one catalog through change analysis and test discovery**

Replace direct `find_module_root` calls with `catalog.owner_for`. On a complete catalog, enumerate declared descendants only; on uncertainty, retain existing marker-recursive discovery. Append the catalog diagnostic to `TestImpactResult` exactly once.

- [ ] **Step 4: Run integration tests to verify they pass**

Run: `cargo test -p extractor-runtime --test impact_git_diff --test impact_java_tests && cargo test -p cli --test test_impact_cli`

Expected: PASS.

- [ ] **Step 5: Run complete verification**

Run: `cargo fmt --all && cargo test --workspace && git diff --check`

Expected: all tests pass and no whitespace errors.

- [ ] **Step 6: Commit**

```bash
git add extractor-runtime/src/impact extractor-runtime/tests cli/src/bin/test-impact.rs cli/tests/test_impact_cli.rs
git commit -m "feat: use declared Java build modules in TIA"
```

## Self-Review

- Spec coverage: Tasks 2 and 3 cover each build tool; Task 4 covers shared ownership, declared-only discovery, diagnostics, and fallback behavior. WALA is intentionally unchanged as required.
- Step scan: every task starts with a targeted red test, defines its output interface, and ends with a focused verification and commit.
- Type consistency: `ModuleCatalog` is created in Task 1, populated in Tasks 2–3, and consumed through `ChangeAnalysis` in Task 4.
- Review focus: all five specified edge classes have an owning task and explicit test requirement.
- Proportion: the plan defines contracts and test cases without prescribing parser internals beyond the design's literal-only rules.
