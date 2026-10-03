# Gradle Java TIA Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Automate Gradle Java bytecode preparation so the existing WALA test-root graph and conservative TIA CLI work for Groovy/Kotlin DSL Gradle modules.

**Architecture:** Extract shared prepared-bytecode result types and a preparer interface from the Maven adapter, then add a wrapper-first Gradle implementation which uses a temporary Groovy init script to query Gradle's Java source sets and `testRuntimeClasspath`. Extend module ownership mapping so the Rust CLI scopes Gradle changes correctly; all existing WALA selection and fallback behavior remains unchanged.

**Tech Stack:** Java 17, WALA 1.8.0, Gradle Java plugin/init scripts, Rust 2024, Tree-sitter Java, Clap.

**Spec:** `docs/superpowers/specs/2026-10-03-gradle-java-tia-design.md`

## Global Constraints

- Support `build.gradle` and `build.gradle.kts` without parsing project build scripts.
- Support conventional `src/main/java` and `src/test/java` Gradle Java modules, including multi-module builds.
- Prefer `gradlew`/`gradlew.bat`; only fall back to system `gradle` when no wrapper exists.
- `testClasses` and Gradle's `testRuntimeClasspath` are the required preparation inputs.
- Keep explicit `--classes-dir` and `--test-classes-dir` as the highest-priority, build-tool-independent inputs.
- Any Gradle uncertainty must yield the existing module-wide TIA fallback; it must never narrow to an empty test set.
- Do not implement framework-injected, reflective, dynamic-test, Android, or cross-service edges.
- Add purpose comments/Javadoc to every new production type and function.
- Make one focused commit per completed task on `feat/gradle-java-tia`.

## Review Focus

- A Kotlin DSL project must use the same init-script discovery as Groovy DSL; Task 2 owns this fixture.
- A nested module must execute `:nested:module:testClasses`, not the root task; Task 2 owns this fixture.
- A missing wrapper and system Gradle must return preparation failure, causing TIA fallback rather than source-graph narrowing; Task 2 owns this fixture.
- A Java/resource/POM-equivalent change below a Gradle module must use that module's test inventory, not the repository root; Task 3 owns this fixture.
- Explicit class/test directories must bypass both Maven and Gradle preparation; Task 4 owns this adapter test.

---

### Task 1: Extract shared bytecode-preparation contracts

**Files:**
- Create: `wala-callgraph/src/main/java/com/voyantclair/wala/BytecodeArtifacts.java`
- Create: `wala-callgraph/src/main/java/com/voyantclair/wala/BytecodeArtifactPreparer.java`
- Modify: `wala-callgraph/src/main/java/com/voyantclair/wala/MavenBytecodeArtifacts.java`
- Modify: `wala-callgraph/src/main/java/com/voyantclair/wala/WalaCallGraphMain.java`
- Modify: `wala-callgraph/src/test/java/com/voyantclair/wala/MavenBytecodeArtifactsTest.java`

**Interfaces:**
- Produces `BytecodeArtifacts.PreparedArtifacts(List<Path> applicationClassDirs, List<Path> testClassDirs, List<Path> dependencyJars, Path moduleRoot)` and `BytecodeArtifacts.Preparation(PreparedArtifacts artifacts, String diagnostic)`.
- Produces `BytecodeArtifactPreparer.prepare(Path sourceRoot) -> BytecodeArtifacts.Preparation`.
- `MavenBytecodeArtifacts` implements the interface; `WalaCallGraphMain` consumes only the shared contract.

- [ ] **Step 1: Write a failing Maven preparation test using the shared artifact result**

Assert the existing two-module Maven fixture still exposes main directories, test directories, dependencies, and its module root through `BytecodeArtifacts.PreparedArtifacts`.

- [ ] **Step 2: Run the focused test to verify it fails**

Run: `mvn -q -f wala-callgraph/pom.xml -Dtest=MavenBytecodeArtifactsTest test`

Expected: FAIL because the shared artifact contract does not exist.

- [ ] **Step 3: Add the shared records and preparer interface; migrate Maven preparation without behavior changes**

Keep Maven's current reactor build and test-scope classpath semantics. Remove nested artifact/result records only after all callers use the shared types.

- [ ] **Step 4: Run the focused test to verify it passes**

Run: `mvn -q -f wala-callgraph/pom.xml -Dtest=MavenBytecodeArtifactsTest test`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add wala-callgraph/src/main wala-callgraph/src/test
git commit -m "refactor: share bytecode preparation contracts"
```

### Task 2: Prepare Gradle Java test bytecode and classpaths

**Files:**
- Create: `wala-callgraph/src/main/java/com/voyantclair/wala/GradleBytecodeArtifacts.java`
- Create: `wala-callgraph/src/test/java/com/voyantclair/wala/GradleBytecodeArtifactsTest.java`

**Interfaces:**
- Implements `BytecodeArtifactPreparer.prepare(Path sourceRoot) -> BytecodeArtifacts.Preparation`.
- Detects a conventional Gradle module/root; invokes `<wrapper-or-gradle> <projectPath>:testClasses` and a temporary `-I` init script task named `voyantclairWalaArtifacts`.
- Parses exactly `VOYANTCLAIR_MAIN=`, `VOYANTCLAIR_TEST=`, and `VOYANTCLAIR_TEST_RUNTIME=` path-separated records.

- [ ] **Step 1: Write failing fixtures for Groovy DSL root, Kotlin DSL nested module, and unavailable Gradle**

Create executable fake `gradlew` fixtures that record requested task paths, create known `build/classes/java/main` and `build/classes/java/test` directories, and emit the three artifact records when invoked with the init script. Assert:

```java
assertTrue(GradleBytecodeArtifacts.prepare(sourceRoot).succeeded());
assertTrue(preparation.artifacts().testClassDirs().contains(testClasses));
assertEquals(":services:orders:testClasses", recordedTask);
```

For a module without wrapper or system Gradle, assert `succeeded()` is false and `diagnostic()` is nonblank.

- [ ] **Step 2: Run the focused test to verify it fails**

Run: `mvn -q -f wala-callgraph/pom.xml -Dtest=GradleBytecodeArtifactsTest test`

Expected: FAIL because Gradle preparation is absent.

- [ ] **Step 3: Implement wrapper-first Gradle preparation**

Use a temporary Groovy init script (deleted in `finally`) rather than reading either DSL. Locate nearest module build file and enclosing `settings.gradle`/`settings.gradle.kts`; derive `:`-separated Gradle project paths. Filter parsed artifacts to existing class directories/JARs and return structured failure for command, task, or record errors.

- [ ] **Step 4: Run the focused test to verify it passes**

Run: `mvn -q -f wala-callgraph/pom.xml -Dtest=GradleBytecodeArtifactsTest test`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add wala-callgraph/src/main/java/com/voyantclair/wala/GradleBytecodeArtifacts.java wala-callgraph/src/test/java/com/voyantclair/wala/GradleBytecodeArtifactsTest.java
git commit -m "feat: prepare Gradle Java test bytecode"
```

### Task 3: Route Gradle preparation and scope Git changes to Gradle modules

**Files:**
- Modify: `wala-callgraph/src/main/java/com/voyantclair/wala/WalaCallGraphMain.java`
- Modify: `wala-callgraph/src/test/java/com/voyantclair/wala/WalaCallGraphMainTest.java`
- Modify: `extractor-runtime/src/impact/git_diff.rs`
- Modify: `extractor-runtime/tests/impact_git_diff.rs`

**Interfaces:**
- `WalaCallGraphMain.runPreferred(...)` chooses Maven or Gradle preparation from source-root ownership and passes `PreparedArtifacts` unchanged into existing test-root WALA execution.
- `find_module_root(projectRoot, sourcePath) -> PathBuf` recognizes `pom.xml`, `build.gradle`, and `build.gradle.kts` ancestors.

- [ ] **Step 1: Write failing adapter and impact-mapping tests**

Assert a Gradle-owned conventional source root chooses `GradleBytecodeArtifacts` before source fallback. Add a temporary Git fixture with `build.gradle.kts`, a changed Java method, and a changed resource; assert both changed elements use the Gradle module absolute path.

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `mvn -q -f wala-callgraph/pom.xml -Dtest=WalaCallGraphMainTest test`

Run: `cargo test -p extractor-runtime --test impact_git_diff`

Expected: FAIL because preparation routing and Gradle module ownership are unsupported.

- [ ] **Step 3: Implement preparer routing and build-file module ownership**

Maven retains priority when a module has `pom.xml`; Gradle is selected otherwise. Preserve explicit class-dir bypass. Make module-root discovery use the closest ancestor containing any recognized Maven/Gradle build marker, so nested Gradle modules win over their root build.

- [ ] **Step 4: Run the focused tests to verify they pass**

Run: `mvn -q -f wala-callgraph/pom.xml -Dtest=WalaCallGraphMainTest,GradleBytecodeArtifactsTest test`

Run: `cargo test -p extractor-runtime --test impact_git_diff`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add wala-callgraph/src/main/java/com/voyantclair/wala/WalaCallGraphMain.java wala-callgraph/src/test/java/com/voyantclair/wala/WalaCallGraphMainTest.java extractor-runtime/src/impact/git_diff.rs extractor-runtime/tests/impact_git_diff.rs
git commit -m "feat: route Gradle Java TIA preparation"
```

### Task 4: Verify Gradle test-root graph reuse and document support

**Files:**
- Modify: `wala-callgraph/src/test/java/com/voyantclair/wala/BytecodeCallGraphTest.java`
- Modify: `cli/tests/test_impact_cli.rs`
- Modify: `docs/call_graph_providers.md`
- Modify: `docs/benchmarks/java-tia-baseline.md`

**Interfaces:**
- Gradle-prepared main/test class inputs are consumed by existing `BytecodeCallGraph.analyze(..., List<MethodSelector>, Algorithm)` without a second graph implementation.
- The TIA CLI continues to output `TestImpactResult`; Gradle preparation failure has `ModuleFallback`/`AnalysisFailure`, never a narrowed empty result.

- [ ] **Step 1: Write failing graph-reuse and CLI fallback tests**

Compile a known `Service`/`ExampleTest` fixture into Gradle-shaped class directories and assert `ExampleTest.testChanged → Service.changed` appears with an explicit selector. Add a Git/Gradle fixture whose fake wrapper fails `testClasses`; assert the CLI writes JSON selecting both discovered module tests with a fallback reason.

- [ ] **Step 2: Run the tests to verify current behavior fails**

Run: `mvn -q -f wala-callgraph/pom.xml -Dtest=BytecodeCallGraphTest test`

Run: `cargo test -p cli --test test_impact_cli`

Expected: the graph fixture passes only after Gradle inputs are correctly routed; the CLI fallback fixture fails until Gradle module ownership/preparation is integrated.

- [ ] **Step 3: Finish integration only where test evidence exposes a gap**

Do not add Gradle-specific call-graph logic. Use the artifact directories/dependencies from Task 2 and preserve `test_root_preparation_failed` diagnostics for failed test-root preparation.

- [ ] **Step 4: Update provider and benchmark documentation**

Document wrapper-first behavior, supported conventional Java layouts, Groovy/Kotlin DSL compatibility, explicit artifact inputs, and the fallback/unsupported-framework limits. Record direct-edge and failed-preparation fixture outcomes.

- [ ] **Step 5: Run full verification**

Run:

```bash
cargo fmt --check
cargo test -p models
cargo test -p extractor-runtime
cargo test -p cli
mvn -q -f wala-callgraph/pom.xml test
```

Expected: all commands PASS.

- [ ] **Step 6: Commit**

```bash
git add wala-callgraph cli docs
git commit -m "docs: evaluate Gradle Java TIA support"
```
