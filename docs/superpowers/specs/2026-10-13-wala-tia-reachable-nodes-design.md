# WALA TIA reachable-node contract

## Goal

Avoid module-wide test fallback when a changed Java callable is known to be
reachable from a subset of WALA test entrypoints but has no emitted incoming or
outgoing application edge. Preserve the existing conservative fallback whenever
the analysis cannot prove a callable's test reachability.

## Current limitation

The WALA adapter emits only caller-to-callee edges whose caller is in the
application loader. TIA identifies a changed callable only by searching edge
targets. A reachable leaf has no outgoing edge; an entrypoint-like node may
also have no incoming edge. Further, a library callback can reach an
application method through an external-caller edge that the adapter does not
export. Therefore a source node alone, or reverse traversal over exported
edges, cannot safely prove that no test reaches a callable.

In the Debezium fixture, all WALA edges map to source callables. Three changed
callables resolve through edges, while `AbstractType.unexpectedValue()` and
`ConnectDecimalType.getDefaultValueBinding()` do not. The resulting fallback is
therefore caused by missing node membership, not unresolved edges.

## Contract

The JSON response becomes schema version 2 and adds
`reachability: Reachability[]`, where a reachability record contains an
application `MethodRef` and the selected test-entrypoint `MethodRef`s that can
reach it through WALA's full internal call graph.

- `reachability` contains every application-loader method node reachable from
  at least one supplied test entrypoint, including isolated/leaf nodes.
- Each record's `entrypoints` lists all selected test entrypoints that can
  reach that method, including paths that cross library nodes.
- `edges` remains caller-to-callee relations and keeps its present meaning.
- Failed and no-entrypoint results emit an empty `reachability` list.
- Rust accepts only schema version 2 from the updated adapter. An absent
  reachability list is invalid rather than silently weakening selection.

## Resolution and selection

Rust resolves each reachability method with exactly the same Java source
matching rules as edges. It joins the record's entrypoint identities directly
to discovered JUnit tests. The resolver returns resolved edges and a mapping
from source callable ID to reachable test IDs, plus uncertainty diagnostics
for source methods or entrypoints that cannot be mapped uniquely.

For each changed callable or class:

1. Match it against resolved reachability method IDs, not merely edge targets.
2. If no method record matches, retain the existing module-wide fallback: the
   graph did not prove coverage of that changed element.
3. If a method record maps to one or more test IDs, select exactly those tests.
4. If WALA explicitly emits a record with an empty entrypoint set, select no
   test for that element. (Normal selected-entrypoint analysis is expected to
   produce at least one.)

Module, configuration, adapter-failure, timeout, and any unresolved-source
diagnostic paths remain module-wide fallbacks.

## Implementation boundaries

1. Java adapter: extend `CallGraphResult`; compute selected-entrypoint
   reachability through the full WALA graph for application nodes; update JSON
   and unit tests.
2. Shared Rust model/provider: add `reachability` to `CallGraphOutcome`, parse
   schema version 2, and preserve empty reachability in synthetic failures.
3. Resolver: map reachability methods to callable IDs and entrypoints to JUnit
   tests using existing Java matching rules; surface unmapped source methods or
   entrypoints as uncertainty.
4. Selection: accept resolved callable-to-test reachability and select those
   tests directly; edges remain available for existing call-graph consumers.

## Verification

- Java adapter test: application leaf reachability is emitted with its test
  entrypoint, including a path crossing a library node; dependency nodes are
  excluded.
- Rust provider test: schema version 2 with reachability parses; version 1 is
  rejected.
- Resolver test: a source method maps to the exact discovered test ID even
  when it has no exported application edge.
- Selection tests: direct reachability selects only listed tests; an absent
  method record retains module fallback; an explicitly empty record selects no
  tests.
- Full CLI impact suite remains green.
- Debezium rerun must either narrow selection or explain any remaining fallback
  with an explicit absent/ambiguous changed-node diagnostic.
