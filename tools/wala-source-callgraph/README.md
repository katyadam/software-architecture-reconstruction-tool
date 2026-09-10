# WALA Java source call graph

This utility creates a whole-program Java call graph from `.java` files. The target project is
read as source by WALA's ECJ frontend; it is not compiled to application bytecode.

It requires JDK 17 or newer and Maven. Build and run it from this directory:

```bash
mvn -q compile exec:java -Dexec.args="--source-dir /path/to/src --main-class com.example.Main --output call-graph.json"
```

`--main-class` identifies the program entry point. The generated JSON contains sorted `caller` /
`callee` edges using WALA method references. By default, edges whose callee is a JDK class are
filtered out; pass `--include-primordial` to retain them.

The analysis is WALA 0-1-CFA with reflection disabled. It resolves virtual calls more accurately
than syntax-only extraction, but, like every static call graph, may include conservative targets
and cannot fully model reflection or framework dependency injection.
