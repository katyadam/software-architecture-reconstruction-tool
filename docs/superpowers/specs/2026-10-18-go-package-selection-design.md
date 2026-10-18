# Opt-in Go test-package selection

Status: written design for user review; not implemented.
Base: `df0560e` on `feat/go-callgraph-tia`.

## Agreed outcome and boundaries

The user approved package-level granularity as the first exclusion policy.
Select entire Go test binaries affected by changed packages, rather than infer
that an individual test is unaffected from a missing CHA path. Keep the existing
run-all policy as the default. WALA, SootUp and Java contracts remain separate.

Success means a supported comparison can exclude independent execution packages,
retain every test in affected binaries, and explain both decisions using verified
package evidence. A failed or unsupported comparison must not become an empty
successful selection. More mutation failures captured is useful evidence, not
proof that arbitrary Go integration tests can safely be excluded.

This is an architectural extension to discovery, evidence validation, reporting
and selection. Implementation follows approval of this spec and its subsequent
implementation plan. This document does not authorize a default-policy switch.

## Approach and alternatives

Use independent native package-dependency evidence and a Rust package policy.
The native provider reports facts about loaded packages and test-binary roots;
Rust validates those facts and owns execution decisions. Do not couple discovery
or package selection to CHA success. Other future Go graph providers can continue
to supply positive suggestions without owning package-exclusion authority.

Alternative: exclude individual tests using graph reachability. Defer it:
opaque dependencies, unmodeled callbacks and other dynamic behavior still make
absent graph paths insufficient. Alternative: retain only the current run-all
policy. Preserve it as the default and fallback, but it cannot demonstrate actual
test-package reduction for supported inputs.

## Explicit scope of the exclusion claim

The new policy concerns **linked-binary unit-test impact** in one clean Git root
module, for the recorded platform and build tags. A test binary is unaffected by
a Go body-only edit when none of its compiled package dependencies owns that
edit, provided it does not separately consume the changed source or derived
artifacts outside that recorded binary.

This is a caller-declared execution contract, not a property that an import
graph can prove about arbitrary code. In particular, the claim excludes:

- Tests that build or execute other parts of the checkout as subprocesses.
- Loading plugins/shared artifacts derived from changed module source.
- Reading source text, Git state or separately built artifacts as test inputs.
- Distributed/service effects, shared external infrastructure and dependence on
  another test binary's side effects or execution order.
- Different runtime/build contexts from those recorded in the report.

Use `--selection-policy conservative|packages`, default `conservative`.
Requesting `packages` also requires `--test-scope linked-binary`, making that
contract explicit. Missing or incompatible scope is an input error; the command
does not guess that existing tests are unit-isolated. The report repeats the
declared assumptions. Projects unable to make them must use `conservative`.
The scope flag is accepted only with the package policy; it is not silently
ignored by the default policy.

Local escape indicators are additional safeguards: native evidence must flag
`TestMain`, imports of `plugin`, `os/exec` or `unsafe`, `go:linkname`, direct
`os.StartProcess`/`syscall` process-launch use, and module-local cgo/assembly.
Their presence prevents package exclusions in this first release. This scan is
limited to active module-local sources and must not be presented as proof that
opaque dependencies never perform such operations. The caller's scope contract
is still necessary. Uninspectable active local inputs also prevent exclusions.

Use structured blocker codes: `test_main`, `dynamic_loading`,
`process_execution`, `unsafe_linkage`, `cgo_or_assembly`,
`outside_local_replacement` and `uninspectable_local_sources`. Imports must be
resolved despite aliases; process-launch references passed as function values
must not evade detection merely because the call uses a different identifier.

Reflection and callbacks wholly within an affected binary do not require
function-level completeness: the policy keeps that whole binary. This does not
extend coverage to plugins or source-consuming subprocesses. Existing graph
limitations remain visible and cannot be erased to certify the new policy.

## Eligible changes and existing input safeguards

Retain current Git, canonical-root, clean-checkout, committed-active-source,
revision, output-publication, offline-loading and Unix-supervision safeguards.
Unsupported module/workspace layouts remain rejected inputs, not successful
repository-wide selection reports. Do not mutate benchmark checkouts or install
dependencies during normal analysis.

Only nonempty comparisons where **every** changed element is a uniquely mapped,
body-only modification of an existing Go declaration are eligible. Its old/new
header, source ownership and package membership must be consistent. Under these
conditions the package/import structure has not changed, so candidate package
metadata suffices; no implicit baseline checkout or baseline dependency install
is needed.

If any file requires the existing module fallback, run the whole module:
configuration/resources, module/build files, imports/packages/build constraints,
global/type/initialization edits, additions, deletions, renames, signature changes,
inactive/unmapped source, invalid ranges, parse errors or ambiguous ownership.
A body edit to a helper used by initialization remains eligible: dependency
closure selects every importing binary, including initialization-only consumers.
An explicit `init` edit remains broad under the existing mapper.

Local module replacements outside the verified checkout prevent exclusions.
Metadata must identify such replacements rather than mistake their source for
immutable module-cache dependencies. This restriction does not change the
existing conservative analysis path's supported loading behavior.

An empty committed diff retains the current no-op behavior: skip discovery and
graphs, select nothing, and report `no_changes`, not a verified empty inventory.

## Native package evidence, independent of SSA

Add a separate `--mode package-inventory` native operation with a version-1
envelope carrying provider identity, source root, build scope and package
evidence. Leave ordinary inventory and its schema-1 JSON contract unchanged.
Legacy providers that do not support this mode cause package-policy fallback;
their ordinary inventory remains usable. Do not infer completeness from
nonempty lists or fabricate evidence from declaration names.

The envelope contains status, an explicit completeness flag, package nodes,
test-binary roots, structured blocking-feature records and diagnostics:

- Each node has the opaque Go loader package ID, canonical import path, direct
  imported package IDs, module provenance, and active source files for local
  packages. Package IDs distinguish production and test-augmented instances;
  Rust must not synthesize or parse IDs from bracketed names or `.test` suffixes.
  Provenance explicitly distinguishes verified module source, module-cache
  dependencies, standard-library packages and generated test harnesses; a nil
  module record is not itself sufficient provenance for an unknown package.
- Include the **complete transitive package graph**, including external and
  standard-library intermediary nodes. A path through an external dependency
  can return to another package in the main module; dropping intermediary nodes
  could incorrectly certify a binary as independent.
- Each execution package records its import path, module-relative directory and
  the roots that jointly represent its compiled binary. Group internal tests,
  external `_test` packages and their initialization dependencies together.
  Do not use only the package containing an individual test declaration.
- Enumerate execution packages with active test files even when they contain no
  top-level `TestX`: examples, fuzz seeds, benchmarks and setup-only packages
  cannot silently disappear from the execution decision. The existing per-test
  inventory remains specifically an inventory of top-level `TestX` functions.
- Record source-file ownership across all package instances. A production file
  can belong to normal and test-augmented nodes. A test-only file belongs to its
  test instances, not every production consumer with a similar import path.
- Distinguish ordinary module-owned source from the Go-generated test harness.
  Generated harness files outside the checkout are not committed local source
  and must not be misreported as such. Their imports may be represented, but
  binary grouping must be grounded in the actual internal/external source
  package instances, not a guessed synthetic-main name.
- Blocking features have stable codes, source provenance where applicable and
  explanatory diagnostics. An incomplete local feature scan prevents exclusions;
  free-form old diagnostics are not parsed as capability evidence.

Collect graph metadata without constructing SSA or parsing dependency bodies.
A metadata-only dependency load must not enable dependency syntax loading merely
to enumerate imports. Reuse the same root, scratch module/sum protections,
offline environment, build flags and Go
toolchain context as normal inventory. Invoke this operation only for requested
package selection after ordinary discovery and body-only mapping succeed. Give
it its own supervised invocation deadline and stage timing. An error, unsupported
mode or timeout cannot erase the already completed ordinary inventory; it forces
run-all fallback. Default conservative requests do not invoke this new operation.

Record the effective Go driver/toolchain version, GOOS/GOARCH, CGO setting,
architecture feature settings, GOEXPERIMENT and normalized flags. Do not identify
the loading toolchain solely by the version that compiled the provider. Validate
agreement with ordinary inventory and its active file set. This first policy
does not model alternate race/coverage/compiler-flag builds; execution under
such a different context is outside the contract, not certified by a normal
analysis. The control runner must apply the recorded context when running tests.

The existing scope envelope, provider identity and committed-file checks apply
to package evidence. The trusted prebuilt-provider contract is unchanged; this
is not protection against a deliberately malicious executable.

## Rust validation and package decision

Validate the package capability independently of graph output. Reject unknown
capability versions, failed/incomplete status, duplicate node IDs, dangling
import references, missing roots, invalid execution directories, foreign local
source paths, missing provenance and contradictory file ownership. Every active
local inventory file must have consistent package ownership. Every discovered
test must belong to one known execution package whose roots include that test's
source-owning instance. Enumeration must cover all active test-file groups,
including groups without ordinary TestX entries.

Use the existing committed-source verification for local files used by this
evidence as well. Package metadata, ordinary discovery and mapping must agree on
build context and source identity. Missing/legacy/invalid capability data causes
run-all fallback with an explicit reason, not a new exclusion opportunity.

For an eligible comparison:

1. Let C be all package-instance IDs owning any changed source file, using exact
   native ownership rather than directory-prefix or import-path guesses.
2. For each execution package, traverse imports from the union of its binary
   roots, including the roots themselves. Traverse through nonlocal nodes too.
3. Select its entire binary iff that closure intersects C. Otherwise exclude
   its entire binary under the declared linked-binary contract.
4. Partition every discovered TestX by its execution-package decision. Never
   select some TestX and exclude another TestX in the same binary.

Selection needs complete package evidence but not successful CHA analysis.
A failed/timed-out graph yields no suggestions; it does not invalidate an
independently complete import graph. Unresolved function callbacks likewise
cannot justify per-test cuts but need not block independent-package exclusion.
If valid positive graph evidence contradicts a proposed binary exclusion, treat
that inconsistency as run-all fallback, not as permission to discard the positive
evidence. Invalid graph responses are unavailable evidence, never an alternative
exclusion proof.

If any eligibility or evidence check fails, select the whole module and state
why. If eligibility succeeds but all binaries depend on C, retain them all and
report that no execution reduction occurred. If none depend on C, an explicitly
empty execution-package set is valid under the stated contract. Consumers must
not turn that empty set into bare `go test`, which would run a default package.

## Reports and execution instructions

Keep the existing default conservative command/report behavior and Java JSON
unchanged. Preserve the current conservative runtime entrypoint; expose policy
selection through a separate policy-aware entrypoint rather than require old
GoImpactOptions callers to provide new fields.

Package-policy requests produce Go report schema version 2, including when they
fall back. Default conservative requests retain report schema version 1. Version 2
retains existing identities, stages, graph suggestions and per-test lists, and
adds requested policy, declared test scope, effective build context,
package-evidence status, `execution_inventory_complete` and
discovered/selected/unselected execution-package lists. These lists are complete
only when the independent package evidence was successfully validated; ordinary
TestX discovery alone cannot certify the example/fuzz-only execution groups.
Fallback with unavailable package evidence explicitly marks execution inventory
incomplete and requires a whole-module run, never just its known package list.
An empty-diff no-op marks it incomplete and skipped, not an empty proven suite.
Uncollected effective build context is explicitly unavailable on skipped/failed
metadata operations; do not fabricate it for an empty-diff report.
Each execution package
includes its canonical import identity and validated module-relative directory.
Binary selection/exclusion reasons identify changed owners and the dependency
intersection or its absence. Missing evidence is explicitly unavailable, not an
empty successful list.

Use `selection_mode: conservative_packages` when the package decision is valid,
even if all binaries happen to be selected. Set `run_all_tests` true iff a whole
module run is required or a nonempty set of discovered execution packages is
entirely selected; it is false for a proper subset or an eligible empty selection.
Broad fallback uses the existing `conservative_all` mode. Empty diffs retain
`no_changes` and false.

The CLI remains an analyzer, not a test runner. For a package subset the consumer
must execute **whole selected packages**, without a TestX-only filter, so examples,
fuzz seeds and package initialization are included. Exact execution arguments are
`.` or `./<relative-directory>`, validated to stay inside the checkout and avoid
Go wildcard/metapattern expansion such as `...`. Publish argument data, not an
interpolated shell command. Nonrepresentable exact-package arguments cause broad
fallback. Analysis and execution must share build tags/platform and test scope.

Graph suggestions remain diagnostic detail, not execution instructions. Counting
fewer graph suggestions is not execution reduction; report selected/excluded
binaries separately from selected/excluded TestX declarations.

## Review gates and acceptance tests

1. Native evidence proves direct/transitive imports, a dependency path through
   an external module, internal/external test-root grouping, initialization-only
   imports, test-file ownership and example/fuzz-only execution packages. It is
   graph-free and preserves inventory on capability failure.
2. Rust validation rejects incomplete/dangling/duplicate/foreign evidence,
   missing test roots, inconsistent source ownership, build drift, external local
   replacements and legacy capabilities. Test missing evidence separately from
   a genuinely empty module inventory.
3. Selection tests use multiple test binaries. Keep the whole importing binary,
   exclude independent binaries, preserve external/internal siblings, and handle
   helper-only test changes, multiple owners, all-affected and none-affected
   cases. Graph failure must not replace dependency evidence; contradictory
   positive evidence must force fallback.
4. CLI tests verify opt-in/scope validation, unchanged default schema1 behavior,
   schema2 fallback, exact execution-package reporting, empty-set handling,
   config/unmapped changes and each supported blocking-feature code. Existing
   atomic-publication, clean-input and process-deadline protections still pass.
5. Add a one-command real-project control on isolated copies. Compare its
   package decisions with independent `go list -test`/`go test` observations and
   separately executed mutations. Begin with go-micro-services and
   go-coffeeshop; do not require downloads during analyzer execution.
   Expected acceptance targets, to verify rather than assume: an isolated
   profile/geo/rate implementation edit keeps that service's binary and excludes
   independent service test binaries; coffeeshop keeps both tests in its shared
   binary. Frontend changes keep every frontend test, not just graph suggestions.
   The augmented sort-callback case remains selected regardless of CHA coverage.
6. Configuration/signature/deletion/initialization changes and unsupported scope
   must still fall back. Preserve artifacts, record exclusions and observed
   mutation failures separately, and require zero observed omitted failing
   binaries. Report that this empirical gate is not a general soundness proof.
7. Verify toolchain/context drift and missing execution-inventory completeness
   independently of ordinary TestX completeness. Metadata timeouts and unsupported
   modes retain basic discovery but require a module run; a default policy never
   invokes package metadata.
8. Run native race/vet checks, native/Rust adapter controls, the complete Rust
   workspace tests and the existing default seven-discovered/five-suggested/
   seven-selected control. No Java provider, default Go policy or protocol1
   conservative behavior regressions are accepted.

## Deferred work

Individual-test exclusions inside an affected binary, arbitrary integration-test
safety, automatic proof of dependency side-effect behavior, cross-module/workspace
or distributed impact, broad reflection/plugin/process modeling and default
enablement remain separate work. Package-level results must not be marketed as
solving those problems.
