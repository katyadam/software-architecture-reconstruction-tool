# Gradle Java Test Impact Analysis Design

## Goal

Extend the Java TIA baseline so a conventional Gradle Java project receives the
same test-root WALA call graph and conservative selection behavior as a Maven
project. The first version supports Groovy (`build.gradle`) and Kotlin
(`build.gradle.kts`) DSLs, including Gradle multi-module builds.

## Scope and success criteria

The implementation must:

- detect a Gradle module owning a conventional `src/main/java` source root;
- prefer the project `gradlew`/`gradlew.bat` wrapper and only use a system
  `gradle` executable when no wrapper exists;
- run the selected module's `testClasses` task, allowing Gradle to build its
  declared upstream project dependencies;
- collect main/test class directories and the module test runtime classpath;
- pass those values and discovered JUnit roots to the existing bytecode WALA
  graph path;
- support both Gradle DSLs without parsing either build file;
- select every discovered test in the affected module when preparation,
  classpath discovery, test-root binding, or graph resolution is uncertain.

It does not model Spring DI/AOP, reflection, framework callbacks, dynamic
tests, or inter-service messaging. It does not attempt Android or non-Java
Gradle plugins in this milestone.

## Architecture

Introduce a build-tool-neutral bytecode-artifact contract in the WALA adapter:

```text
BytecodeArtifactPreparer
  prepare(sourceRoot) -> Preparation
Preparation
  PreparedArtifacts(main class dirs, test class dirs, dependency JARs, module root)
```

The existing Maven preparation moves behind the contract without altering its
behavior. `GradleBytecodeArtifacts` becomes a second implementation. The
adapter chooses a preparer by conventional project ownership: Maven first for
`pom.xml`, Gradle for an ancestor `settings.gradle[.kts]`, `build.gradle[.kts]`,
or wrapper. Explicit `--classes-dir`/`--test-classes-dir` remains the highest
priority and bypasses preparation entirely.

The Rust provider and TIA CLI do not gain Gradle-specific code. They already
call the adapter with Java source roots and explicit JUnit method selectors;
the adapter's build-tool preparation decision remains internal and
provider-neutral.

## Gradle preparation

Given `module/src/main/java`, locate the nearest Gradle module directory and
its enclosing Gradle root. Derive the project path from its relative directory:
the root module is `:`, and `services/orders` is `:services:orders`.

Run the following with the wrapper when present:

```text
<gradle-command> :module:testClasses
<gradle-command> -q -I <temporary-init-script> :module:voyantclairWalaArtifacts
```

The temporary Groovy init script installs a task in all projects. The task
reads Gradle's Java `SourceSetContainer` and `testRuntimeClasspath`, emitting
three machine-readable, path-separated records:

```text
VOYANTCLAIR_MAIN=<classesDirs.asPath>
VOYANTCLAIR_TEST=<testClassesDirs.asPath>
VOYANTCLAIR_TEST_RUNTIME=<testRuntimeClasspath.asPath>
```

An init script is valid for both Groovy and Kotlin DSL projects because Gradle
executes it independently of the project's build-script language. The adapter
does not parse or modify the project's build files. Its temporary script is
removed even after a Gradle failure.

Only existing directories and JAR files are retained. Class directories from
the selected project are application inputs; classpath JARs and dependent
project outputs are dependency inputs. Gradle's normal project dependency
resolution provides required upstream compilation.

## Failure behavior

`GradleBytecodeArtifacts` returns structured preparation failure text rather
than throwing. When TIA supplied explicit test roots, WALA returns a failed
provider result with `test_root_preparation_failed=...`; the existing selector
then applies module-wide fallback. Non-TIA call-graph requests may retain the
source fallback used by the Maven path.

An absent Java plugin/source set, a failed `testClasses`, absent wrapper/system
Gradle, malformed artifact records, missing class output, or Gradle timeout all
count as preparation uncertainty. No uncertainty may yield an empty narrowed
test selection.

## Testing

Add focused Java adapter fixtures for:

1. a single-module Groovy DSL project where `ExampleTest.testChanged` reaches
   `Service.changed`;
2. the same fixture expressed with Kotlin DSL;
3. a multi-module project with a test in one module and code in its dependent
   module;
4. an invalid Gradle project that returns preparation failure.

The fixtures create a wrapper-like command only where necessary for unit-level
process control; integration verification uses a locally available Gradle or
wrapper and is skipped only when neither is installed. The full TIA CLI test
adds a Gradle version pair once the adapter fixtures prove artifact discovery.

## Documentation and limits

Update the provider and benchmark documentation with Gradle invocation,
wrapper behavior, required Java plugin layouts, and conservative fallback
rules. Record whether each Gradle fixture obtained the expected direct edge and
the selected/total test ratio. Do not claim support for framework-injected or
distributed communication paths.
