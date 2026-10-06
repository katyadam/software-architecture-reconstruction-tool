# Java TIA module metadata design

## Goal

Replace marker-only Java module discovery with declared Maven and Gradle module
metadata. TIA must identify the owning module for a changed path and the test
modules below a changed aggregator without traversing arbitrary nested build
directories. When metadata cannot be interpreted with certainty, selection
must broaden to the enclosing build root rather than omit tests.

## Scope

This design supports conventional, statically declared multi-module projects:

- Maven reactor `<modules><module>…</module></modules>` declarations;
- Gradle `include(...)` declarations in `settings.gradle` and
  `settings.gradle.kts` with literal project paths;
- Gradle literal `project(…).projectDir = file(…)` mappings.

It does not evaluate Maven profiles/properties, Gradle variables, functions,
conditionals, plugins, composite builds, or generated settings. Any such
construct that affects module membership makes the catalog incomplete and
triggers conservative root fallback for that build.

## Architecture

Introduce an `impact::java_modules` component in `extractor-runtime`.
`ModuleCatalog::load(project_root)` detects the build root, reads its metadata,
and returns either a complete catalog or an uncertainty diagnostic.

```text
Git changed path
        |
        v
ModuleCatalog ownership lookup
  complete -> declared closest module
  uncertain -> enclosing build root
        |
        v
TIA change mapping and test discovery
```

The existing Git mapper uses the catalog instead of walking up to the nearest
marker. Java test discovery receives the catalog's declared descendants rather
than recursively walking every directory. WALA keeps its current build-tool
preparation path; this change does not run Maven or Gradle to discover modules.

## Maven behavior

Parse reactor POM XML and retain direct `<modules>` entries from the project
model. Resolve each literal path relative to the declaring POM; recursively
read child POMs only when they are declared by their parent. Paths outside the
project root, missing declared directories, malformed XML, property expansion,
or profile-dependent module declarations make the catalog uncertain.

For a complete catalog, a changed file belongs to its deepest declared module.
A root POM change owns the reactor root and discovery includes only declared
descendants. A nested unlisted POM is not treated as a reactor module.

## Gradle behavior

Read the root `settings.gradle` or `settings.gradle.kts`. Recognize literal
`include` invocations and colon-delimited project paths. By default `:a:b`
maps to `a/b`; recognize literal `project(':a:b').projectDir = file('path')`
or Kotlin equivalent to override that path. A settings file using unsupported
expressions, variable interpolation, nonliteral include arguments, or
conflicting mappings makes the catalog uncertain.

For a complete catalog, a changed file belongs to the deepest declared project
directory. Discovery visits only those declared projects. A project directory
with a build file but no settings inclusion is excluded from the root's
descendant test set.

## Failure and fallback behavior

No metadata failure narrows selection. An uncertain catalog assigns affected
paths to the enclosing build root and emits an auditable diagnostic explaining
why declared module membership was not used. Existing module-wide fallback
policy then selects every discovered test below that root. Single-module Maven
and Gradle projects remain complete catalogs without declared descendants.

## Testing

Add unit and integration coverage for:

1. Maven reactor ownership and exclusion of an unlisted nested POM.
2. Gradle Groovy and Kotlin literal `include` ownership.
3. Gradle literal `projectDir` override mapping.
4. Dynamic/unsupported Maven and Gradle metadata producing root fallback plus
   diagnostics.
5. Existing root-plus-child change deduplication with declared descendants.

The existing CLI fixture tests continue to verify end-to-end selection. Tests
assert module paths and selected tests, not parser implementation details.

## Compatibility

Public CLI arguments and JSON schema do not change. New metadata uncertainty
diagnostics append to the existing `TestImpactResult.diagnostics` array.
