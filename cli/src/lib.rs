use anyhow::{Context, Result};
use extractor_runtime::pipeline::pass3::{evaluate, evaluate_with_llm, pass_attr, pass_module};
use extractor_runtime::pipeline::{build_project_ir, dispatch_syntactic};
use models::{CodeElementsAggregate, ConfigurationData};
use sage::resolver::client::SageClient;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub mod metadata;

pub async fn get_all_code_elements(
    project_dir: &PathBuf,
    external_constants: &HashMap<String, String>,
    config: &ConfigurationData,
    sage: Option<&SageClient>,
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

    let evaluated_ir = if let Some(sage) = sage {
        println!("Evaluating with LLM...\nClient: {sage:?}");
        evaluate_with_llm(
            project_ir,
            external_constants,
            &per_file_attrs,
            &per_file_module_consts,
            config,
            sage,
        )
        .await
    } else {
        evaluate(
            project_ir,
            external_constants,
            &per_file_attrs,
            &per_file_module_consts,
        )
    };

    Ok(CodeElementsAggregate::from(evaluated_ir))
}

/// Serialize `data` to pretty JSON and write it to `<dir>/<filename>`.
pub fn save_json<T: serde::Serialize>(dir: &Path, filename: &str, data: &T) -> Result<()> {
    let path = dir.join(filename);

    let json_data = serde_json::to_string_pretty(data)
        .with_context(|| format!("Failed to serialize {}", filename))?;

    fs::write(&path, json_data).with_context(|| format!("Failed to write file: {:?}", path))?;

    println!("   📄 Generated: {}", filename);
    Ok(())
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
