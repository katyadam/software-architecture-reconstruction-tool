# Maven profile-aware Java TIA design

## Goal

Narrow Java test impact analysis for source changes in unconditional Maven reactor
modules even when a reachable POM contains Maven profiles.  Preserve conservative
selection whenever the active Maven profile set could change the answer.  Ensure
WALA's Maven bytecode preparation never runs project tests or integration tests.

## Module metadata model

The Maven parser will collect direct child module declarations in two categories:

- **Unconditional** declarations under `project/modules`.
- **Profile-defined** declarations under `project/profiles/profile/modules`.

Only unconditional declarations form the precise catalog used to assign changed
source files to a module.  Profile-defined declarations make the catalog
profile-sensitive, but do not discard the unconditional portion.

For a changed source path within a known unconditional module, TIA analyses that
module and its unconditional descendants.  For root-level build/configuration
changes, paths outside the unconditional catalog, and changes to profile metadata,
TIA must use the existing broad fallback.  This prevents a profile from silently
removing eligible tests.

Malformed XML, dynamic module paths, missing child POMs, duplicate module paths,
and paths escaping the project root remain metadata failures and retain the broad
fallback.

## WALA Maven preparation

`MavenBytecodeArtifacts` currently prepares the selected Maven reactor with
`install -DskipTests`.  Maven Failsafe integration tests may still execute during
that lifecycle.  Preparation will instead invoke the `test-compile` phase with
test execution disabled.  The phase produces main and test bytecode required by
WALA without installing artifacts or invoking Surefire/Failsafe tests.

Classpath resolution continues to use Maven's dependency-classpath goal after
bytecode preparation.  Any Maven command failure remains a structured
`test_root_preparation_failed` diagnostic and triggers safe broad test selection.

## Data flow and failure behavior

1. Parse Maven POMs into the unconditional catalog plus a profile-sensitivity
   marker.
2. Classify each changed file.  A known unconditional-module source change is
   analysed at that module; all uncertain classifications use the existing
   module/root fallback.
3. Discover tests only in the selected scope and invoke WALA with explicit test
   roots.
4. Run Maven `test-compile` only to prepare bytecode.  Do not run project tests.
5. On metadata or preparation failure, emit an actionable diagnostic and select
   conservatively.

## Verification

- Parser tests cover a POM containing both unconditional and profile-only
  modules, profile-only module source changes, and root POM changes.
- WALA preparer tests assert its Maven invocation uses `test-compile` and does
  not contain lifecycle goals that execute tests.
- Existing full workspace tests remain green.
- Re-run Debezium: its JDBC source diff should be scoped to the JDBC module;
  its diagnostic must no longer contain Testcontainers/Docker failures from
  preparation.
