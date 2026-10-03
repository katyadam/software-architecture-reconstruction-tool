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
    time::Duration,
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
    validate_arguments(&arguments)?;
    let candidate_revision = git_revision(&arguments.project_dir, "HEAD")?;
    let changes = analyze_changes(&arguments.project_dir, &arguments.baseline_revision)
        .context("Unable to determine candidate changes from Git")?;
    let aggregate = get_all_code_elements(&arguments.project_dir, &HashMap::new())
        .context("Unable to extract source callables for call-graph resolution")?;
    let provider = WalaJavaProvider::with_timeout(
        &arguments.wala_adapter_jar,
        Duration::from_secs(arguments.wala_timeout_seconds),
    );
    let mut selected_tests = Vec::new();
    let mut unselected_tests = Vec::new();
    let mut diagnostics = Vec::new();

    for module in changed_modules(&changes) {
        let module_changes: Vec<_> = changes
            .iter()
            .filter(|change| change.module_root == module)
            .cloned()
            .collect();
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
fn validate_arguments(arguments: &Arguments) -> Result<()> {
    if !arguments.project_dir.is_dir() || !arguments.project_dir.join(".git").exists() {
        anyhow::bail!("--project-dir must be a Git checkout");
    }
    if !arguments.wala_adapter_jar.is_file() {
        anyhow::bail!("--wala-adapter-jar must be an existing file");
    }
    git_revision(&arguments.project_dir, &arguments.baseline_revision)
        .context("--baseline-revision is not a resolvable Git revision")?;
    Ok(())
}

/// Runs discovery, WALA, resolution, and selection for one independently affected Maven module.
fn analyze_module(
    provider: &WalaJavaProvider,
    module: &str,
    changes: Vec<ChangedElement>,
    callables: &[models::Callable],
) -> TestImpactResult {
    let module_path = Path::new(module);
    let tests = match discover_java_tests(module_path) {
        Ok(tests) => tests,
        Err(error) => {
            let mut result = select_tests(
                changes,
                Vec::new(),
                failed_outcome(module, format!("test discovery failed: {error}")),
                Vec::new(),
            );
            result
                .diagnostics
                .push(format!("test discovery failed: {error}"));
            return result;
        }
    };
    let source_root = module_path.join("src/main/java");
    let mut outcome = provider.analyze_test_roots(&source_root, &tests);
    let (resolved_edges, resolution_diagnostics) = resolve_edges(&outcome, module_path, callables);
    outcome.diagnostics.extend(resolution_diagnostics);
    select_tests(changes, tests, outcome, resolved_edges)
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

/// Returns unique module paths in stable order so result serialization is deterministic.
fn changed_modules(changes: &[ChangedElement]) -> BTreeSet<String> {
    changes
        .iter()
        .map(|change| change.module_root.clone())
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
