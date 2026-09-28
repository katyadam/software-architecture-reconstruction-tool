# Extensible Static Call-Graph Integration

## Purpose

Add project-wide static call graphs to VoyantClair so later Test Impact
Analysis can traverse resolved intra-service calls as well as existing
syntactic call observations and reconstructed service communication links.

Java/WALA is the first provider to implement. The architecture must support
future Python and Go providers without changes to orchestration, normalization,
IMCG construction, or Test Impact Analysis. Existing Tree-sitter extraction is
unchanged and test selection remains out of scope.

## Scope and success criteria

For every configured service root, the pipeline selects compatible call-graph
providers for detected languages, normalizes their results, and imports stable
resolved caller-to-callee edges into IMCG. Each provider runs at most once per
eligible service root.

The initial analysis uses WALA's source-directory driver architecture,
`ZeroOneContainerCFA`, and no reflection modelling. An interface dispatch in a
small fixture must produce an edge to its concrete implementation. Existing
extraction must still succeed if any provider cannot analyze a service.

## Architecture

Use ports-and-adapters. The Rust core owns a language-neutral
`CallGraphProvider` port and statically registered provider registry. A
provider owns its tool invocation, entrypoint discovery, and raw call graph.
The core owns language detection, selection, result validation, conservative
mapping to Tree-sitter callables, diagnostics, persistence, and IMCG merging.

The first provider is `WalaJavaProvider`, backed by a small Maven module named
`wala-callgraph`. It adapts WALA-start's `SourceDirCallGraph` rather than
embedding Java into Rust. Future `GoToolsProvider` and `PythonProvider`
implement the same contract. Adding a provider needs one implementation and
registration, not a dynamic plugin mechanism.

```
configured service root
  -> language detection
  -> CallGraphProvider registry
       -> WalaJavaProvider
       -> future GoToolsProvider
       -> future PythonProvider
  -> normalized raw method/edge records
  -> conservative callable resolver
  -> IMCG alongside Tree-sitter callables/calls and SDG links
```

Tree-sitter `Callable` and `CallStatement` records remain unchanged. Raw
provider identities are translated to existing callable signatures only by a
shared conservative resolver.

## Provider contract

`CallGraphRequest` contains a canonical service root, a detected language, and
an optional entrypoint policy. `CallGraphProvider` exposes `supports(language)`
and `analyze(request) -> CallGraphOutcome`.

`CallGraphOutcome` contains status `ok`, `no_entrypoints`, `unsupported`, or
`failed`; diagnostics; analysis metadata; and zero or more `RawCallEdge`s.
Every edge contains source and target `MethodRef`s. A method reference carries
language, qualified declaring type/module, method/function name,
language-appropriate descriptor, and optional source path/line. Edges include
provider ID, analysis algorithm, and confidence/evidence. This versioned model
is the only wire contract for external adapters.

The shared resolver maps a raw method to a Tree-sitter `Callable` only when
source path, type/module, name, constructor status, and parameter information
identify exactly one callable within the service root. Ambiguous or unresolved
methods are diagnosed and omitted; no edge is guessed from a name alone.

## Java/WALA entrypoint discovery

`WalaJavaProvider` builds WALA's Java source analysis scope and class hierarchy, then
discovers every application method with the JVM signature
`public static void main(String[] args)` (`([Ljava/lang/String;)V`). Each
matching method is made a WALA entry point. This avoids adding a mandatory
`main_class` to existing service configuration files.

If a source root has no discoverable main method, the adapter reports a typed
`no_entrypoints` result. The runtime logs the reason and preserves the
Tree-sitter-only output for that service. A later increment may add optional
configured entrypoints for frameworks, libraries, or tests.

Future providers determine their own entrypoint policy while returning the
same outcome type. A Go provider can use the Go toolchain's SSA and selectable
static/CHA/RTA/VTA call-graph analyses. A Python provider must be selected by
benchmark evaluation; its dynamic-dispatch uncertainty is recorded in edge
evidence rather than hidden by the shared model.

## Provider result contract

Every external provider writes one JSON document to stdout containing:

- schema version, provider ID, language, and analysis mode;
- source-root path, elapsed time, node and edge counts;
- zero or more directed edges, each with structured caller/callee method
  identifiers and, where available, source class/file/line metadata;
- warnings and an explicit status (`ok`, `no_entrypoints`, `unsupported`, or
  `failed`).

The adapter must keep diagnostic output on standard error so standard output
is always parseable JSON.

## Error handling

Call-graph enrichment is optional. A provider process start failure, invalid
adapter output, analysis exception, unsupported language, or `no_entrypoints`
result must not fail the existing reconstruction pipeline. The failure is
recorded in diagnostics and the service retains its Tree-sitter call data. This
conservative availability policy prevents external-tool and language-environment
problems from erasing architecture reconstruction results.

## Testing and verification

The Maven module has a fixture source tree containing a `main` method,
interface, implementation, and call site. Its test asserts that the JSON
conforms to the shared contract, includes the expected resolved implementation
edge, and has no non-JSON text on standard output.

The Rust side has provider-registry, result-validation, generic callable
resolution, and degradation tests using fixture results. An end-to-end CLI test
verifies that a Java configured source root exports the WALA edge data while
existing IMCG fields remain available. Tests also prove that unregistered
Python or Go roots are reported as `unsupported` without disrupting normal
reconstruction. Existing workspace tests are run before handoff.

## Explicit limitations

The first version does not implement Go or Python providers, infer Spring
controller entrypoints, resolve Maven or Gradle dependencies, model reflection,
merge raw provider edges into `CallStatement`, or implement test selection.
Those require independent algorithm choice and benchmark evaluation because
they affect precision, scalability, and soundness.
