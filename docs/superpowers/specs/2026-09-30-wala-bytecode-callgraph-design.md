# WALA Bytecode Call-Graph Backend

## Purpose

Make Java call-graph reconstruction work on modern Maven services whose source
contains constructs unsupported by WALA's ECJ source translator, such as method
references. The adapter will analyze build-produced bytecode while retaining
the existing versioned JSON contract, Rust provider, resolver, and IMCG merge.

## Scope

The first increment supports conventional Maven Java service modules. Given a
source root at `<module>/src/main/java`, the adapter identifies the module POM
and an enclosing reactor POM, compiles the requested module and Maven-required
reactor dependencies, and reads their output class directories. Gradle remains
supported through explicit class-output and dependency-classpath arguments;
automatic Gradle builds are a later increment.

No source files in the analyzed project are modified. Maven may create normal
`target/` outputs and use the local Maven artifact cache.

## Design

The adapter accepts either:

- `--source-dir <module>/src/main/java`, from which it discovers and builds a
  Maven module; or
- `--classes-dir <compiled output>` together with optional `--classpath`, for
  already-built Maven, Gradle, or CI artifacts.

For a Maven source root, the adapter runs Maven with the reactor POM and the
selected module plus required upstream modules. It uses their `target/classes`
directories as application modules. It separately obtains external dependency
JARs and adds them to WALA's extension scope. An explicit `--classpath` adds
to, rather than replaces, discovered external dependencies.

WALA builds a binary analysis scope: Java runtime classes are primordial,
application class directories are application modules, and third-party JARs
are extension modules. It discovers all application methods with the JVM main
signature `([Ljava/lang/String;)V`, applies 0-1-container-CFA, and emits only
edges whose caller belongs to an application class directory. Callees may be
application classes or dependencies in raw output, but the existing Rust
resolver keeps only uniquely resolvable application callables.

If bytecode analysis cannot be prepared because Maven fails, no class output
exists, no entry point is present, or WALA cannot parse a class-file version,
the adapter returns a structured `failed` or `no_entrypoints` result. It may
attempt the existing source backend only when bytecode artifacts are absent;
source-backend failures remain non-fatal diagnostics.

## Entry points and limitations

Bytecode removes WALA ECJ source-parser limitations, including method
references. It does not automatically model reflection, Spring proxies,
dependency injection, Kafka listeners, or other framework callbacks. The
initial graph is therefore a conservative explicit-call baseline for later
framework entrypoint and synthetic-edge enrichment.

Compiled classes must be compatible with the WALA version used by the adapter.
The analysis requires a Java runtime model compatible with the target bytecode
and a resolved dependency classpath.

## Verification

Tests create a small external dependency JAR and application class directory,
then verify bytecode interface dispatch produces the expected application edge.
Tests verify absent class output and unsupported class-file preparation return
structured JSON rather than throwing. A benchmark run against the
Spring/Kafka order service must complete without the ECJ
`ExpressionMethodReference` failure and report the number of application call
edges and diagnostics.
