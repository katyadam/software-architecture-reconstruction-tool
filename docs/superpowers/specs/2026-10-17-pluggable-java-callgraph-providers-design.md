# Pluggable Java Call-Graph Providers — Design

## Purpose

The Java test-impact analysis must evaluate call-graph construction strategies rather than
being defined by WALA. WALA remains the default baseline, but a caller must be able to run a
compatible SootUp backend and later add OPAL, a small ASM baseline, or dynamic coverage without
changing the impact-selection algorithm.

This design preserves conservative selection: an unavailable, malformed, timed-out, or failed
provider never causes tests to be omitted.

## Scope

The first delivered provider is SootUp using class-hierarchy analysis (CHA) over the existing
compiled Maven/Gradle artifacts. The delivery includes provider selection, a shared adapter
protocol, unit tests, and two comparative benchmark commands:

1. Kafka positive control, which already has an expected callable-level selection result.
2. grpc-java xds, where WALA currently fails while parsing bytecode.

RTA, pointer-analysis variants, cross-provider result merging, OPAL, ASM, and dynamic coverage
are deliberately out of scope for this slice.

## Architecture

```text
Maven/Gradle artifact discovery + Java source/callable mapping
                         |
                         v
                 JavaCallGraphProvider
                         |
          +--------------+--------------+
          |                             |
          v                             v
  WALA adapter JAR                SootUp adapter JAR
          |                             |
          +--------------+--------------+
                         v
           schema-v2 CallGraphOutcome JSON
                         v
        existing edge/reachability resolution
                         v
              existing conservative TIA selection
```

### Provider-neutral contract

The standalone Java adapters share exactly one versioned JSON protocol:

- `schema_version = 2`.
- `provider_id` identifies the backend, for example `wala-java` or `sootup-java`.
- `status`, `algorithm`, diagnostics, `MethodRef`, `Edge`, and `Reachability` retain their current
  meanings.
- Method references use JVM internal declaring-type notation (`Lpkg/Type`), member name, and JVM
  descriptor. Constructors use `<init>`.
- The process command accepts the existing arguments: `--source-dir`, `--classes-dir`,
  `--test-classes-dir`, `--classpath`, `--entrypoint`, `--target`, and `--algorithm`.

The protocol, not a Java implementation class, is the interoperability boundary. A backend may
emit an empty `edges` list when it provides targeted reachability; it must emit an explicit empty
`reachability` list when no selected target is reached. It must not use absence of data to signal
success.

### Rust provider launcher

Replace the WALA-named process-launcher abstraction with a generic Java adapter launcher that
owns process isolation, timeouts, JSON validation, argument construction, and streaming phase
telemetry. It receives a provider label and adapter JAR. Backend-specific wording appears only in
the CLI option compatibility layer and diagnostics.

The test-impact CLI exposes a provider selector with `wala` as its default and preserves
`--wala-adapter-jar` as a deprecated compatible alias. A new explicit adapter argument is required
for `sootup`; an invalid selector or missing JAR fails argument validation before project analysis.

### Java adapter layout

Extract the schema-v2 records and shared CLI parsing into a small Java contract/support module
that does not depend on WALA. Keep WALA implementation code in its existing module. Add a sibling
SootUp adapter module that depends only on that shared support module and SootUp artifacts.

SootUp CHA receives application main/test class directories and the resolved dependency classpath.
Its entrypoint selectors are the discovered JUnit methods. For changed callable targets, it emits
target-to-entrypoint reachability using the common schema. All method-identity conversion is
centralized in the SootUp adapter; Rust continues to resolve source callables as it does today.

### Failure handling and observability

Every provider writes compact phase records to stderr in this neutral form:

```
callgraph_phase=<phase> provider=<provider-id> elapsed_ms=<milliseconds>
```

The generic launcher forwards these records live with the provider label. A timeout includes a
bounded stderr tail. Exceptions become a schema-v2 `failed` outcome with a short diagnostic and
one useful top stack frame. The TIA selector interprets these as uncertainty and selects the
conservative fallback set.

## Alternatives considered

1. **Add SootUp directly to the WALA JAR.** Fast initially, but makes another tool inherit WALA
   naming, dependencies, and release cycle. Rejected.
2. **Fork selection logic per backend.** Allows backend-specific shortcuts but prevents valid
   comparisons. Rejected because both implementations must answer the same selection question.
3. **Use only a command-line exporter.** Lowest integration effort, but method identity,
   reachability, and diagnostics would be tool-specific. Rejected as the core design; it remains
   viable for later exploratory baselines.

## Testing and acceptance criteria

1. Contract tests prove both adapters serialize the same schema-v2 shape and JVM method identity
   conventions.
2. Launcher tests prove provider-neutral argument forwarding, timeout diagnostics, invalid JSON,
   and provider-name attribution.
3. SootUp adapter unit tests compile a small Java fixture, discover a direct test-to-production
   call, and report no reachability for an unrelated changed method.
4. Existing WALA tests and the existing CLI compatibility path remain green.
5. Kafka positive control runs under both providers. The report records selected/unselected counts,
   analysis duration, and whether the changed `OrderConverter.orderDaoToKafka()` reaches the same
   tests.
6. grpc-xds runs under SootUp with `-PskipAndroid=true`; the report records either a valid outcome
   or a specific SootUp failure diagnostic. A successful process alone is not treated as proof of
   useful selection.

## Evaluation data for the diploma

For every backend/project pair, collect preparation success, call-graph completion status, elapsed
time, fallback reason, selected/unselected counts, and the changed-callable reasons. On controlled
changes, compare selected tests with the known affected test or mutation result. Report agreement
and disagreement between providers rather than declaring either graph intrinsically correct.

## Compatibility and migration

WALA remains the default backend through this change. Existing scripts that pass
`--wala-adapter-jar` continue to execute WALA unchanged. New scripts use the provider-neutral
arguments and are the preferred examples. Removing WALA-specific flags is a future breaking-change
decision and is not part of this work.
