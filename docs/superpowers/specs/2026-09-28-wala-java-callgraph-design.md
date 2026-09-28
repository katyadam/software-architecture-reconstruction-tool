# WALA Java Source Call-Graph Integration

## Purpose

Add a project-wide, points-to-aware Java call graph to VoyantClair so that
later Test Impact Analysis can traverse resolved intra-service calls in
addition to the current syntactic call observations and reconstructed service
communication dependencies.

This first increment is deliberately Java-only. It does not change the
existing Tree-sitter extraction for Java, Python, or Go, and it does not yet
implement test selection.

## Scope and success criteria

For every Java source root belonging to a configured service, the pipeline
will be able to invoke WALA once and import a stable list of resolved
caller-to-callee method edges into the IMCG export.

The initial analysis uses WALA's source-directory driver architecture,
`ZeroOneContainerCFA`, and no reflection modelling. An interface dispatch in a
small fixture must produce an edge to its concrete implementation. Existing
extraction must still succeed if WALA cannot analyze a service.

## Architecture

Introduce a small, independently built Maven module named `wala-callgraph`.
It adapts WALA-start's `SourceDirCallGraph` example rather than embedding the
JVM library into Rust. Its command-line interface receives one Java source
root and writes a machine-readable result to standard output.

The Rust extractor runtime remains the orchestration point. After it has
collected the Java files for one configured source root, it invokes the
adapter once, parses the result, and adds WALA edges to the data supplied to
the existing IMCG export. Tree-sitter `Callable` and `CallStatement` records
remain unchanged and continue to support all current architecture features.

```
configuration service/source root
        -> Rust file collection
        -> WALA adapter (once per Java root)
        -> resolved WALA edges
        -> IMCG export alongside Tree-sitter callables/calls
```

## Entrypoint discovery

The adapter builds WALA's Java source analysis scope and class hierarchy, then
discovers every application method with the JVM signature
`public static void main(String[] args)` (`([Ljava/lang/String;)V`). Each
matching method is made a WALA entry point. This avoids adding a mandatory
`main_class` to existing service configuration files.

If a source root has no discoverable main method, the adapter reports a typed
`no_entrypoints` result. The runtime logs the reason and preserves the
Tree-sitter-only output for that service. A later increment may add optional
configured entrypoints for frameworks, libraries, or tests.

## Adapter result contract

The adapter produces JSON containing:

- schema version and analysis mode;
- source-root path, elapsed time, node and edge counts;
- zero or more directed edges, each with canonical WALA caller and callee
  method identifiers and, where available, source class/file/line metadata;
- warnings and an explicit status (`ok`, `no_entrypoints`, or `failed`).

The adapter must keep diagnostic output on standard error so standard output
is always parseable JSON.

## Error handling

WALA is an optional enrichment. A process start failure, invalid adapter
output, analysis exception, or `no_entrypoints` result must not fail the
existing reconstruction pipeline. The failure is recorded in diagnostics and
the service retains its Tree-sitter call data. This conservative availability
policy prevents external Java build and classpath problems from erasing
architecture reconstruction results.

## Testing and verification

The Maven module has a fixture source tree containing a `main` method,
interface, implementation, and call site. Its test asserts that the JSON
includes the expected resolved implementation edge and has no non-JSON text on
standard output.

The Rust side has parser and integration tests using a fixture adapter result.
An end-to-end CLI test verifies that a Java configured source root exports the
WALA edge data while existing IMCG fields remain available. Existing workspace
tests are run before handoff.

## Explicit limitations

The first version does not infer Spring controller entrypoints, resolve Maven
or Gradle dependencies, model reflection, merge WALA edges into the existing
`CallStatement` representation, or implement change-based test selection.
Those need separate design decisions because they affect analysis soundness,
runtime cost, and the public data schema.
