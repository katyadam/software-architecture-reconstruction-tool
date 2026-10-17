# Go call-graph and test-impact support — draft

## Intent and checkpoint

Extend the diploma evaluation to a second language while preserving the Java
checkpoint at `74efc00`. Go is the proposed next language: the existing provider
contract already includes `Language::Go`, the workspace has a Go extractor, the
local Go toolchain is installed, and sibling Go benchmarks contain test files.
The standalone package-loading and reachability proof is implemented in
`go-callgraph/`. Rust TIA integration and real-project benchmarking remain next
steps, gated on review of the proof and its explicit coverage limitations.

Success means that a controlled Go mutation selects the tests that actually
fail, excludes unrelated tests, and records incomplete analysis explicitly.
The first delivery concerns local calls and package boundaries, not distributed
HTTP, gRPC, or message-broker impact.

## Independent native provider

Add a standalone Go executable, separate from both Java adapter JARs and their
build preparation. Use Go-aware package loading and type information to build
a static graph, with interface dispatch included in the initial algorithm.
The proof pins `golang.org/x/tools v0.52.0`, using `go/packages`, SSA, and CHA.
Dependency bodies remain opaque. External callbacks, reflection and generic
dispatch are not covered, so graph non-reachability is not yet a safe production
test-exclusion guarantee. `TestMain` is rejected explicitly.

The executable emits the existing versioned call-graph JSON contract. Method
references use Go package paths, receiver identity where applicable, function
names and source positions. They must not imitate JVM descriptors. Progress is
written to stderr and machine-readable results to stdout. Loading or type errors
must produce a failed/uncertain outcome, never an empty successful graph.

Load test variants deliberately: production packages, internal tests and external
`package example_test` tests must resolve to the correct package identities.
Compiler-generated test wrappers must not appear as duplicate user tests.
Build tags, platform and workspace/module configuration form part of the analysis
scope and must be recorded. Unsupported workspace arrangements remain explicit.

## TIA integration boundary

The provider registry is already language-aware, but the current TIA command,
test discovery, source resolver and selection data use Java-specific identities.
Introduce Go discovery and resolution alongside them. Do not pass Go through
`JavaTestCase`, Java module paths or the Java resolver. Preserve existing Java
command compatibility and JSON behavior when introducing language-neutral test
identities and dispatch.

Use top-level Go test functions as the initial selection unit. Nested `t.Run`
cases are covered by their parent test; do not claim separate subtest selection.
Account for package initialization and test setup before claiming safe exclusion.
Benchmark failures outside the discovered test universe must remain visible.

## Delivery sequence

1. Prove the native provider on a dependency-light fixture: direct calls,
   a transitive call, interface implementations, and an unrelated test.
2. Add Go-specific Git change mapping, test discovery and source resolution,
   extending the shared selection interfaces without changing Java identities.
3. Verify calls across package boundaries and external test packages. Compare
   selected tests against executed mutation failures.
4. Select a small real project from `../benchmarks/go` after checking its
   build requirements and tests; provide one reproducible benchmark command.

## Review and stopping criteria

Review the package-loading/API proof before building the integration. Each
subsequent slice must include executable regression checks and a concrete
selection report. Keep graph construction, test discovery and selection separate
so a future Go provider can use the same test and mutation inputs.

The initial language slice is complete when the controlled direct, transitive,
interface and cross-package cases match observed failures, one small real
benchmark runs reproducibly, and unsupported cases are documented. Reflection,
plugin loading, runtime-generated calls, precise goroutine scheduling, and
cross-service causality are outside this first delivery.

## Proof checkpoint

The standalone control is reproducible with
`scripts/run-go-callgraph-control.sh`. Its mutation reaches five of seven tests,
matching the five observed test failures. The initialization target reaches six
importing tests, and an uncalled target returns explicit empty reachability.
Internal and external test packages share initialization roots according to
their test binary. Regression coverage also includes test type aliases,
repeated targets, unsupported `TestMain`, load failures, missing targets,
structured timeouts, and JSON/progress separation.

This completes the package-loading/API proof, not the full language slice.
Next: review the Go-specific discovery/change-mapping and conservative coverage
policy before wiring the provider into Rust TIA, then evaluate a small real Go
benchmark. Existing Java adapters and selection behavior are unchanged.

## Rust process boundary checkpoint

The bounded adapter slice adds `GoNativeProvider::analyze_targets`, preserving
native tests and scope in `GoAnalysisResult` instead of discarding them through
the older provider-neutral response. It validates target coverage and native
identities, supervises the independent executable with progress/timeout handling,
and fails closed when projecting incomplete coverage to the generic contract.
Java and Go reuse only the neutral process transport, not artifact preparation,
test discovery, or resolution. `scripts/test-go-native-adapter.sh` builds and
checks the actual Go/Rust boundary in one command. Go change mapping and test
selection remain a separate next slice.
