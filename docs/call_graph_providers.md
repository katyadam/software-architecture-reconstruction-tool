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
cargo run -p cli -- --project-dir <project> --config-file <config> --output-dir <output> --wala-adapter-jar wala-callgraph/target/wala-callgraph-all.jar
```

WALA uses 0-1 container CFA and discovers `public static void main(String[] args)` entrypoints. Set `VOYANTCLAIR_WALA_RT_JAR` to a Java 8 `rt.jar` when the default is unsuitable. Missing entrypoints or adapter failures are diagnostic only; existing Tree-sitter extraction still runs.

## Adding a provider

Implement `CallGraphProvider` for the language, emit the shared `MethodRef` /
`RawCallEdge` schema, and register it in `ProviderRegistry`. The resolver maps
only unambiguous source methods to `Callable.signature`; the IMCG consumes the
resulting `ResolvedCallEdge` values without provider-specific changes.
