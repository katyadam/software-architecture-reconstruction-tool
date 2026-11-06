use anyhow::{Context, Result};
use extractor_runtime::call_graph::{CallGraphProvider, resolve_edges, wala::WalaJavaProvider};
use extractor_runtime::pipeline::pass3::{pass_attr, pass_module};
use extractor_runtime::pipeline::{build_project_ir, dispatch_syntactic, evaluate};
use models::{CodeElementsAggregate, ConfigurationData};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

pub fn get_all_code_elements(
    project_dir: &PathBuf,
    external_constants: &HashMap<String, String>,
) -> Result<CodeElementsAggregate> {
    let paths = collect_files(project_dir)?;
    let files_to_process = paths.len();
    println!("Total files found: {files_to_process}");

    let mut file_records = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        let code = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!(
                    "⚠️  Skipping file {:?}: {}",
                    path.file_name().unwrap_or_default(),
                    e
                );
                continue;
            }
        };
        println!(
            "Extracting ({}/{files_to_process}): {:?}",
            i + 1,
            path.file_name().unwrap_or_default()
        );
        if let Some(record) = dispatch_syntactic(&code, path.to_str().unwrap_or_default())
            .with_context(|| format!("Error dispatching file: {:?}", path))?
        {
            file_records.push(record);
        }
    }

    let project_ir = build_project_ir(file_records);

    let per_file_attrs = pass_attr::resolve_all(&project_ir, external_constants);
    let per_file_module_consts =
        pass_module::resolve_all(&project_ir, external_constants, &per_file_attrs);
    let evaluated_ir = evaluate(
        project_ir,
        external_constants,
        &per_file_attrs,
        &per_file_module_consts,
    );
    Ok(CodeElementsAggregate::from(evaluated_ir))
}

/// Adds uniquely resolved WALA Java edges for configured service directories when enabled.
///
/// A missing adapter JAR, a deadline breach, or a non-Java service leaves syntactic extraction unchanged.
pub fn enrich_with_wala(
    mut aggregate: CodeElementsAggregate,
    project_dir: &PathBuf,
    configuration: &ConfigurationData,
    jar: Option<&std::path::Path>,
    wala_timeout: Duration,
) -> CodeElementsAggregate {
    let Some(jar) = jar else {
        return aggregate;
    };
    let provider = WalaJavaProvider::with_timeout(jar, wala_timeout);
    for service in &configuration.service_descriptions {
        let root = project_dir.join(&service.base_dir_path);
        if !root.is_dir() {
            continue;
        }
        let outcome = provider.analyze(&models::call_graph::CallGraphRequest::new(
            root.to_string_lossy(),
            models::call_graph::Language::Java,
        ));
        let (edges, diagnostics) = resolve_edges(&outcome, &root, &aggregate.callables);
        for diagnostic in diagnostics {
            eprintln!("WALA: {diagnostic}");
        }
        aggregate.resolved_call_edges.extend(edges);
    }
    aggregate
}

pub fn collect_files(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut results = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                results.extend(collect_files(&path)?);
            } else {
                results.push(path);
            }
        }
    }
    Ok(results)
}
