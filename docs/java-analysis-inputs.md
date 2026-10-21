# Java analysis inputs in service configurations

The existing `service_descriptions` format supports optional Java build inputs.
Existing configuration files and Java TIA commands without `--config-file` retain
their automatic discovery/preparation behavior. WALA remains the default provider.

```json
{
  "service_descriptions": [{
    "name": "orders",
    "base_dir_path": "train-ticket/ts-order-service",
    "urls": ["http://ts-order-service"],
    "java_analysis": {
      "maven_profiles": ["ci"],
      "artifacts_file": "target/tia-artifacts.json"
    }
  }]
}
```

`ci` is illustrative; no profile is assumed to exist in Train Ticket. An empty
`java_analysis` keeps automatic inputs. A nonempty `maven_profiles` list requires
a supplied manifest in this first version; it does not pass arbitrary Maven
command text or silently prepare a default-profile build.

## Paths and invocation

`test-impact --project-dir` identifies the candidate Git root. Configured service
paths resolve relative to `--config-base-dir`, which defaults to that root.
The config file's own directory does not change service path resolution.

For the existing Train Ticket path convention:

```sh
target/debug/test-impact \
  --project-dir /benchmarks/java/train-ticket \
  --config-file config/configurations/train-ticket-config.json \
  --config-base-dir /benchmarks/java \
  --baseline-revision BASELINE_SHA \
  --wala-adapter-jar wala-callgraph/target/wala-callgraph-all.jar \
  --output /tmp/train-ticket-impact.json
```

That unextended config reuses service ownership and automatic build preparation.
To bypass preparation, add `java_analysis.artifacts_file` for the relevant Java
services and supply their manifests. For SootUp, replace the WALA JAR option with
`--java-callgraph-provider sootup --java-callgraph-adapter-jar sootup-callgraph/target/sootup-callgraph-all.jar`.

Relative `artifacts_file` paths resolve within the service directory; they cannot
escape it through `..` or symlinks. An explicit absolute manifest path supports
CI-produced artifacts outside the checkout. Module roots resolve relative to the
service, and can use `../shared` inside the candidate checkout. Other module paths
resolve relative to the module. Dependencies refer to module roots relative to
the service. External dependency JARs **and class directories** are permitted on
the classpath. Source and output directories remain within the candidate checkout.
Use literal paths: no shell expansion, wildcard expansion or command strings.

Missing directories, overlapping service roots, wrong revisions/profiles and
conflicting shared-module definitions are errors. Services outside this one Git
checkout are reported outside scope; Java settings requesting analysis for such
a service are an error. One baseline revision does not describe multiple repos.

## Manifest responsibilities

See [the complete example](../config/examples/java-analysis/tia-artifacts.json).
Schema version 1 records:

- Full candidate commit SHA and build context: tool (`maven` or `gradle`), requested
  profiles, effective active profiles and Java version.
- In-checkout build modules, direct module dependencies and inventory completeness.
- Actual production/test source directories, including generated roots where used.
- Compiled production/test directories and the resolved dependency classpath.
- Effective supported test methods as `JavaTestCase` identities, after the build's
  test filters/profile selection. Canonical class names use `Lpackage/Class$Nested`;
  dotted qualified names are normalized to that representation. Source paths and
  module roots can be relative under the preceding rules.

The manifest producer must evaluate the actual build and supply fresh candidate
artifacts. The analyzer validates the schema, paths, identities and declared
revision/profile context, but a SHA string cannot prove how a class file was
built. The manifest digest records the metadata consumed, not a cryptographic
proof of source-to-bytecode correspondence.

Requested profiles must match the config as a set and be active in the supplied
Maven build. Additional automatically activated profiles are recorded separately.
Run separate analysis/execution contexts for the combinations actually used in CI.
Spring runtime profiles are not Maven profiles and are not modeled by this setting.

This release **consumes** supplied manifests; it does not export effective Maven
or Gradle models or implement all test selectors. A build integration/CI producer
must populate the effective test inventory. Mark unsupported selectors, dynamic
test forms or uncertain module/test scope incomplete. Completeness flags are
required assertions about that producer's scope, not call-graph soundness claims.

Services are not the complete build-module universe: include shared libraries and
their dependency edges, and retain automatic metadata for unconfigured modules.
Unknown consumers or incomplete dependency metadata prevent narrow exclusions.
Configuration never silently drops changed files outside listed services.

## Reports and execution

Manifest-backed results add `analysis_contexts` with service/module/source roots,
manifest path/digest, candidate SHA, profiles and Java version. Empty optional
fields stay absent for legacy reports. Diagnostics explicitly identify skipped
artifact preparation; both providers receive the same prepared inputs.

If `requires_full_test_run` is true, **run the whole checkout's test suite in the
recorded profile context**. The available method list is incomplete execution
guidance, not a sufficient command list. `unselected_tests` is empty in this case.
An incomplete empty inventory is therefore different from a valid empty suite.
If a supplied empty inventory contradicts tests found in its declared source roots,
known tests are retained and full execution is required. This also applies when
profile filtering could explain the empty inventory: the current format cannot
substantiate that explanation. Checkout scanning checks builds outside the declared
reactor, and discovered test owners require dependency evidence before exclusion.

With complete supplied scope, selection keeps the existing conservative graph
fallback behavior. This feature does not repair reflection, Spring injection,
proxies, dynamic callbacks or distributed-service impact. Missing graph paths do
not acquire a new completeness guarantee just because a manifest exists.

The reconstruction CLI honors manifests during optional WALA enrichment using its
existing `--project-dir` path convention. Plain reconstruction without a graph
adapter does not require Java artifacts. Generated Java callable metadata can be
collected from declared roots even when normal traversal skips `target` outputs.

## Small reproducible example

The example contains custom source/test roots, a Maven POM and a local annotation
stub so compilation does not need a JUnit download. It demonstrates the two
explicit test methods, not a complete JUnit execution engine. The real-provider
acceptance tests additionally execute mutation failures independently.

Copy it into an isolated Git checkout; do not run these initialization commands
inside an existing project:

```sh
demo_dir=$(mktemp -d /tmp/java-input-example.XXXXXX)
cp -R config/examples/java-analysis/. "$demo_dir/"
git -C "$demo_dir" init -q
git -C "$demo_dir" -c user.name=Fixture -c user.email=fixture@example.invalid add .
git -C "$demo_dir" -c user.name=Fixture -c user.email=fixture@example.invalid commit -qm baseline
baseline=$(git -C "$demo_dir" rev-parse HEAD)
perl -pi -e 's/return 1;/return 2;/' "$demo_dir/source/java/fixture/Service.java"
git -C "$demo_dir" add source/java/fixture/Service.java
git -C "$demo_dir" -c user.name=Fixture -c user.email=fixture@example.invalid commit -qm candidate
mvn -q -f "$demo_dir/pom.xml" test-compile -DskipTests
python3 - "$demo_dir/tia-artifacts.json" "$(git -C "$demo_dir" rev-parse HEAD)" <<'PY'
import json, pathlib, sys
path = pathlib.Path(sys.argv[1])
data = json.loads(path.read_text())
data["candidate_revision"] = sys.argv[2]
path.write_text(json.dumps(data, indent=2) + "\n")
PY
target/debug/test-impact \
  --project-dir "$demo_dir" --baseline-revision "$baseline" \
  --config-file "$demo_dir/service-config.json" \
  --wala-adapter-jar "$PWD/wala-callgraph/target/wala-callgraph-all.jar" \
  --output /tmp/java-input-example-result.json --quiet
```

Use the actual JDK version in `build_context.java_version` if it differs from the
example. The zero SHA in the checked-in example is deliberately invalid for a
real candidate until replaced. Do not commit generated manifests into application
history merely to provide them to analysis.

Verification targets:

```sh
cargo test -p models --test java_analysis_configuration
cargo test -p extractor-runtime --test java_analysis_inputs --test configured_java_scope --test java_prepared_inputs
cargo test -p cli --test configured_java_impact --test configured_java_reconstruction --test java_manifest_providers
```

Real-provider tests require built WALA/SootUp adapter JARs, Java 17+ and `javac`.
They precompile candidate classes, execute the baseline/mutation controls and
analyze the same prepared artifacts under both providers without build descriptors.
