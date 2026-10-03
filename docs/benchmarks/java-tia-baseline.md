# Conservative Java TIA baseline benchmark

This benchmark is a controlled Maven/Git fixture used by the CLI integration
test. It demonstrates the contract of the current Java baseline; it is not a
claim of production recall for Spring or distributed systems.

## Fixture

The fixture has one Maven module, `fixture:service`, with two JUnit-style test
methods:

- `ServiceTest.coversChanged` calls `Service.changed`.
- `ServiceTest.coversUnchanged` calls `Service.unchanged`.

The test provides a source-local `org.junit.jupiter.api.Test` annotation, so it
does not require a repository download. Maven compiles both source sets; the
WALA adapter uses the discovered test methods as bytecode entry points.

## Controlled version pairs

| Candidate change | Expected affected tests | Selected / total | Reduction | Recall |
| --- | --- | ---: | ---: | ---: |
| Body of `Service.changed` | `coversChanged` | 1 / 2 | 50% | 100% |
| `application.properties` added | all module tests | 2 / 2 | 0% | 100% |
| `Service.java` deleted | all module tests | 2 / 2 | 0% | 100% |

The automated integration cases are `cli/tests/test_impact_cli.rs`. They assert
both the narrow method result and mandatory broad fallbacks. A representative
adapter result additionally records `analysis_duration_ms`; Maven preparation
and source extraction are outside that adapter-only metric.

## Interpretation and limits

The callable pair validates the complete local route: `git diff` → Tree-sitter
owner mapping → JUnit discovery → Maven test compilation → WALA test-root graph
→ resolved edge traversal → JSON selection. The configuration and deletion
pairs validate that missing static evidence cannot silently reduce the suite.

The measurable suite reduction in this fixture is 50% only for the isolated,
direct static call. Current limitations are intentionally conservative:

- Maven is automated; Gradle requires explicit compiled-class inputs.
- Spring DI, AOP/proxies, reflection, dynamic tests, and framework callbacks
  are not converted into graph edges.
- Cross-service HTTP, gRPC, Kafka, and RabbitMQ paths are not part of this
  local Java TIA baseline.
- WALA test-bytecode preparation can install local reactor artifacts and adds
  build time; a timeout or any non-timing diagnostic falls back to module scope.

Future benchmark work should add real multi-module Spring applications,
framework-mediated calls, asynchronous communication, known-failing tests, and
larger version histories. Report affected-test recall, selection reduction,
WALA time, Maven preparation time, and total wall-clock time separately.
