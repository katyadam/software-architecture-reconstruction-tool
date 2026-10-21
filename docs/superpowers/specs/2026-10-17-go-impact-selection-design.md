# Go Git impact mapping and conservative selection

Status: design for user review. This extends the native Go proof and Rust
adapter on `feat/go-callgraph-tia`; it does not change the Java TIA contract.

## Agreed outcome

Turn a Git baseline-to-candidate comparison into an auditable Go impact report.
Keep graph suggestions separate from the tests that callers should execute.
The user approved this policy: the controlled mutation has five graph-reachable
tests, but all seven discovered tests remain selected because coverage is
incomplete. An absent graph path is not a safe exclusion guarantee.

This is an architectural slice: it introduces Go change identities, discovery,
selection policy, and command orchestration. A written implementation plan follows
approval of this spec. The existing proof and adapter are prerequisites, not work
to recreate.

## Architecture and alternatives

Use a separate Go impact pipeline with small, explicit boundaries:

1. Git reader: revisions, file statuses, old/new blobs and changed ranges.
2. Go source inventory/mapper: native declarations, test identities and ownership.
3. Go call-graph adapter: targeted reachability and coverage, already implemented.
4. Go selection policy: graph suggestions versus conservative execution decisions.
5. A thin Go command and a reproducible control script.

The Go-aware inventory/mapper must not depend on CHA graph construction. Future
graph providers can consume the same native targets and discovery information.
Reuse native Go parsing/type metadata where available rather than manufacture
JVM descriptors. Share only language-neutral utilities with Java.

Alternative: generalize every existing Java impact model and command now. That
would increase compatibility risk without helping this first Go evaluation, so
defer it. Alternative: expose graph reachability directly as selected tests.
Reject it because opaque dependencies and callbacks make that claim unsound.

## Scope

Support one Git checkout whose module root contains `go.mod`, including ordinary
package subdirectories, internal tests and external `_test` packages. Analyze
the configured platform and build tags with `GOWORK=off`, consistent with the
native provider. Top-level `TestX` functions are the selection unit; nested
subtests belong to their parent. A separate execution-package identity groups
internal and external tests into the package used by `go test` without changing
their distinct native declaration identities.

Nested modules and active workspace arrangements are not silently folded into
the root module. Detect them and reject this unsupported input with a diagnostic
identifying the module/workspace boundary; do not write a successful selection
report claiming repository-wide coverage. Cross-module orchestration,
distributed HTTP/gRPC/message impact,
reflection modeling, precise generics, and safe graph-based exclusion are not
part of this slice. Real-project benchmarking follows the controlled end-to-end
gate rather than being a prerequisite for implementing this report.

## Git comparison and declaration mapping

Resolve the supplied baseline and `HEAD` to full commit IDs. Compare those
commits, not an implicit merge base. Refuse a dirty candidate checkout rather
than read working-tree source under an unrelated committed revision. Validate
revisions as data, not executable shell text or Git options. Support linked
worktrees. Do not alter the analyzed checkout, install dependencies, or run its
tests during normal analysis. Use dependencies already available locally;
missing dependencies produce explicit discovery/analysis failure rather than
automatic dependency installation.

Read file statuses using NUL-delimited output. Obtain patches per identified
path with external diff/text conversion disabled; do not infer filenames from
quoted patch headers. Retain both old and new paths for renames. Unrepresentable
paths or malformed diff records must fail explicitly, not disappear.

Map modifications inside a uniquely identified Go function or receiver method
to its native package/receiver/name target. Use Go-aware declaration boundaries
and verify the declaration identity/header against the old blob before calling
an edit body-only. A closure edit belongs to its enclosing named declaration.
Handle multiple hunks and deduplicate targets. Preserve pure-deletion hunks:
zero candidate lines do not mean no change. A deletion inside an unchanged
callable may map using its old owner; otherwise broaden the change.

Use module-level fallback records for deleted/renamed files, declaration
additions/removals or signature changes, package/import changes, global/type
declarations, initialization changes, build constraints, `go.mod`, `go.sum`,
workspace files, non-Go configuration/resources, parse errors, ambiguous
ownership, and any unmapped range. An added/deleted declaration can affect
interface dispatch, so do not narrow it solely to a new graph target. Changed
test files follow the same mapping/fallback rules; they are not ignored.

For configuration-only or wholly unmappable changes, skip expensive graph
construction but still discover tests and produce an explicit fallback report.
An empty committed diff is a no-op: no tests selected, no graph construction,
and an explicit `no_changes` decision. Skip discovery too, set
`run_all_tests: false`, and record discovery as skipped with
`test_inventory_complete: false`;
that state does not claim the module contains no tests.

## Discovery independent of graph success

Provide a Go source inventory step without constructing SSA/CHA. Reuse the
loader's build context and native test-signature checks, including type aliases.
Record declaration source ownership, receiver identity and native target keys.
Deduplicate test-augmented variants while retaining external-package tests.
Inventory completeness must be explicit; never derive it from a nonempty list.

Inventory remains useful when graph construction fails, times out, encounters
unsupported `TestMain`, or is skipped for a broad change. Discovering a
`TestMain` must not discard sibling tests: setup remains an analysis limitation
requiring broad execution, as it is in the current proof.

If loading fails and a complete test inventory cannot be established, do not
invent tests or report an empty suite. Set `test_inventory_complete: false`,
`run_all_tests: true`, and include the failure diagnostics. Callers must run the
whole supported module rather than execute only an incomplete per-test list.

## Go-specific report

Add Go report types alongside, not inside, the Java `TestImpactResult` and
`JavaTestCase` types. A versioned Go report contains:

- `language: go`, schema version, module root and full baseline/candidate IDs.
- `changed_elements`: file statuses, paths, callable targets where known, and
  explicit fallback classification/reasons.
- `discovered_tests` and `test_inventory_complete`, using native declarations
  plus their execution-package identity.
- `graph_suggested_tests`: deduplicated discovered tests with positive graph
  paths to changed targets, each with the corresponding changed-target reasons.
- `selected_tests`: the actual conservative execution selection and reasons.
- `unselected_tests`: no entries for a changed module while coverage is partial.
- `selection_mode`: `no_changes` or `conservative_all`; no aggressive mode.
- `run_all_tests`: true for every nonempty change in this initial partial-coverage
  implementation, including when discovery fails.
- Graph status, native coverage/scope metadata, mapping/discovery status,
  diagnostics and stage timings. Skipped graph analysis has an explicit reason.

Do not label suggestions “safe”, “complete”, or “selected” in JSON or summaries.
Do not reuse the generic graph projection that intentionally removes incomplete
reachability: retain the typed `GoAnalysisResult` for positive suggestions while
using its coverage decision to prevent exclusion. If graph analysis fails or its
response is invalid, produce no graph suggestions and still select broadly.

All discovered tests are selected for a nonempty comparison in this slice,
including tests with positive graph evidence and tests selected only by fallback.
The report must explain the distinction; deduplication cannot erase independent
reasons. Even an empty limitations list or a claimed complete scope from a
provider cannot enable exclusion: the current Rust policy remains fail-closed.

## Command and execution behavior

Add a separate `go-test-impact` Rust binary rather than change the existing
Java `test-impact` arguments or JSON. It accepts `--project-dir`,
`--baseline-revision`, `--go-callgraph-binary`, `--output`, optional `--build-tags`,
and `--analysis-timeout-seconds` (default 300; positive and user-configurable).
The tool path refers to a prebuilt native executable, never a JAR. Analyze a
module root, not an arbitrary repository subtree.

Write phase progress to stderr and the report to the requested JSON file.
Keep machine-readable output free of progress text. Validate paths/revisions
before analysis, and write the final report atomically. Keep output outside the
candidate checkout so reporting does not make the analyzed candidate dirty.

A successfully written, explicit conservative fallback report is a successful
command outcome, even if graph analysis failed. Invalid arguments, invalid Git
state, unsupported module/workspace scope, or inability to write the report
return nonzero with an actionable diagnostic. Do not publish a newly successful
report for those input/output failures. Do not silently overwrite a previous
report until the replacement is complete.

Use independent inventory and graph deadlines so a timed-out graph does not
erase an already discovered inventory. Record which stage timed out. The
timeout governs each native invocation; record stage durations, not an implied
single end-to-end budget. Continue to supervise native subprocesses using the
existing Unix process-group transport and preserve failure/progress diagnostics.

## Review and acceptance gates

1. **Models and discovery:** serialization round trips preserve the two test
   lists, native/execution package identities and coverage. Internal/external
   tests, aliases, build tags, init setup, and failed discovery are covered.
2. **Git mapping:** actual temporary Git repositories prove body edits,
   receiver methods, closures, multiple hunks, deletion-only hunks, deleted and
   renamed files, signature/global/config changes, unusual paths, linked
   worktrees, invalid revisions and dirty-checkout rejection. Nothing unmapped
   silently vanishes.
3. **Policy and command:** a partial graph, absent graph, corrupt response,
   timeout, and incomplete inventory cannot populate `unselected_tests` for a
   nonempty change. No-change handling remains explicit. Tests verify command
   exit behavior, report replacement and clean progress/output separation.
4. **Real native control:** a single script creates a fresh temporary Git
   module from the existing control fixture, commits baseline and mutation,
   invokes the new command, and compares suggestions against separately executed
   mutation failures. Expected: seven discovered, five graph-suggested, seven
   selected, zero unselected, `conservative_all`, `run_all_tests: true`.
   Preserve artifacts and require no multiline copied commands. The analysis
   command itself does not run tests; only the control script does.
5. **Compatibility:** existing Go proof/adapter controls and the complete
   runtime/Java CLI suites pass. Review each implementation slice before
   proceeding. Java adapters, CLI flags, identities and report shape stay intact.

Completion of these gates means auditable conservative Go impact reporting is
implemented, not that arbitrary Go projects can safely skip tests. Reducing the
final selection requires a separately reviewed coverage improvement.
