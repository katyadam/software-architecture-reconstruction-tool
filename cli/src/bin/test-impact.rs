use anyhow::{Context, Result};
use clap::Parser;
use cli::get_all_code_elements;
use extractor_runtime::{
    call_graph::{resolve_edges, wala::WalaJavaProvider},
    impact::{analyze_changes, discover_java_tests, select_tests},
};
use models::{ChangedElement, TestImpactResult};
use std::{
    collections::{BTreeSet, HashMap},
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
    /// Standalone WALA adapter JAR with Maven test-bytecode support.
    #[arg(long, value_name = "FILE")]
    wala_adapter_jar: PathBuf,
    /// Destination JSON file for the auditable selection result.
    #[arg(long, value_name = "FILE")]
    output: PathBuf,
    /// Maximum duration allowed for each WALA module analysis.
    #[arg(long, default_value_t = 300, value_name = "SECONDS")]
    wala_timeout_seconds: u64,
}

/// Runs conservative TIA and writes a JSON result even when analysis must fall back broadly.
fn run(arguments: Arguments) -> Result<()> {
    validate_project_and_revision(&arguments)?;
    let candidate_revision = git_revision(&arguments.project_dir, "HEAD")?;
    let change_analysis_started = Instant::now();
    let changes = analyze_changes(&arguments.project_dir, &arguments.baseline_revision)
        .context("Unable to determine candidate changes from Git")?;
    let mut diagnostics = vec![timing_diagnostic(
        "git diff",
        change_analysis_started.elapsed(),
    )];
    if changes_are_module_only(&changes) {
        return write_module_fallback_result(&arguments, candidate_revision, changes, diagnostics);
    }
    validate_wala_adapter(&arguments)?;
    let extraction_started = Instant::now();
    let aggregate = get_all_code_elements(&arguments.project_dir, &HashMap::new())
        .context("Unable to extract source callables for call-graph resolution")?;
    diagnostics.push(timing_diagnostic(
        "source extraction",
        extraction_started.elapsed(),
    ));
    let provider = WalaJavaProvider::with_timeout(
        &arguments.wala_adapter_jar,
        Duration::from_secs(arguments.wala_timeout_seconds),
    );
    let mut selected_tests = Vec::new();
    let mut unselected_tests = Vec::new();

    for module in changed_modules(&changes) {
        let module_changes = changes_for_module(&changes, &module);
        let module_result =
            analyze_module(&provider, &module, module_changes, &aggregate.callables);
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

/// Requires an adapter JAR only when callable-level analysis needs WALA.
fn validate_wala_adapter(arguments: &Arguments) -> Result<()> {
    if !arguments.wala_adapter_jar.is_file() {
        anyhow::bail!("--wala-adapter-jar must be an existing file");
    }
    Ok(())
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
    provider: &WalaJavaProvider,
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
    let wala_started = Instant::now();
    let mut outcome = provider.analyze_test_roots(&source_root, &tests);
    let resolution_started = Instant::now();
    let (resolved_edges, resolution_diagnostics) = resolve_edges(&outcome, module_path, callables);
    outcome.diagnostics.extend(resolution_diagnostics);
    let selection_started = Instant::now();
    let mut result = select_tests(changes, tests, outcome, resolved_edges);
    result.diagnostics.push(timing_diagnostic(
        "test discovery",
        wala_started.duration_since(discovery_started),
    ));
    result.diagnostics.push(timing_diagnostic(
        "WALA analysis",
        resolution_started.duration_since(wala_started),
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
