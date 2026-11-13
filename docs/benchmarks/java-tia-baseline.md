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

- Maven and conventional Gradle Java projects are automated. Gradle uses
  `testClasses` and `testRuntimeClasspath` through its wrapper/init-script path;
  explicit compiled-class inputs remain available for CI artifacts.
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

## Local real-project check

Run `scripts/run-java-tia-benchmarks.sh` from this repository to compare each
selected sibling benchmark project's `HEAD~1` to `HEAD`. It defaults to the
two Spring projects exercised during development, writes JSON results to a new
`/tmp/sar-java-tia-benchmark.*` directory, and prints selection counts plus
the TIA stage timings. Pass one or more project names to override that default
set, or set `JAVA_BENCHMARK_ROOT` when the sibling checkout is elsewhere.

## Direct and transitive provider comparison

Run this single command from the repository root:

```bash
scripts/run-spring-stream-comparison-tia.sh
```

The script creates a fresh detached worktree of the sibling
`sample-spring-cloud-stream-kafka` repository. It adds three baseline control
tests to `order-service`: a direct call to `Order.getAmount()`, a call through
`OrderImpactFacade.currentAmount()`, and an unrelated call to `Order.getId()`.
It executes these controls at baseline, then commits a candidate changing only
the return value in `Order.getAmount()`. The direct and transitive tests must
fail under the mutation; the unrelated control must pass. Docker-dependent
startup tests are not executed.

Both independent adapters analyze the same candidate against the same baseline.
The fresh `/tmp/sar-spring-stream-comparison.*` directory retains the worktree,
JUnit reports, execution logs, individual provider JSON results, and
`results/comparison.json`. The comparison records selected and excluded tests,
selection reasons, timings and fallback diagnostics, whether all observed
failures were selected, and whether the unrelated control was excluded.
Provider failures and conservative selections remain visible in this report.

Set `TIA_SPRING_PROJECT_DIR` to override the source checkout or
`WALA_TIMEOUT_SECONDS` to override the per-process timeout (default 300 seconds).
The adapters are built through the shared Maven reactor before analysis; their
dependencies remain in separate JARs. Each run preserves its artifacts for
inspection. This control measures local direct and transitive calls; it does
not validate Spring injection, reflection, or the existing empty startup test.

Verified on 2026-10-17 using source checkout `0419246`, control baseline
`9ac27f7`, and mutation candidate `f7b28a5`:

| Provider | Selected | Excluded | Graph duration | Analysis stage including preparation |
| --- | --- | --- | ---: | ---: |
| SootUp CHA | `readsAmount`, `readsAmountThroughFacade` | `readsId`, `startup` | 6.17s | 11.17s |
| WALA CHA | `readsAmount`, `readsAmountThroughFacade` | `readsId`, `startup` | 44.53s | 48.00s |

Both providers attributed selection to `Order.getAmount()` and emitted no
fallback diagnostic. The executed controls passed at baseline; only
`readsAmount` and `readsAmountThroughFacade` failed after mutation. These are
single-run timings, with SootUp run first and WALA second, so they do not
establish a general performance ranking. The result excludes tests only within
the affected `order-service` module; it does not count the other reactor module
as an additional suite reduction.

## Dispatch, reactor and Spring injection controls

Run the small synthetic reactor fixture with:

```bash
scripts/run-java-dispatch-comparison-tia.sh
```

This is a controlled fixture, separate from the real-project Spring benchmark
above. It requires Java 17+, Maven, Cargo, Git, jq and Python 3. The current fixture
has a library, a consuming app, and an independent module. All five tests
must pass at baseline. Each candidate changes exactly one production method,
then executes the whole reactor suite to identify the failing test:

| Candidate | Changed method | Observed test expected to fail |
| --- | --- | --- |
| Interface dispatch/override | `OverriddenQuote.amount()` | `interfaceDispatch` |
| Cross-module dependency | `library` module's `SharedPrice.value()` | `app` module's `crossModule` |
| Spring injection | `QuoteConfiguration.quote()` bean factory | `springInjection` |

The interface test calls through the `Quote` interface; a second test calls
the unchanged base implementation. The injection test uses Spring's real
JUnit extension and an `@Autowired` field. Its mutation changes the bean's
configured value, so finding the directly called `InjectedQuote.amount()` alone
is insufficient: analysis must account for the bean factory callback.

Each scenario has a fresh detached worktree and the same baseline for WALA and
SootUp. Before analysis, the candidate reactor is installed into the local
Maven cache to avoid using a stale dependency from a previous scenario. Runs
using these same fixture Maven coordinates should be sequential. Adapter
preparation still follows each provider's existing path; the comparison
therefore measures the whole TIA pipeline, including test discovery.

The script preserves baseline and mutation JUnit XML, logs, individual TIA
results and a combined `comparison.json` under a fresh
`/tmp/sar-java-dispatch-comparison.*` directory. The report explicitly lists
missed observed failures and failures absent from both discovered test sets.
Consequently a zero-test result cannot be mistaken for successful reduction.
`scripts/junit-failing-methods.py` parses JUnit XML rather than matching XML
with regular expressions; its regression checks run with
`python3 scripts/test-junit-failing-methods.py`.

Verified on 2026-10-17 in `sar-java-dispatch-comparison.IGrdRQ`:

| Control | Observed failing tests | SootUp selection | WALA selection |
| --- | --- | --- | --- |
| Interface/override | `interfaceDispatch` | Affected test plus `baseImplementation` | Affected test plus `baseImplementation` |
| Cross-module | `crossModule` | No tests discovered; failing consumer test missed | No tests discovered; failing consumer test missed |
| Spring bean factory | `springInjection` | All 4 tests through unresolved-source fallback | No tests selected; failing injection test missed |

The interface case has an extra selection consistent with CHA's broad dispatch;
it is not evidence of precise receiver tracking. The cross-module miss occurs
before graph construction: the pipeline discovers tests inside the changed
library module but not in its sibling consumer. This is a shared TIA scope
limitation and invalidates a project-wide conservative-selection claim for this
case. The Spring result does not establish framework support for either
backend. SootUp's fallback happens to retain the affected test; WALA reports a
successful analysis but does not include the bean factory dependency in the
test's reachability.

Analysis-stage times including preparation were respectively SootUp/WALA:
6.27s/5.22s (dispatch), 5.61s/3.32s (cross-module), and 6.14s/5.19s
(injection). The cross-module timings describe an incomplete test scope, so
they are not useful call-graph performance measurements. All are single runs.
The script intentionally records missed tests as evaluation results instead
of treating them as harness execution errors.

### Initial cross-module safety fallback

The initial safety fix added project-wide test discovery before finalizing a
result. For production or configuration changes, tests outside the changed
module are selected with a `cross-module dependency scope unresolved` module
fallback reason. This applies to Maven and Gradle and to both providers,
including the fast path that skips graph construction for module-level changes.
Existing local graph decisions are retained; tests already selected are not
duplicated, and tests previously excluded in another changed module are promoted
when needed. Conventional `src/test/`-only changes do not trigger this widening.

This is deliberately broad: the catalog knows module ownership but does not
resolve reverse dependency relationships, so independent sibling modules may
also be selected. The change closes the demonstrated missing-consumer test
scope; it does not add cross-module graph edges or fix Spring callbacks.
Consumers outside the supplied project checkout are outside this scope.

Re-running the controls in `sar-java-dispatch-comparison.1nD95O` confirms that
both providers select all four consumer-module tests for the library mutation,
including the observed failing `crossModule` test. The previous empty result
is replaced by an explicitly labelled fallback. Interface-dispatch and Spring
injection outcomes are recorded separately and retain their known limitations.

### Dependency-aware cross-module selection

The current implementation resolves direct and transitive consumers before
selecting tests. For a callable change it prepares the consumer and upstream
application modules, then asks the selected provider which consumer tests reach
the changed library method. Reactor JARs already supplied as application class
directories are removed from the library classpath to prevent loader shadowing.
Independent tests are explicitly listed as unselected. Configuration changes
still select all tests in affected consumers, without invoking a graph.

Supported metadata is intentionally explicit: Maven reactor dependencies with
literal/resolved local coordinates, local parent inheritance and POM properties;
and literal Gradle project dependencies in simple Java/Groovy/Kotlin build files.
Profiles, dependency management, external parents, dynamic versions or properties,
Gradle conventions, relocated projects and arbitrary build logic cause a named
metadata fallback. Such uncertainty retains the initial broad safety behavior;
the implementation never treats an unsupported declaration as proof of independence.

Run the same control with Gradle available on `PATH`:

```bash
TIA_DISPATCH_BUILD_TOOL=gradle scripts/run-java-dispatch-comparison-tia.sh
```

The runner uses Gradle's actual tests and artifact preparation, and normalizes
the `()` suffix Gradle writes on the parameterless control test names. It uses
fail-at-end/continue execution so the independent module runs even when the
consumer mutation fails. It preserves all modules' test reports.

Verified Maven run `sar-java-dispatch-comparison.oh9r3V` and Gradle run
`sar-java-dispatch-comparison.rDedIz` both select only `crossModule` for the
library mutation under both WALA and SootUp. The reason is
`ChangedCallable: SharedPrice.value()`, with no module fallback. The four
other tests, including `independentModule`, are excluded. Actual suite execution
confirms only `crossModule` fails. This completes the demonstrated cross-module
case; the separately recorded Spring bean-factory limitation remains.
