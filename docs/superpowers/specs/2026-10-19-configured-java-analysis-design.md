# Java analysis inputs in existing service configurations

## Intent and scope

The user already supplies `*-config.json` files describing microservices and
their directories. Reuse those files for Java test-impact analysis (TIA), adding
optional explicit build inputs where they improve ownership, discovery and
source/bytecode matching. Avoid a second implementation of Maven or Gradle.
Existing configurations and invocations without a configuration must continue
to work.

The approved conversational design is an optional `java_analysis` object per
service, a build-artifact manifest, and `--config-file` support in the standalone
`test-impact` command. This document specifies the first implementation. It
consumes manifests supplied by CI or a build integration; it does not introduce
a new automatic manifest-generator subsystem. Automatic export can subsequently
extend the existing artifact preparer against this contract.

Success means both WALA and SootUp can consume the same declared inputs, including
nonstandard source/test directories, without recompiling manifest-backed modules.
Unsupported or incomplete scope remains visible and cannot justify exclusions.
Configuration improves analysis inputs; it does not certify graph completeness.

## Current behavior and constraints

- `models::ConfigurationData` contains `service_descriptions`; each service has
  `name`, `base_dir_path` and `urls`.
- Reconstruction already resolves a service as
  `project_dir.join(service.base_dir_path)`. Keep that behavior.
- Standalone Java TIA currently accepts no service config and treats
  `--project-dir` as the Git checkout used for its one baseline/candidate pair.
- TIA source extraction and test discovery currently assume conventional roots.
- Java adapter invocation already supports explicit application/test class
  directories and dependency classpath entries. Extend that path instead of
  creating another provider implementation.
- Build modules and deployable services are not interchangeable. Shared libraries
  and nested modules can exist outside the service list.
- The existing Train Ticket config uses paths such as
  `train-ticket/ts-order-service`; some local configs use `../train-ticket/...`.
  Do not silently strip components to guess the intended root.

## Approaches considered

1. **Recommended: optional configuration plus supplied resolved manifest.**
   Keep stable service settings in the existing config and build-specific paths
   in generated metadata. Reuse Maven/Gradle/CI resolution and the existing
   provider protocol. This requires a manifest producer but avoids reproducing
   build semantics inside Rust.
2. **List every class directory and dependency JAR directly in each config.**
   Easy initially, but brittle across dependency versions, machines and build
   profiles. Do not use this as the maintained default.
3. **Continue reconstructing everything from raw build scripts.**
   No new input format, but duplicates known service ownership and cannot
   accurately evaluate arbitrary build logic. Retain the existing automatic path
   for compatibility, without expanding its parser as part of this work.

## Configuration extension

Add an optional field with deserialization default and omitted serialization
when absent:

```json
{
  "service_descriptions": [
    {
      "name": "ts-order-service",
      "urls": ["http://ts-order-service"],
      "base_dir_path": "train-ticket/ts-order-service",
      "java_analysis": {
        "maven_profiles": ["ci"],
        "artifacts_file": "target/tia-artifacts.json"
      }
    }
  ]
}
```

`ci` is an example, not an asserted Train Ticket profile.

- `java_analysis` is optional, so existing Java and non-Java services remain
  compatible. Do not infer that a service is Java merely from its name.
- `maven_profiles` is an optional list, default empty. It declares explicitly
  requested Maven profiles, not arbitrary Maven command text. It is matched
  against the manifest's requested profiles; record effective active profiles
  separately because defaults and automatic activation can add profiles.
- `artifacts_file` is optional and resolves relative to the configured service
  directory. A relative path must stay within that service directory. Permit an
  explicit absolute manifest path for CI artifacts.
- An empty `java_analysis` uses the existing automatic behavior with configured
  ownership. Nonempty requested profiles without a manifest are rejected with
  an actionable error in v1, rather than silently preparing the default profile.
- Do not change all existing benchmark configuration files to contain guessed
  profiles or nonexistent manifests. Add a documented example and test fixtures.

## CLI and path rules

Add optional `--config-file FILE` and `--config-base-dir DIR` to `test-impact`.
`--config-base-dir` requires `--config-file` and defaults to `--project-dir`.
Resolve it explicitly, never relative to the JSON file's location by accident.
Relative CLI paths remain relative to the process working directory, as usual.

`--project-dir` remains the candidate Git root. Resolve each service using the
configuration base directory plus `base_dir_path`, then canonicalize. This lets
the unmodified Train Ticket config work with its original path convention:

```text
test-impact --project-dir /benchmarks/java/train-ticket
            --config-file /tool/config/configurations/train-ticket-config.json
            --config-base-dir /benchmarks/java
            --baseline-revision BASELINE
            --wala-adapter-jar /tool/wala-callgraph/target/wala-callgraph-all.jar
            --output /tmp/train-ticket-impact.json
```

Only services inside the candidate checkout participate in this one-repository
comparison. Existing services outside it are explicitly reported as outside
scope; an outside service with requested Java analysis settings is an error so
the user cannot mistake it for an analyzed service. Missing directories,
duplicate canonical service roots and ambiguous overlapping configured service
roots are configuration errors. Library roots supplied by manifests are not
additional service declarations and can be nested as ordinary build modules.

Do not reinterpret baseline revisions across several Git repositories. Do not
change reconstruction's existing base-directory semantics or require this new
flag in its command.

## Artifact manifest contract, version 1

A manifest describes one service's candidate build and the in-checkout build
modules needed for its analysis, including shared libraries. Preserve distinct
application and library classes. The shape is:

```json
{
  "schema_version": 1,
  "candidate_revision": "FULL_CANDIDATE_COMMIT_SHA",
  "build_context": {
    "tool": "maven",
    "requested_profiles": ["ci"],
    "active_profiles": ["ci"],
    "java_version": "17"
  },
  "module_inventory_complete": true,
  "modules": [
    {
      "root": ".",
      "source_roots": ["src/main/java"],
      "test_source_roots": ["src/test/java"],
      "application_class_dirs": ["target/classes"],
      "test_class_dirs": ["target/test-classes"],
      "classpath": ["/dependency-cache/example/library/1.0/library-1.0.jar"],
      "dependencies": [],
      "dependency_inventory_complete": true,
      "test_inventory_complete": true,
      "tests": []
    }
  ]
}
```

The empty test list in this structural example explicitly describes zero tests;
it is not a placeholder for undiscovered tests. An actual producer must populate
it or set completeness false. A source file with known tests cannot be
misrepresented as complete/empty by an invalid producer.

Path and identity rules:

- Module `root` paths resolve relative to the configured service. A shared module
  may use a parent-relative path, but its canonical root must stay inside the
  candidate Git checkout.
- Source roots, test roots, application/test class directories and relative
  classpath entries resolve relative to their module root. Source and output
  directories must remain in the candidate checkout. Absolute external dependency
  JARs/directories are allowed; there is no shell expansion or wildcard expansion.
- `dependencies` names direct in-checkout module roots relative to the same
  service base as module `root` values. Every dependency must resolve to a module
  in the merged manifest inventory. External dependencies belong in `classpath`.
- Generated source roots can be included. Their source ownership must not be
  confused with committed changed files; unresolved generated-source relationships
  require broad fallback.
- `tests` uses the existing `JavaTestCase` identity fields: module root, qualified
  class name, method name, JVM descriptor, callable signature and source path.
  Normalize path fields under the preceding rules and validate identity fields
  before invoking either provider. Manifest-backed source/test roots feed source
  extraction and source resolution as well as test discovery.
- `test_inventory_complete` is a producer assertion that the listed supported
  method roots correspond to the effective build's test discovery and filtering.
  It does not assert reflective/callback graph completeness. Unsupported dynamic
  test forms or selectors must be marked incomplete; do not invent precise roots.
- `module_inventory_complete` covers the service's declared build scope, not
  every service or remote consumer. Global exclusion of a sibling requires complete
  in-checkout consumer evidence across the merged project inventory.

Do not store runtime credentials, environment dumps, raw shell commands, provider
algorithms, or copies of all Java source files in this manifest.

## Validation, precedence and provenance

Manifest-backed input takes precedence for the declared modules. Services with
no manifest retain supported automatic discovery/preparation. Merge shared-module
entries only when canonical roots, paths and context agree; conflicting entries
are errors, not last-writer-wins overrides.

Check schema version, full candidate revision, requested profile set, readable
paths, unique identities, declared dependencies and test membership. Every input
must be validated before any narrow selection is published. Non-Java changed
files outside declared services still receive project/module fallback; the service
config is never an allowlist for silently dropping changed shared code.

Record service, resolved module/source/test roots, manifest path/content digest,
requested and active profiles, JDK and candidate revision in a new optional
`analysis_contexts` result field. Existing result fields retain their meanings.
Old consumers can keep reading the existing selection arrays; absence of contexts
means legacy automatic analysis, not a new completeness guarantee.

A revision string and existing class files cannot prove that bytecode was built
from that revision. The manifest producer/CI must supply a fresh build and correct
metadata. State this producer contract in the docs and report; do not claim to
detect all stale bytecode. The manifest records build provenance, while validation
rejects observable inconsistencies such as a different revision or missing files.

## Runtime flow

1. Parse CLI and configuration, resolve the candidate checkout and service paths.
2. Load available manifests, normalize and validate them, and form a common
   Java analysis context. Keep the no-config path unchanged.
3. Obtain Git changes once for the candidate checkout. Use explicit module/source
   roots for ownership where available; retain automatic/fallback ownership for
   unconfigured libraries and project-level changes.
4. Discover source callables from declared roots and consume validated manifest
   test roots. For automatic modules, retain existing discovery. Incomplete
   effective-profile discovery blocks narrow decisions.
5. Resolve local consumers from complete supplied dependencies plus supported
   automatic metadata. Missing consumer evidence keeps project-wide fallback.
6. Feed declared class directories and classpath to the existing Java adapter
   invocation for both WALA and SootUp. Bypass build preparation for those
   modules, including the legacy WALA source-preparation path.
7. Retain existing graph/source matching and conservative selection policies;
   append context provenance and explicit uncertainty diagnostics to the report.

The reconstruction CLI must deserialize the extended config without breaking
service assignment. When WALA enrichment is enabled, manifest-backed classpaths
and source roots must be honored through the same shared input loader. Plain
reconstruction without graph enrichment must not require build artifacts merely
because optional Java analysis settings are present.

## Failure behavior

- Invalid explicit user input, unreadable required manifests, revision/profile
  mismatches, conflicting contexts or invalid paths fail clearly before analysis.
  Do not silently substitute an unrequested default-profile build.
- A valid manifest can declare incomplete test or dependency scope. In that case,
  publish an explicit `requires_full_test_run` indication and diagnostics for the
  affected scope; never present the available method list as a complete execution
  list. Do not populate `unselected_tests` for uncertified scopes. Existing broad
  selections remain available as suggestions, not a complete execution contract.
- A failed graph or unresolved source retains existing broad selection behavior.
- Overlapping timers remain diagnostics; manifest consumption should separately
  record that build preparation was skipped rather than report a fake zero-cost
  graph computation.

For legacy runs, preserve report compatibility and existing behavior. The new
full-run indication must default to absent/false for legacy deserialization and
be documented for consumers of manifest-backed reports. Exact result type changes
belong in the implementation plan, preserving the semantics above.

## Tests and acceptance criteria

1. All existing configuration JSON files deserialize unchanged. Serialization of
   legacy service entries does not add a null Java object. Existing service
   assignment and non-Java reconstruction remain unchanged.
2. Configured Train Ticket-style paths resolve using an explicit base directory;
   relative, absolute, outside-checkout and ambiguous paths have tested behavior.
3. Both providers consume a prepared nonstandard-root fixture and select the test
   reaching the changed method without launching Maven/Gradle preparation. A
   marker/failing preparation command proves preparation was bypassed.
4. Generated roots and dependency directories are retained when supplied, and
   duplicate application/library representations use existing deduplication.
5. A library change retains the actual failing consumer test while excluding an
   independent test only with complete dependency/test evidence. Missing evidence
   requests a full run rather than silently narrowing to configured services.
6. Two synthetic profile contexts differ in their effective test inventory and
   classpath. Each manifest is analyzed independently; a requested-profile mismatch
   is rejected and one context's report is never reused for the other.
7. Missing/stale/wrong-version manifests, missing classes, malformed test roots,
   duplicate/conflicting modules and unsupported discovery are explicit errors or
   full-run outcomes as specified. No successful empty suite masks missing data.
8. The ordinary no-config Java CLI and both providers' current controlled tests
   still pass. The Go CLI and its policy are unchanged.
9. Documentation provides a small working manifest/config fixture, the path rules,
   input precedence, producer freshness contract and execution/fallback meanings.

## Explicit non-goals

- Automatic support for every Maven profile/test selector or arbitrary Gradle
  script. Producers must evaluate the real build or mark scope incomplete.
- Automatically generating manifests for every benchmark in this first change.
- Spring profiles/bean graphs, reflection modeling, dynamic proxy analysis,
  interservice HTTP/message test impact, or changing provider defaults.
- Replacing Tree-sitter, WALA or SootUp, adding Spoon, or modifying Go support.
- Multiple Git repositories in one comparison or enumerating every possible
  profile combination. Analyze the contexts actually used in CI separately.

## Review state

Conversational direction approved by the user. This written specification is
ready for review. Implementation planning follows written-spec approval.
