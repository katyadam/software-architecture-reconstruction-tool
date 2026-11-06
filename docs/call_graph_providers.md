# Call-graph providers

VoyantClair accepts optional static call-graph providers per configured service root.
Providers return versioned JSON with `ok`, `no_entrypoints`, `unsupported`, or
`failed` status. Only uniquely matched provider methods become IMCG edges.

## Java WALA

Build the adapter:

```bash
mvn -f wala-callgraph/pom.xml package
```

Run reconstruction with it:

```bash
cargo run -p cli -- --project-dir <project> --config-file <config> --output-dir <output> --wala-adapter-jar wala-callgraph/target/wala-callgraph-all.jar --wala-timeout-seconds 300
```

WALA prefers bytecode analysis for a conventional Maven service source root
(`module/src/main/java`). It builds the selected reactor module and required
upstream modules with Maven, then analyzes their `target/classes` outputs using
Class Hierarchy Analysis (CHA). CHA is conservative and is the default because
it remains practical for large framework dependency scopes. This supports modern
Java syntax, including method references, which WALA's source frontend cannot parse. Maven may create
normal `target/` files and install local module artifacts in the local Maven
cache; it does not modify source files.

`--wala-timeout-seconds` bounds each WALA adapter process. A timeout produces a
non-fatal diagnostic and leaves Tree-sitter extraction intact. The adapter also
includes `analysis_duration_ms` in its JSON diagnostics. For precision studies,
the standalone adapter accepts `--algorithm rta` or the more expensive
`--algorithm zero-one-container-cfa`; CHA is selected by default:

```bash
java -jar wala-callgraph/target/wala-callgraph-all.jar \
  --source-dir <project>/src/main/java \
  --algorithm zero-one-container-cfa
```

For Gradle projects or CI-produced artifacts, invoke the adapter directly with
compiled application outputs and a path-separated dependency classpath:

```bash
java -jar wala-callgraph/target/wala-callgraph-all.jar \
  --source-dir <project>/src/main/java \
  --classes-dir <project>/build/classes/java/main \
  --classpath "<dependency-jar-1>:<dependency-jar-2>"
```

The adapter discovers `public static void main(String[] args)` entrypoints.
Set `VOYANTCLAIR_WALA_RT_JAR` to a Java 8 `rt.jar` when the default is
unsuitable. It emits edges only from application callers, preventing JDK and
framework implementation details from dominating the result. Reflection,
Spring proxies, dependency injection, and message-listener callbacks are not
yet modeled as synthetic edges. Missing entrypoints, failed builds, and adapter
failures are diagnostic only; existing Tree-sitter extraction still runs.

## Adding a provider

Implement `CallGraphProvider` for the language, emit the shared `MethodRef` /
`RawCallEdge` schema, and register it in `ProviderRegistry`. The resolver maps
only unambiguous source methods to `Callable.signature`; the IMCG consumes the
resulting `ResolvedCallEdge` values without provider-specific changes.
