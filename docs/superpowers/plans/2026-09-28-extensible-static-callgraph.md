# Extensible Static Call-Graph Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a provider-based static-call-graph pipeline, implementing WALA Java first and carrying its resolved edges into IMCG.

**Architecture:** Rust owns a normalized provider port, registry, validation, and conservative `Callable` matching. WALA is an external Java adapter that implements the same versioned JSON contract planned for Go and Python tools; IMCG receives only resolved existing callable signatures.

**Tech Stack:** Rust 2024, Serde JSON, Java 17+, Maven, WALA Java source front end / ECJ / `ZeroOneContainerCFABuilderFactory`, JUnit 5.

**Spec:** `docs/superpowers/specs/2026-09-28-wala-java-callgraph-design.md`

## Global Constraints

- Run each provider at most once per eligible configured service root.
- The only adapter wire format is the versioned `CallGraphOutcome` JSON schema.
- Java uses `ZeroOneContainerCFA` with reflection disabled and discovers all `public static void main(String[])` methods after class-hierarchy construction.
- Tree-sitter `Callable` and `CallStatement` behavior does not change.
- `ok`, `no_entrypoints`, `unsupported`, and `failed` are non-fatal outcomes.
- Resolve an edge only when both endpoint methods map uniquely to existing callable signatures; never infer by bare name.
- Go and Python provider implementations are explicitly out of scope; the registry must represent their absence as `unsupported`.

## Review Focus

- An adapter warning or log line must never corrupt JSON stdout (Task 2).
- Multiple Java main methods must all be entry points, rather than an arbitrary one (Task 2).
- An unregistered Go/Python service root must receive `unsupported` and keep normal extraction output (Task 3).
- An ambiguous class/method mapping must not emit a guessed IMCG edge (Task 3).
- Adapter start failures, non-zero exits, and malformed JSON must not abort CLI reconstruction (Task 3).

## File Structure

- `models/src/call_graph.rs` — language-neutral request, method, raw edge, outcome, and resolved-edge data types.
- `extractor-runtime/src/call_graph/{mod.rs,registry.rs,resolver.rs,wala.rs}` — provider port/registry, exact resolver, and WALA adapter process implementation.
- `wala-callgraph/...` — Java implementation of the shared JSON protocol.
- `cli/src/lib.rs` / `cli/src/main.rs` — configured-root discovery and optional WALA JAR wiring.
- `synthesizer/src/imcg/construction/inter.rs` — merges resolved call-graph edges with heuristic and SDG edges.
- S3 IMCG DTOs — preserve resolved edges between extractor and synthesizer deployments.

### Task 1: Create normalized provider model and registry

**Files:**
- Create: `models/src/call_graph.rs`
- Modify: `models/src/lib.rs`, `models/src/api.rs`, `models/src/ir/evaluted.rs`
- Create: `extractor-runtime/src/call_graph/mod.rs`, `extractor-runtime/src/call_graph/registry.rs`
- Modify: `extractor-runtime/src/lib.rs`
- Create: `extractor-runtime/tests/call_graph_registry.rs`
- Modify: `extractor-runtime/tests/mod.rs`

**Interfaces:**
- Produces: `Language`, `MethodRef`, `RawCallEdge`, `CallGraphOutcome`, `ResolvedCallEdge`, and `CallGraphProvider`.
- Produces: `ProviderRegistry::analyze(request: &CallGraphRequest) -> CallGraphOutcome`.

- [ ] **Step 1: Write failing registry tests**

Assert a registered stub Java provider receives its request and returns `ok`; assert Go and Python requests with no registered provider return `unsupported` with no edges.

- [ ] **Step 2: Verify RED**

Run: `cargo test -p extractor-runtime call_graph_registry -- --nocapture`

Expected: FAIL because the call-graph module and types do not exist.

- [ ] **Step 3: Implement models and registry**

Define Serde-compatible versioned result types. `MethodRef` must include language, declaring type/module, member name, descriptor, source path, and optional line; `RawCallEdge` includes provider ID, algorithm, confidence/evidence. Add `resolved_call_edges: Vec<ResolvedCallEdge>` to aggregate/evaluated IR and initialize it empty in existing evaluation. Register providers statically, selecting solely by language.

- [ ] **Step 4: Verify GREEN and commit**

Run: `cargo test -p extractor-runtime call_graph_registry -- --nocapture`

Expected: PASS.

```bash
git add models extractor-runtime
git commit -m "feat: add call graph provider contract"
```

### Task 2: Implement the WALA Java provider and shared JSON contract

**Files:**
- Create: `wala-callgraph/pom.xml`
- Create: `wala-callgraph/src/main/java/com/voyantclair/wala/CallGraphResult.java`
- Create: `wala-callgraph/src/main/java/com/voyantclair/wala/WalaCallGraphMain.java`
- Create: `wala-callgraph/src/test/java/com/voyantclair/wala/WalaCallGraphMainTest.java`
- Create: `wala-callgraph/src/test/resources/fixtures/{dispatch,no-entrypoint,multiple-entrypoints}/...`
- Create: `extractor-runtime/src/call_graph/wala.rs`
- Create: `extractor-runtime/tests/wala_provider.rs`

**Interfaces:**
- Consumes: `java -jar <jar> --source-dir <absolute-root>`.
- Produces: stdout `CallGraphOutcome` JSON, and `WalaJavaProvider` implementing `CallGraphProvider`.

- [ ] **Step 1: Write failing adapter and Rust parsing tests**

Adapter tests assert JSON-only stdout, `no_entrypoints`, all discovered main methods, and an interface-dispatch implementation edge. Rust tests feed valid, malformed, and `failed` JSON to `WalaJavaProvider` and assert only valid schema-version results become raw edges.

- [ ] **Step 2: Verify RED**

Run: `mvn -f wala-callgraph/pom.xml test`

Expected: FAIL because the module does not exist.

- [ ] **Step 3: Implement `WalaCallGraphMain.run(Path) -> CallGraphResult`**

Adapt WALA-start: build `JavaSourceAnalysisScope`, J2SE scope, source tree, and ECJ hierarchy; select all public static `main` methods with descriptor `([Ljava/lang/String;)V`; create `DefaultEntrypoint`s; build `ZeroOneContainerCFA`; traverse/deduplicate non-synthetic caller→callee edges into structured `MethodRef`s. Configure Maven Shade with the main class and keep diagnostics on stderr.

- [ ] **Step 4: Implement `WalaJavaProvider::analyze`**

Execute the supplied JAR using `Command`, validate exit status and JSON schema/provider/language fields, then return `failed` diagnostics rather than Rust extraction errors for all process or decoding failures.

- [ ] **Step 5: Verify GREEN and commit**

Run: `mvn -f wala-callgraph/pom.xml test && cargo test -p extractor-runtime wala_provider -- --nocapture`

Expected: PASS.

```bash
git add wala-callgraph extractor-runtime
git commit -m "feat: add WALA Java call graph provider"
```

### Task 3: Resolve provider methods and orchestrate configured service roots

**Files:**
- Create: `extractor-runtime/src/call_graph/resolver.rs`
- Create: `extractor-runtime/tests/call_graph_resolver.rs`
- Modify: `cli/src/lib.rs`, `cli/src/main.rs`, `cli/tests/e2e/helpers.rs`, `cli/tests/e2e/scenario_java.rs`

**Interfaces:**
- Produces: `resolve_edges(outcome: &CallGraphOutcome, root: &Path, callables: &[Callable]) -> (Vec<ResolvedCallEdge>, Vec<String>)`.
- Changes: `get_all_code_elements(project_dir, configuration, external_constants, providers)`.

- [ ] **Step 1: Write failing resolver and CLI tests**

Use callables with duplicate method names in different classes/files. Assert only exact source/type/name/parameter matches resolve, ambiguity yields no edge plus a diagnostic, and a Java fixture configured with a WALA provider enriches the aggregate. Assert absent Java provider and unregistered Go/Python roots remain non-fatal.

- [ ] **Step 2: Verify RED**

Run: `cargo test -p extractor-runtime call_graph_resolver -- --nocapture && cargo test -p cli scenario_java -- --nocapture`

Expected: FAIL because no resolver or provider orchestration exists.

- [ ] **Step 3: Implement exact resolution and root orchestration**

Canonicalize `service_root` and callable paths. Match a `MethodRef` only when its language, source file under root, declaring class/module, member name, constructor flag, and erased parameter count produce one callable. After normal three-pass extraction, deduplicate configuration roots, detect contained source languages, call registry providers once per `(root, language)`, append resolved edges, and print diagnostics to stderr. Add optional `--wala-adapter-jar`; omitted JAR means Java returns non-fatal unsupported/failed diagnostics without execution.

- [ ] **Step 4: Verify GREEN and commit**

Run: `cargo test -p extractor-runtime call_graph_resolver -- --nocapture && cargo test -p cli scenario_java -- --nocapture`

Expected: PASS.

```bash
git add extractor-runtime cli
git commit -m "feat: resolve provider call graph edges per service"
```

### Task 4: Merge and persist resolved edges in IMCG

**Files:**
- Modify: `synthesizer/src/imcg/construction/inter.rs`, `synthesizer/src/s3/{model.rs,service.rs}`
- Modify: `extractor-runtime/src/client/s3/{model.rs,client.rs}`, `extractor-runtime/src/api/connectors/s3_connector.rs`
- Create: `synthesizer/tests/imcg_resolved_call_edges.rs`
- Modify: `cli/tests/e2e/scenario_java.rs`

**Interfaces:**
- Changes: `ImcgBuilder::build(..., resolved_call_edges: &[ResolvedCallEdge], ...)`.
- Produces: union of heuristic, resolved, and SDG calls, deduplicated by `(source_id, target_id, request)`.

- [ ] **Step 1: Write failing IMCG tests**

Assert a valid resolved signature pair creates one request-less call; repeated edges create one call; IDs absent from the callable map create no dangling call; existing SDG request calls remain unchanged.

- [ ] **Step 2: Verify RED**

Run: `cargo test -p synthesizer imcg_resolved_call_edges -- --nocapture`

Expected: FAIL because the builder has no resolved-edge input.

- [ ] **Step 3: Implement merge and S3 propagation**

Thread `resolved_call_edges` through direct builder and S3 IMCG DTO/aggregation paths. Add only edges whose both signature IDs exist in the service-callable map; merge and deterministically deduplicate without changing heuristic scoring or SDG request creation.

- [ ] **Step 4: Verify GREEN and commit**

Run: `cargo test -p synthesizer imcg_resolved_call_edges -- --nocapture && cargo test -p cli scenario_java -- --nocapture`

Expected: PASS.

```bash
git add synthesizer extractor-runtime cli
git commit -m "feat: merge resolved call graph edges into IMCG"
```

### Task 5: Verify and document provider extension

**Files:**
- Create: `docs/call_graph_providers.md`
- Modify: `CHANGELOG.md` if required by project convention

- [ ] **Step 1: Document the provider protocol and Java run path**

Describe JSON contract versioning, status semantics, conservative matching, `--wala-adapter-jar`, and the exact steps required to add a future Go or Python provider without changing IMCG.

- [ ] **Step 2: Run full verification**

Run: `mvn -f wala-callgraph/pom.xml test && cargo test --workspace`

Expected: PASS; report every pre-existing failing test by name if baseline is not green.

- [ ] **Step 3: Run documented end-to-end fixture command**

Run: `cargo run -p cli -- --project-dir <fixture-root> --config-file <fixture-config> --output-dir <temporary-output> --wala-adapter-jar wala-callgraph/target/wala-callgraph-*-all.jar`

Expected: exit 0 and `imcg.json` includes the resolved interface implementation edge.

- [ ] **Step 4: Commit**

```bash
git add docs CHANGELOG.md
git commit -m "docs: explain extensible call graph providers"
```

## Self-Review

- Spec coverage: Tasks 1–4 implement the normalized port, static registry, WALA provider, Java entrypoints, conservative mapping, non-fatal outcomes, and IMCG/S3 integration. Task 5 documents future extension. Go/Python implementations remain intentionally deferred.
- Type consistency: external adapters produce `CallGraphOutcome`; the resolver converts `RawCallEdge` to `ResolvedCallEdge`; only resolved signature IDs reach `ImcgBuilder`.
- Review focus: each listed failure mode has an explicit test in Tasks 2 or 3.
- Scope: no dynamic plugin loader, framework entrypoints, dependency-resolution, reflection, or test-selection work is introduced.
