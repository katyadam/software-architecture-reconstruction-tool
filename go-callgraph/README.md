# Native Go call-graph proof

This standalone executable is independent of WALA, SootUp, and the Rust TIA
selection pipeline. It proves targeted Go reachability, not production-safe
test exclusion. Go 1.26+ is required by the pinned `golang.org/x/tools v0.52.0`.

Run the complete regression and mutation control from the repository root:

```sh
scripts/run-go-callgraph-control.sh
```

It copies a small module to a fresh temporary directory, verifies its baseline,
analyzes three targets, mutates one return value, and compares reachable tests
with actual failing tests. Artifacts are retained at the printed path. The
original source and benchmark checkouts are not modified.

Standalone usage (inside this directory):

```sh
go build -o /tmp/go-callgraph .
/tmp/go-callgraph --source-dir "$PWD/testdata/control" --target 'example.com/tia#Changed' --timeout 5m
```

Targets use `package/import/path#Function` or
`package/import/path#(Receiver).Method` (including `*` for pointer receivers).
References carry Go signatures and source positions, not JVM descriptors.
Repeat `--target` to analyze several declarations. Uncalled declarations produce
explicit empty reachability; unmatched targets fail explicitly.

Results are one JSON document on stdout; phase logs go to stderr. Exit codes:
0 for successful analysis, 1 for a structured analysis failure, 2 for invalid
arguments. `--timeout` bounds the executable: package loading receives a context;
if SSA/graph construction has not returned, the executable emits a failed result
and exits, terminating its worker. `Analyze` alone cannot interrupt SSA mid-build.

## Scope and limitations

- [`go/packages`](https://pkg.go.dev/golang.org/x/tools/go/packages) loads `./...`
  and internal/external test variants. User tests are deduplicated by declaration.
- [`SSA`](https://pkg.go.dev/golang.org/x/tools/go/ssa/ssautil) builds bodies for
  the loaded module packages, not dependency packages. The output records
  `dependency_bodies: opaque`.
- [`CHA`](https://pkg.go.dev/golang.org/x/tools/go/callgraph/cha) provides
  conservative interface dispatch within the built graph. It can overapproximate
  and does not establish runtime feasibility.
- Package initialization is attached to tests in importing packages. `TestMain`
  is rejected explicitly until its setup/teardown is modeled. Top-level `TestX`
  functions are the selection unit; subtests belong to their parent.
- External-library callbacks, reflection, plugins, and generic dispatch are not
  covered by this proof. Absence of a path is **not** a safe exclusion guarantee
  for arbitrary projects.
- A module-root `go.mod` is required; `GOWORK=off`, `GOTOOLCHAIN=local`, and
  `-mod=readonly` are enforced. `--tags`, GOOS, GOARCH, and the provider's Go
  version are recorded. Dependencies must already be resolvable; loading errors
  are failures, not empty graphs.

The schema-version-2 envelope includes `tests` and `scope` proof metadata.
`reachability` contains only requested targets; `edges` is intentionally empty.
No Go Git change mapping, Rust test discovery, selection command, or real-project
benchmark integration has been added in this slice.

## Rust adapter

`extractor_runtime::call_graph::go_native::GoNativeProvider` invokes this
executable directly, without Java preparation. `analyze_targets` returns a typed
`GoAnalysisResult`: its `graph`, native `tests`, and `scope` remain available for
auditing. The adapter validates schema/provider/algorithm, source paths, build
tags, test membership, and the presence of every requested target before
accepting the result. Valid failed JSON retains its diagnostics even when the
executable exits nonzero. Invalid output, process failures, and deadlines are
explicit failures, never successful empty graphs.

Run the Rust protocol checks and the real executable control from the repository
root with one command:

```sh
scripts/test-go-native-adapter.sh
```

Process supervision currently supports Unix (macOS/Linux), streams phase logs
to stderr, and kills the process group on timeout. Other platforms fail
explicitly in this slice. Only the language-neutral process transport is shared
with Java adapters; build preparation and graph/test semantics stay separate.

`requires_conservative_fallback()` is always true for the current Go proof.
`into_conservative_outcome()` projects it into the older scope-less graph
contract as `unsupported`, removes incomplete reachability/edges, and includes
coverage diagnostics. The generic provider-registry request lacks native targets
and returns `unsupported` with a diagnostic directing callers to
`analyze_targets`. No coverage claim in the executable's JSON can opt into safe
exclusion. The Java CLI and its output contract are unchanged.

Verification inside this module:

```sh
go test -race ./...
go vet ./...
```
