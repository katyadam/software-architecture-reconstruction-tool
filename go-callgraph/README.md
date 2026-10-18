# Native Go call graphs and conservative impact reports

This native executable is independent of WALA and SootUp. It supplies targeted
reachability and graph-free metadata to a separate Rust Go impact pipeline;
it does not prove production-safe test exclusion. Go 1.26+ is required by the
pinned `golang.org/x/tools v0.52.0`.

## End-to-end Go impact control

From the repository root, run:

```sh
scripts/run-go-impact-control.sh
```

The command creates a fresh temporary Git module, builds the tools, commits a
baseline and a return-value mutation, writes an impact report and independently
executes the tests. Expected: **7 discovered, 5 graph suggestions, 7 selected,
0 unselected**. The five suggestions match the five actual mutation failures.
Both unrelated tests stay selected by conservative fallback. Inspect
`graph_suggested_tests`, `selected_tests`, `selection_mode`, `run_all_tests`,
`test_inventory_complete`, stage diagnostics and timing in the printed report.
Artifacts remain available for inspection; benchmark checkouts are untouched.

The separate Rust `go-test-impact` command accepts `--project-dir`,
`--baseline-revision`, `--go-callgraph-binary`, `--output`, `--build-tags`, and
`--analysis-timeout-seconds` (default 300, independently per native invocation).
It never executes tests or builds tools itself. Output must be outside the
analyzed checkout, whose tracked/untracked state must be clean. Single root Go
modules are supported; nested modules, workspace context and ambient `GOFLAGS`
are rejected explicitly. Use the build-tags flag instead of hidden build flags.
Native loading disables persisted `go env -w` configuration (`GOENV=off`) and
clears implicit `GOFLAGS`. Set tags explicitly and export `GOOS`/`GOARCH` when
needed; otherwise the provider's host platform is used. Persisted cache/module
path settings must likewise be exported explicitly if needed for offline loading.

A nonempty diff always produces `conservative_all` and `run_all_tests: true`.
Discovery failure is incomplete inventory, **not** an empty test suite; callers
must run the whole module rather than execute only an incomplete per-test list.
An empty committed diff is an explicit `no_changes` result. Positive suggestions
are auditable evidence, not permission to skip other tests. Arbitrary safe
exclusion and real-project benchmark evaluation remain future work.

## Standalone graph proof

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
Source paths and line numbers always use physical files, ignoring `//line`
aliases so discovery and committed-diff ownership stay consistent.
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
Go Git mapping, independent discovery and conservative impact reporting consume
this contract without changing the Java CLI or models. No real-project Go
benchmark integration is claimed by the controlled fixture.

## Rust adapter

The native executable also supports `--mode inventory --source-dir DIR` for
graph-free typed discovery and `--mode describe` for AST ownership metadata.
Describe reads `{"files":[{"source_path":"snapshot-key","source":"Go source"}]}`
from stdin; it does not load packages or require a module. Metadata envelopes
use schema version 1. Graph mode remains the default and uses schema version 2.
Discovery preserves tests even when unsupported `TestMain` setup is present.
Package loading is offline and uses a scratch module/sum copy so analyzing a
checkout cannot rewrite its module files. Missing cached dependencies fail
explicitly rather than trigger downloads.

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
