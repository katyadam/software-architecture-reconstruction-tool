use extractor_runtime::pipeline;
use extractor_runtime::pipeline::pass3::{pass_attr, pass_module};
use models::{CodeElementsAggregate, ir::syntax::FileRecord};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn fixture_base() -> String {
    format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"))
}

pub fn collect_source_files(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir).expect("Failed to read fixture directory") {
            let path = entry.unwrap().path();
            if path.is_dir() {
                results.extend(collect_source_files(&path));
            } else if let Some(ext) = path.extension() {
                if ext == "java" || ext == "py" {
                    results.push(path);
                }
            }
        }
    }
    results
}

/// Synchronous helper: extracts `FileRecord`s from all source files in `dirs`
/// using `dispatch_syntactic` (Pass 1 only — no cross-file resolution).
pub fn collect_file_records(dirs: &[&Path]) -> Vec<FileRecord> {
    let mut records = Vec::new();
    for dir in dirs {
        for file_path in collect_source_files(dir) {
            let code = fs::read_to_string(&file_path)
                .unwrap_or_else(|_| panic!("Failed to read {:?}", file_path));
            let path_str = file_path.to_str().expect("Non-UTF8 path");
            if let Some(record) =
                pipeline::dispatch_syntactic(&code, path_str).unwrap_or_else(|e| {
                    panic!("Syntactic dispatch failed for {:?}: {:?}", file_path, e)
                })
            {
                records.push(record);
            }
        }
    }
    records
}

/// Runs the full 3-pass pipeline over all source files in `dirs` and returns
/// the result as a `CodeElementsAggregate` suitable for synthesizer calls.
pub fn extract_from_dirs(dirs: &[&Path]) -> CodeElementsAggregate {
    let records = collect_file_records(dirs);
    let project_ir = pipeline::build_project_ir(records);
    let external_constants = HashMap::new();
    let per_file_attrs = pass_attr::resolve_all(&project_ir);
    let per_file_module_consts =
        pass_module::resolve_all(&project_ir, &external_constants, &per_file_attrs);
    let evaluated_ir = pipeline::evaluate(
        project_ir,
        &external_constants,
        &per_file_attrs,
        &per_file_module_consts,
    );
    CodeElementsAggregate::from(evaluated_ir)
}
