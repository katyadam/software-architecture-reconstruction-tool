use anyhow::{Context, Result};
use clap::Parser;
use cli::get_java_callables_for_modules_with_progress;
use extractor_runtime::{
    call_graph::{
        java_adapter::JavaAdapterProvider, resolve_reachability_with_java_tests,
        wala::WalaJavaProvider,
    },
    impact::{ModuleCatalog, analyze_changes, discover_java_tests, select_tests},
};
use models::{Callable, ChangedElement, ChangedElementKind, Namespace, TestImpactResult};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

/// Defines the standalone conservative Java test-impact command-line interface.
#[derive(Parser, Debug)]
#[command(about = "Select Java regression tests conservatively from a Git diff and WALA graph")]
struct Arguments {
    /// Candidate Git checkout whose HEAD is analyzed.
    #[arg(long, value_name = "DIR")]
    project_dir: PathBuf,
    /// Git revision used as the old side of the candidate comparison.
    #[arg(long, value_name = "REVISION")]
    baseline_revision: String,
    /// Java call-graph implementation used for callable-level analysis.
    #[arg(long, default_value = "wala", value_name = "PROVIDER")]
    java_callgraph_provider: String,
    /// Adapter JAR for a non-WALA Java call-graph provider such as SootUp.
    #[arg(long, value_name = "FILE")]
    java_callgraph_adapter_jar: Option<PathBuf>,
    /// Standalone WALA adapter JAR; preserved as the compatibility path for WALA.
    #[arg(long, value_name = "FILE")]
    wala_adapter_jar: Option<PathBuf>,
    /// Destination JSON file for the auditable selection result.
    #[arg(long, value_name = "FILE")]
    output: PathBuf,
    /// Maximum duration allowed for each WALA module analysis.
    #[arg(long, default_value_t = 300, value_name = "SECONDS")]
    wala_timeout_seconds: u64,
    /// Suppress source-extraction progress output while retaining diagnostics and JSON output.
    #[arg(long)]
    quiet: bool,
}

/// Runs conservative TIA and writes a JSON result even when analysis must fall back broadly.
fn run(arguments: Arguments) -> Result<()> {
    validate_project_and_revision(&arguments)?;
    let provider_kind = JavaCallGraphProviderKind::parse(&arguments.java_callgraph_provider)?;
    let candidate_revision = git_revision(&arguments.project_dir, "HEAD")?;
    let module_metadata_diagnostic = ModuleCatalog::load(&arguments.project_dir)
        .diagnostic()
        .map(str::to_owned);
    let change_analysis_started = Instant::now();
    let changes = analyze_changes(&arguments.project_dir, &arguments.baseline_revision)
        .context("Unable to determine candidate changes from Git")?;
    let mut diagnostics = vec![timing_diagnostic(
        "git diff",
        change_analysis_started.elapsed(),
    )];
    if let Some(diagnostic) = module_metadata_diagnostic {
        diagnostics.push(format!("module metadata fallback: {diagnostic}"));
    }
    if changes_are_module_only(&changes) {
        return write_module_fallback_result(&arguments, candidate_revision, changes, diagnostics);
    }
    let provider = select_provider(&arguments, provider_kind)?;
    let modules = changed_modules(&changes);
    let extraction_started = Instant::now();
    let callables = get_java_callables_for_modules_with_progress(
        &modules.iter().map(PathBuf::from).collect::<Vec<_>>(),
        !arguments.quiet,
    )
    .context("Unable to extract source callables for call-graph resolution")?;
    diagnostics.push(timing_diagnostic(
        "source extraction",
        extraction_started.elapsed(),
    ));
    let mut selected_tests = Vec::new();
    let mut unselected_tests = Vec::new();

    for module in modules {
        let module_changes = changes_for_module(&changes, &module);
        let module_result = analyze_module(&provider, &module, module_changes, &callables);
        selected_tests.extend(module_result.selected_tests);
        unselected_tests.extend(module_result.unselected_tests);
        diagnostics.extend(module_result.diagnostics);
    }

    let mut result = TestImpactResult::new(
        arguments.baseline_revision,
        candidate_revision,
        changes,
        selected_tests,
        unselected_tests,
    );
    result.diagnostics.append(&mut diagnostics);
    write_result(&arguments.output, &result)
}

/// Validates paths and Git revisions before any potentially costly source or Maven analysis begins.
fn validate_project_and_revision(arguments: &Arguments) -> Result<()> {
    if !arguments.project_dir.is_dir() || !arguments.project_dir.join(".git").exists() {
        anyhow::bail!("--project-dir must be a Git checkout");
    }
    git_revision(&arguments.project_dir, &arguments.baseline_revision)
        .context("--baseline-revision is not a resolvable Git revision")?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JavaCallGraphProviderKind {
    Wala,
    SootUp,
}

impl JavaCallGraphProviderKind {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "wala" => Ok(Self::Wala),
            "sootup" => Ok(Self::SootUp),
            other => anyhow::bail!("Unsupported Java call-graph provider: {other}"),
        }
    }
}

enum SelectedJavaProvider {
    Wala(WalaJavaProvider),
    SootUp(JavaAdapterProvider),
}

impl SelectedJavaProvider {
    fn label(&self) -> &'static str {
        match self {
            Self::Wala(_) => "WALA",
            Self::SootUp(_) => "SootUp",
        }
    }

    fn analyze_test_roots(
        &self,
        source_root: &Path,
        test_roots: &[models::JavaTestCase],
        targets: &[String],
    ) -> models::call_graph::CallGraphOutcome {
        match self {
            Self::Wala(provider) => provider.analyze_test_roots(source_root, test_roots, targets),
            Self::SootUp(provider) => provider.analyze_test_roots(source_root, test_roots, targets),
        }
    }
}

fn select_provider(
    arguments: &Arguments,
    kind: JavaCallGraphProviderKind,
) -> Result<SelectedJavaProvider> {
    let timeout = Duration::from_secs(arguments.wala_timeout_seconds);
    match kind {
        JavaCallGraphProviderKind::Wala => {
            if arguments.java_callgraph_adapter_jar.is_some() {
                anyhow::bail!(
                    "--java-callgraph-adapter-jar is only valid with --java-callgraph-provider sootup"
                );
            }
            let jar = arguments
                .wala_adapter_jar
                .as_ref()
                .context("--wala-adapter-jar must be supplied for the WALA provider")?;
            if !jar.is_file() {
                anyhow::bail!("--wala-adapter-jar must be an existing file");
            }
            Ok(SelectedJavaProvider::Wala(WalaJavaProvider::with_timeout(
                jar, timeout,
            )))
        }
        JavaCallGraphProviderKind::SootUp => {
            if arguments.wala_adapter_jar.is_some() {
                anyhow::bail!(
                    "--wala-adapter-jar cannot be combined with --java-callgraph-provider sootup"
                );
            }
            let jar = arguments
                .java_callgraph_adapter_jar
                .as_ref()
                .context("--java-callgraph-adapter-jar must be supplied for the SootUp provider")?;
            if !jar.is_file() {
                anyhow::bail!("--java-callgraph-adapter-jar must be an existing file");
            }
            Ok(SelectedJavaProvider::SootUp(JavaAdapterProvider::new(
                "sootup-java",
                jar,
                timeout,
                "cha",
            )))
        }
    }
}

/// Returns whether every change is already required to use module-wide fallback.
fn changes_are_module_only(changes: &[ChangedElement]) -> bool {
    changes
        .iter()
        .all(|change| matches!(change.kind, models::ChangedElementKind::Module))
}

/// Selects module tests without source extraction or WALA when the diff cannot be narrowed.
fn write_module_fallback_result(
    arguments: &Arguments,
    candidate_revision: String,
    changes: Vec<ChangedElement>,
    mut diagnostics: Vec<String>,
) -> Result<()> {
    let mut selected_tests = Vec::new();
    let mut unselected_tests = Vec::new();
    diagnostics.push("skipped call-graph analysis for module-only changes".into());
    for module in changed_modules(&changes) {
        let module_result = select_module_fallback(&module, changes_for_module(&changes, &module));
        selected_tests.extend(module_result.selected_tests);
        unselected_tests.extend(module_result.unselected_tests);
        diagnostics.extend(module_result.diagnostics);
    }
    let mut result = TestImpactResult::new(
        arguments.baseline_revision.clone(),
        candidate_revision,
        changes,
        selected_tests,
        unselected_tests,
    );
    result.diagnostics.append(&mut diagnostics);
    write_result(&arguments.output, &result)
}

/// Runs discovery, WALA, resolution, and selection for one independently affected Maven module.
fn analyze_module(
    provider: &SelectedJavaProvider,
    module: &str,
    changes: Vec<ChangedElement>,
    callables: &[models::Callable],
) -> TestImpactResult {
    let module_path = Path::new(module);
    let discovery_started = Instant::now();
    let tests = match discover_java_tests(module_path) {
        Ok(tests) => tests,
        Err(error) => {
            let discovery_elapsed = discovery_started.elapsed();
            let selection_started = Instant::now();
            let mut result = select_tests(
                changes,
                Vec::new(),
                failed_outcome(module, format!("test discovery failed: {error}")),
                Vec::new(),
            );
            result
                .diagnostics
                .push(format!("test discovery failed: {error}"));
            result
                .diagnostics
                .push(timing_diagnostic("test discovery", discovery_elapsed));
            result.diagnostics.push(timing_diagnostic(
                "test selection",
                selection_started.elapsed(),
            ));
            return result;
        }
    };
    let source_root = module_path.join("src/main/java");
    let callgraph_started = Instant::now();
    let targets = callgraph_targets(&source_root, &changes, callables);
    let mut outcome = provider.analyze_test_roots(&source_root, &tests, &targets);
    let resolution_started = Instant::now();
    let resolution = resolve_reachability_with_java_tests(&outcome, module_path, callables, &tests);
    outcome.diagnostics.extend(resolution.diagnostics);
    let selection_started = Instant::now();
    let mut result = extractor_runtime::impact::select_tests_with_reachability(
        changes,
        tests,
        outcome,
        resolution.edges,
        &resolution.reachable_tests_by_callable,
    );
    result.diagnostics.push(timing_diagnostic(
        "test discovery",
        callgraph_started.duration_since(discovery_started),
    ));
    result.diagnostics.push(timing_diagnostic(
        &format!("{} analysis", provider.label()),
        resolution_started.duration_since(callgraph_started),
    ));
    result.diagnostics.push(timing_diagnostic(
        "edge resolution",
        selection_started.duration_since(resolution_started),
    ));
    result.diagnostics.push(timing_diagnostic(
        "test selection",
        selection_started.elapsed(),
    ));
    result
}

/// Converts changed source owners into conservative WALA method targets for bounded provenance.
fn callgraph_targets(
    source_root: &Path,
    changes: &[ChangedElement],
    callables: &[Callable],
) -> Vec<String> {
    let mut targets = BTreeSet::new();
    for change in changes {
        let method = change
            .callable_signature
            .as_deref()
            .and_then(|signature| signature.split_once('.'));
        for callable in callables {
            if !callable.file_path.ends_with(&change.source_path) {
                continue;
            }
            let Namespace::Class(class) = &callable.namespace else {
                continue;
            };
            let matches_change = match change.kind {
                ChangedElementKind::Callable => method.is_some_and(|(owner, name)| {
                    owner == class
                        && callable_method_name(&callable.name) == Some(name.trim_end_matches("()"))
                }),
                ChangedElementKind::Class => change.callable_signature.as_deref() == Some(class),
                ChangedElementKind::Module => false,
            };
            if !matches_change {
                continue;
            }
            let Ok(relative) = Path::new(&callable.file_path).strip_prefix(source_root) else {
                continue;
            };
            let declaring_type = format!(
                "L{}",
                relative
                    .with_extension("")
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/")
            );
            if let Some(member) = wala_target_member(callable) {
                targets.insert(format!(
                    "{declaring_type}#{member}/{}",
                    callable.parameters.len()
                ));
            }
        }
    }
    targets.into_iter().collect()
}

fn callable_method_name(name: &str) -> Option<&str> {
    name.split_once('(')
        .and_then(|(prefix, _)| prefix.split_whitespace().last())
}

/// Uses the JVM spelling for constructors while preserving ordinary source method names.
fn wala_target_member(callable: &Callable) -> Option<&str> {
    callable
        .is_constructor
        .then_some("<init>")
        .or_else(|| callable_method_name(&callable.name))
}

#[cfg(test)]
mod tests {
    use super::wala_target_member;
    use models::{Callable, Namespace};

    #[test]
    fn converts_java_constructor_targets_to_jvm_init() {
        let constructor = Callable {
            name: "public Service(String dependency)".into(),
            signature: "class:Service/Service(String)".into(),
            namespace: Namespace::Class("Service".into()),
            parameters: vec![],
            return_type: None,
            is_async: false,
            is_constructor: true,
            hash: String::new(),
            file_path: "/service/src/main/java/example/Service.java".into(),
        };

        assert_eq!(wala_target_member(&constructor), Some("<init>"));
    }
}

/// Discovers tests and selects the required broad fallback without invoking the adapter.
fn select_module_fallback(module: &str, changes: Vec<ChangedElement>) -> TestImpactResult {
    let discovery_started = Instant::now();
    let tests = match discover_java_tests(Path::new(module)) {
        Ok(tests) => tests,
        Err(error) => {
            let discovery_elapsed = discovery_started.elapsed();
            let selection_started = Instant::now();
            let mut result = select_tests(
                changes,
                Vec::new(),
                failed_outcome(module, format!("test discovery failed: {error}")),
                Vec::new(),
            );
            result
                .diagnostics
                .push(format!("test discovery failed: {error}"));
            result.diagnostics.push(timing_diagnostic(
                "module fallback test discovery",
                discovery_elapsed,
            ));
            result.diagnostics.push(timing_diagnostic(
                "module fallback selection",
                selection_started.elapsed(),
            ));
            return result;
        }
    };
    let selection_started = Instant::now();
    let mut result = select_tests(
        changes,
        tests,
        models::call_graph::CallGraphOutcome::ok(
            models::call_graph::CallGraphRequest::new(module, models::call_graph::Language::Java),
            "not-run",
            "module-fallback",
        ),
        Vec::new(),
    );
    result.diagnostics.push(timing_diagnostic(
        "module fallback test discovery",
        selection_started.duration_since(discovery_started),
    ));
    result.diagnostics.push(timing_diagnostic(
        "module fallback selection",
        selection_started.elapsed(),
    ));
    result
}

/// Formats elapsed work as a machine-readable, low-overhead diagnostic.
fn timing_diagnostic(stage: &str, elapsed: Duration) -> String {
    format!("timing: {stage}={}ms", elapsed.as_millis())
}

/// Produces a provider-shaped failure that lets the selector retain its module fallback policy.
fn failed_outcome(module: &str, diagnostic: String) -> models::call_graph::CallGraphOutcome {
    models::call_graph::CallGraphOutcome {
        schema_version: 1,
        status: models::call_graph::CallGraphStatus::Failed,
        provider_id: "wala-java".into(),
        source_root: module.into(),
        algorithm: "cha_bytecode".into(),
        diagnostics: vec![diagnostic],
        reachability: vec![],
        edges: Vec::new(),
    }
}

/// Returns outermost affected module paths so descendant tests are analyzed exactly once.
fn changed_modules(changes: &[ChangedElement]) -> BTreeSet<String> {
    let modules: BTreeSet<_> = changes
        .iter()
        .map(|change| change.module_root.clone())
        .collect();
    modules
        .iter()
        .filter(|module| {
            !modules
                .iter()
                .any(|other| other != *module && Path::new(module).starts_with(Path::new(other)))
        })
        .cloned()
        .collect()
}

/// Returns the changes owned by a module root, including descendant modules in a reactor.
fn changes_for_module(changes: &[ChangedElement], module: &str) -> Vec<ChangedElement> {
    changes
        .iter()
        .filter(|change| Path::new(&change.module_root).starts_with(Path::new(module)))
        .cloned()
        .collect()
}

/// Resolves a Git revision in the candidate checkout and returns its immutable commit identity.
fn git_revision(project_dir: &Path, revision: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", revision])
        .current_dir(project_dir)
        .output()
        .context("Unable to start Git")?;
    if !output.status.success() {
        anyhow::bail!(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Serializes the complete result atomically enough for ordinary local CLI usage.
fn write_result(output: &Path, result: &TestImpactResult) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).context("Unable to create output directory")?;
    }
    fs::write(
        output,
        serde_json::to_string_pretty(result).context("Unable to serialize TIA result")?,
    )
    .context("Unable to write TIA result")
}

/// Parses command-line input and reports invalid input without suppressing valid analysis output.
fn main() -> Result<()> {
    run(Arguments::parse())
}
