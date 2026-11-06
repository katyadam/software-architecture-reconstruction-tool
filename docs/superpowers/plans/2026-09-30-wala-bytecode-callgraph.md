# WALA Bytecode Call-Graph Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Analyze Maven-built Java bytecode with WALA so modern Java services produce call graphs without relying on WALA's limited source parser.

**Architecture:** The standalone adapter prepares Maven build artifacts, then places application class directories, dependency JARs, and the Java runtime in a binary WALA analysis scope. It keeps the existing JSON protocol and Rust integration unchanged. Bytecode analysis is preferred whenever class outputs are available; the existing source implementation remains a non-fatal fallback.

**Tech Stack:** Java 17, Maven, WALA 1.8.0, JUnit 5, Maven Shade, Rust adapter client.

**Spec:** `docs/superpowers/specs/2026-09-30-wala-bytecode-callgraph-design.md`

## Global Constraints

- Preserve the versioned `CallGraphResult` JSON contract and `wala-java` provider ID.
- Never modify analyzed project source files; Maven may create `target/` outputs and local-cache artifacts.
- Support conventional Maven roots at `<module>/src/main/java` first.
- Use Maven `-pl <module> -am install -DskipTests` to build the selected module and required reactor modules.
- Add Java runtime classes to the primordial scope, application output directories to the application scope, and dependency JARs to the extension scope.
- Discover every `public static void main(String[])` entry point in application bytecode.
- Emit only edges whose caller is in an application class directory.
- Return `ok`, `no_entrypoints`, or `failed` JSON for every outcome; do not throw stack traces through stdout.
- Preserve source analysis only as a non-fatal fallback when bytecode artifacts cannot be prepared.
- Comment every new function with its purpose.

## Review Focus

- A source tree containing Java method references must use bytecode and not trigger ECJ's `ExpressionMethodReference` error (Task 3).
- A Maven reactor sibling module must be built and visible as an application class directory (Task 2).
- A missing Maven executable, failed build, or absent class output must yield a structured `failed` result (Tasks 2 and 3).
- Framework/JDK edges must not overwhelm output: only application callers may be emitted (Task 1).
- Explicit `--classes-dir` and `--classpath` inputs must work without Maven, enabling Gradle/CI artifacts (Task 3).

---

## File Structure

- `wala-callgraph/src/main/java/com/voyantclair/wala/BytecodeCallGraph.java` — builds binary WALA scopes, finds bytecode entry points, and converts application-originating edges.
- `wala-callgraph/src/main/java/com/voyantclair/wala/MavenBytecodeArtifacts.java` — discovers conventional Maven module/reactor layouts, invokes Maven, and returns application output directories plus dependency JARs.
- `wala-callgraph/src/main/java/com/voyantclair/wala/WalaCallGraphMain.java` — parses artifact CLI arguments, selects bytecode first, and retains source fallback/error serialization.
- `wala-callgraph/src/test/java/com/voyantclair/wala/BytecodeCallGraphTest.java` — real compiled-class/JAR binary-analysis coverage.
- `wala-callgraph/src/test/java/com/voyantclair/wala/MavenBytecodeArtifactsTest.java` — temporary Maven reactor preparation and failure-path coverage.
- `wala-callgraph/src/test/java/com/voyantclair/wala/WalaCallGraphMainTest.java` — public CLI/backend-selection behavior.
- `docs/call_graph_providers.md` — explains bytecode prerequisites and explicit Gradle inputs.

### Task 1: Build a binary WALA call-graph engine

**Files:**
- Create: `wala-callgraph/src/main/java/com/voyantclair/wala/BytecodeCallGraph.java`
- Create: `wala-callgraph/src/test/java/com/voyantclair/wala/BytecodeCallGraphTest.java`

**Interfaces:**
- Produces `BytecodeCallGraph.analyze(List<Path> applicationClassDirs, List<Path> dependencyJars) -> CallGraphResult`.
- Consumes a non-empty list of existing class directories and readable dependency JARs.

- [ ] **Step 1: Write the failing binary dispatch test**

Compile a temporary Java 8 application class directory containing `Main`, `Worker`, and `Impl`; invoke `BytecodeCallGraph.analyze(List.of(classesDir), List.of())`; assert `ok` and an edge from `LMain.main` to `LImpl.run`.

- [ ] **Step 2: Write the application-edge filter test**

Use the same fixture and assert every emitted edge has a caller whose declaring type begins with the fixture application package; assert no `Ljava/` caller appears.

- [ ] **Step 3: Verify RED**

Run: `mvn -q -f wala-callgraph/pom.xml -Dtest=BytecodeCallGraphTest test`

Expected: FAIL because `BytecodeCallGraph` does not exist.

- [ ] **Step 4: Implement `BytecodeCallGraph.analyze`**

Create `AnalysisScope.createJavaAnalysisScope()`, add the configured runtime JAR to `ClassLoaderReference.Primordial`, each `BinaryDirectoryTreeModule` to `ClassLoaderReference.Application`, and each JAR to `ClassLoaderReference.Extension`. Build the class hierarchy, discover all application `main` methods with descriptor `([Ljava/lang/String;)V`, run `ZeroOneContainerCFABuilderFactory`, and emit only application-caller edges with algorithm `zero_one_container_cfa_bytecode`.

- [ ] **Step 5: Verify GREEN**

Run: `mvn -q -f wala-callgraph/pom.xml -Dtest=BytecodeCallGraphTest test`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add wala-callgraph/src/main/java/com/voyantclair/wala/BytecodeCallGraph.java wala-callgraph/src/test/java/com/voyantclair/wala/BytecodeCallGraphTest.java
git commit -m "feat: add WALA bytecode call graph engine"
```

### Task 2: Prepare Maven reactor bytecode artifacts

**Files:**
- Create: `wala-callgraph/src/main/java/com/voyantclair/wala/MavenBytecodeArtifacts.java`
- Create: `wala-callgraph/src/test/java/com/voyantclair/wala/MavenBytecodeArtifactsTest.java`

**Interfaces:**
- Produces `MavenBytecodeArtifacts.prepare(Path sourceRoot) -> PreparedArtifacts`.
- `PreparedArtifacts` contains `List<Path> applicationClassDirs`, `List<Path> dependencyJars`, and the selected module root.

- [ ] **Step 1: Write the failing temporary-reactor test**

Create a temporary parent POM with `base` and `service` modules, where `service` depends on `base`. Call `prepare(service/src/main/java)` and assert both modules' `target/classes` directories are present and `service` can resolve its Maven dependency classpath.

- [ ] **Step 2: Write the failed-build test**

Create a conventional source-root layout with an invalid POM; assert `prepare` returns a typed failure carrying Maven's diagnostic rather than an empty artifact set.

- [ ] **Step 3: Verify RED**

Run: `mvn -q -f wala-callgraph/pom.xml -Dtest=MavenBytecodeArtifactsTest test`

Expected: FAIL because `MavenBytecodeArtifacts` does not exist.

- [ ] **Step 4: Implement Maven discovery and preparation**

Implement `findModulePom`, `findReactorPom`, and `moduleSelector`. For the direct-child conventional reactor layout, invoke `mvn -q -f <reactor-pom> -pl <module-selector> -am install -DskipTests`. Collect produced `target/classes` directories from built reactor modules and invoke Maven `dependency:build-classpath` for the selected module after installation. Split the platform classpath and keep readable JARs only. Return a typed preparation failure with captured process diagnostics.

- [ ] **Step 5: Verify GREEN**

Run: `mvn -q -f wala-callgraph/pom.xml -Dtest=MavenBytecodeArtifactsTest test`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add wala-callgraph/src/main/java/com/voyantclair/wala/MavenBytecodeArtifacts.java wala-callgraph/src/test/java/com/voyantclair/wala/MavenBytecodeArtifactsTest.java
git commit -m "feat: prepare Maven bytecode artifacts for WALA"
```

### Task 3: Select bytecode analysis from the adapter CLI

**Files:**
- Modify: `wala-callgraph/src/main/java/com/voyantclair/wala/WalaCallGraphMain.java`
- Modify: `wala-callgraph/src/test/java/com/voyantclair/wala/WalaCallGraphMainTest.java`

**Interfaces:**
- Consumes `--source-dir <path>`, optional repeatable `--classes-dir <path>`, and optional `--classpath <path-separated-jars>`.
- Produces existing `CallGraphResult` JSON with bytecode algorithm metadata whenever bytecode inputs are available.

- [ ] **Step 1: Write the failing method-reference selection test**

Compile a temporary application containing `System.out::println`; call the public bytecode-input path and assert `ok` rather than `failed` with `UnimplementedError`.

- [ ] **Step 2: Write the explicit-artifact test**

Pass a compiled fixture with `--classes-dir` and no Maven POM; assert the adapter returns an `ok` bytecode result. Pass a nonexistent classes directory and assert `failed` JSON with a diagnostic.

- [ ] **Step 3: Verify RED**

Run: `mvn -q -f wala-callgraph/pom.xml -Dtest=WalaCallGraphMainTest test`

Expected: FAIL because the adapter has no bytecode artifact selection path.

- [ ] **Step 4: Implement bytecode-first selection**

Parse repeatable `--classes-dir` values. If explicit class directories are supplied, use them and optional `--classpath`. Otherwise, for a conventional Maven source root, call `MavenBytecodeArtifacts.prepare`; invoke `BytecodeCallGraph.analyze` on success. Only attempt the current source analyzer when no bytecode artifacts can be prepared. Convert all preparation and WALA errors into existing `failed` JSON output.

- [ ] **Step 5: Verify GREEN**

Run: `mvn -q -f wala-callgraph/pom.xml -Dtest=WalaCallGraphMainTest test`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add wala-callgraph/src/main/java/com/voyantclair/wala/WalaCallGraphMain.java wala-callgraph/src/test/java/com/voyantclair/wala/WalaCallGraphMainTest.java
git commit -m "feat: select WALA bytecode analysis for Maven projects"
```

### Task 4: Validate the provider contract and benchmark behavior

**Files:**
- Modify: `docs/call_graph_providers.md`
- Modify: `wala-callgraph/src/test/java/com/voyantclair/wala/BytecodeCallGraphTest.java`

**Interfaces:**
- Preserves `CallGraphResult` schema version 1, provider ID `wala-java`, and non-fatal statuses consumed by `WalaJavaProvider`.

- [ ] **Step 1: Write the contract regression test**

Assert the bytecode outcome has `provider_id = "wala-java"`, `algorithm = "zero_one_container_cfa_bytecode"`, and only application-originating edges.

- [ ] **Step 2: Verify RED**

Run: `mvn -q -f wala-callgraph/pom.xml -Dtest=BytecodeCallGraphTest test`

Expected: FAIL until bytecode metadata is emitted consistently.

- [ ] **Step 3: Document prerequisites and fallbacks**

Explain Maven reactor build/cache side effects, `--classes-dir` and `--classpath` use for Gradle/CI, entrypoint requirements, application-edge filtering, and remaining framework/reflection limitations.

- [ ] **Step 4: Verify complete adapter suite and benchmark**

Run:

```bash
mvn -q -f wala-callgraph/pom.xml test
mvn -q -f wala-callgraph/pom.xml package -DskipTests
java -jar wala-callgraph/target/wala-callgraph-all.jar --source-dir /Users/macbook/Desktop/dt/benchmarks/java/sample-spring-kafka-microservices/order-service/src/main/java
```

Expected: Maven tests pass. The benchmark emits one JSON document and does not report `ExpressionMethodReference`; report application-edge count and remaining diagnostics.

- [ ] **Step 5: Commit**

```bash
git add docs/call_graph_providers.md wala-callgraph/src/test/java/com/voyantclair/wala/BytecodeCallGraphTest.java
git commit -m "docs: explain WALA bytecode analysis"
```
