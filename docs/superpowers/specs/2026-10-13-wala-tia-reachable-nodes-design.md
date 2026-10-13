# WALA TIA reachable-node contract

## Goal

Avoid module-wide test fallback when a changed Java callable is present in WALA's
test-entrypoint call graph but has no incoming or outgoing emitted edge. Preserve
the existing conservative fallback whenever the analysis cannot prove that a
changed callable is represented by the graph.

## Current limitation

The WALA adapter emits only caller-to-callee edges. TIA identifies a changed
callable only by searching edge targets. A reachable leaf has no outgoing edge;
an entrypoint-like node may also have no incoming edge. Neither can be
distinguished from an absent/unsupported callable, so TIA selects every test in
the module.

In the Debezium fixture, all WALA edges map to source callables. Three changed
callables resolve through edges, while `AbstractType.unexpectedValue()` and
`ConnectDecimalType.getDefaultValueBinding()` do not. The resulting fallback is
therefore caused by missing node membership, not unresolved edges.

## Contract

The JSON response becomes schema version 2 and adds `nodes: MethodRef[]`.

- `nodes` contains every application-loader method node in the WALA call graph
  reachable from the supplied test entrypoints, including isolated/leaf nodes.
- `edges` remains caller-to-callee relations and keeps its present meaning.
- Failed and no-entrypoint results emit an empty `nodes` list.
- Rust accepts only schema version 2 from the updated adapter. An absent node
  list is invalid rather than silently weakening selection.

## Resolution and selection

Rust resolves node references with exactly the same Java source matching rules
as edges. The resolver returns both resolved edges and resolved callable-node
IDs, plus uncertainty diagnostics for source nodes that cannot be mapped
uniquely.

For each changed callable or class:

1. Match it against resolved node IDs, not merely edge targets.
2. If no node matches, retain the existing module-wide fallback: the graph did
   not prove coverage of that changed element.
3. If matching nodes exist, start reverse reachability from those nodes using
   resolved edges. Select only reached tests.
4. If no test reaches a known node, select no test for that element. Since WALA
   constructed the graph from the discovered test entrypoints, node membership
   proves the method was considered; the absence of a reverse path proves it
   cannot be reached from any selected test within this analysis.

Module, configuration, adapter-failure, timeout, and any unresolved-source
diagnostic paths remain module-wide fallbacks.

## Implementation boundaries

1. Java adapter: extend `CallGraphResult`; collect application nodes while
   iterating WALA's graph; update JSON and unit tests.
2. Shared Rust model/provider: add `nodes` to `CallGraphOutcome`, parse schema
   version 2, and preserve empty nodes in synthetic failures.
3. Resolver: map `MethodRef` nodes to callable IDs using the existing resolver;
   surface unresolved source nodes as uncertainty.
4. Selection: accept resolved nodes and use them for changed-element lookup;
   continue to use edges only for reverse traversal.

## Verification

- Java adapter test: application leaf nodes are emitted and dependency nodes are
  excluded.
- Rust provider test: schema version 2 with nodes parses; version 1 is rejected.
- Resolver test: a source node maps even when it has no edge.
- Selection tests: a known leaf node with no reverse path selects no tests; an
  absent node retains module fallback; an edge-backed callable remains selected
  through reverse reachability.
- Full CLI impact suite remains green.
- Debezium rerun must either narrow selection or explain any remaining fallback
  with an explicit absent/ambiguous changed-node diagnostic.
