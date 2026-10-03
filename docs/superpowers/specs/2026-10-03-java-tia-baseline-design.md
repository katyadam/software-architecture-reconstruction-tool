# Conservative Java Test Impact Analysis Baseline

## Goal

Provide a safe first Test Impact Analysis (TIA) workflow for Java systems. Given a
baseline Git revision and a candidate revision, the workflow must select every
Maven-module test that may be affected by a source or configuration change. It
may over-select tests; it must never exclude a test merely because analysis lacks
evidence.

## Scope

The first milestone supports Java projects built with Maven. It uses Git as the
revision source, Tree-sitter Java extraction to assign changed lines to source
elements, and WALA bytecode analysis to connect test methods to application
methods.

Automated Gradle preparation is out of scope. A Gradle user may supply compiled
main/test class directories and a test-scope dependency classpath to the WALA
adapter, but the TIA workflow will not invoke Gradle itself in this milestone.

The milestone covers JUnit 4 and JUnit 5 test methods, including ordinary
Spring test classes where individual test methods use those annotations.

## Non-goals

This milestone does not infer Spring bean injection, AOP proxies, MVC dispatch,
Kafka/RabbitMQ listeners, gRPC handlers, reflection, dynamic class loading, or
cross-service propagation. Those capabilities will be layered onto the local
baseline only after its safety policy and evaluation are established.

## Inputs and Changed-Element Model

The workflow accepts a baseline and candidate Git revision. It obtains changed
paths, status, and zero-context changed line ranges with `git diff --name-status
--unified=0`.

For modified or added Java files, Tree-sitter maps candidate-side ranges to the
smallest containing callable. If no callable contains a changed range, the
containing class is changed. An added or deleted Java file is treated as a
class/module-level change. Renamed files, non-Java source files, resources,
properties/YAML files, schema files, Maven POMs, and any unparsable diff are
module-level changes.

Every changed item records its module, source path, kind, and optional callable
identity. The old revision is used to identify deleted paths; a deleted callable
does not need to be reconstructed exactly because the affected module fallback
is required.

## Test Inventory and Graph Construction

Tree-sitter discovers test methods by JUnit 4/5 annotations, with class and
method identities resolved in the same manner as ordinary Java callables. Test
classes are compiled with Maven's test-compile phase. The WALA adapter receives
both application and test class directories plus test-scope dependency JARs.

Discovered test methods become explicit WALA entry points. The adapter emits
test-originating and application-originating call edges. Existing conservative
source matching resolves WALA method references to VoyantClair callable IDs;
ambiguous matches are recorded as diagnostics rather than guessed.

## Selection Policy

For a callable-level change, select every test that can reach the changed
callable through the reverse local call graph. For a class-level change, select
tests reaching any callable in that class. Deduplicate selected tests and retain
all selection reasons.

For every module-level change, unresolved WALA edge, missing bytecode, unknown
test annotation, reflection/dynamic behavior indicator, or mapping ambiguity,
select the module's complete discovered test inventory. This fallback is the
primary safety mechanism.

The result distinguishes selected tests, fallback-selected tests, unselected
tests, diagnostics, and machine-readable reasons. An empty selection is allowed
only when the candidate has no changes or the affected module has no discovered
tests.

## Maven Preparation

Maven preparation must build the selected module and required reactor upstream
modules using test-compile, discover `target/classes` and `target/test-classes`,
and obtain the test-scope dependency classpath. Preparation remains bounded by
the WALA provider timeout policy. A failure is not silently ignored: it invokes
the affected-module fallback and reports its cause.

## Verification and Evaluation

Tests must cover:

- a changed directly called application method selecting only its reaching test;
- a changed class-level declaration selecting every test reaching that class;
- a deleted/renamed/configuration/POM change selecting the whole module;
- a missing classpath or unresolved WALA mapping selecting the whole module;
- JUnit 4 and JUnit 5 discovery;
- a Maven end-to-end fixture with test classes and test-scope dependencies.

Evaluation reports affected-test recall before reduction. The desired safety
criterion is zero observed affected-test omissions on controlled version-pair
fixtures; reduction rate and analysis time are secondary metrics.

## Future Extensions

Framework entry points, Spring DI/proxy edges, configuration-to-bean links, and
method-to-HTTP/gRPC/Kafka/RabbitMQ edges will enrich this local graph. Each must
be labeled with its evidence/confidence and must preserve module fallback when
uncertainty prevents a safe exclusion.
