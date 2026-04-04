use extractor_runtime::dispatch;
use models::{CodeElementsAggregate, ir::syntax::FileRecord};
use std::fs;
use std::path::{Path, PathBuf};

pub fn fixture_base() -> String {
    format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"))
}

pub fn merge_aggregates(main: &mut CodeElementsAggregate, new: CodeElementsAggregate) {
    main.imports.extend(new.imports);
    main.entities.extend(new.entities);
    main.endpoints.extend(new.endpoints);
    main.restcalls.extend(new.restcalls);
    main.callables.extend(new.callables);
    main.call_statements.extend(new.call_statements);
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
            if let Some(record) = dispatch::dispatch_syntactic(&code, path_str)
                .unwrap_or_else(|e| panic!("Syntactic dispatch failed for {:?}: {:?}", file_path, e))
            {
                records.push(record);
            }
        }
    }
    records
}

pub async fn extract_from_dirs(dirs: &[&Path]) -> CodeElementsAggregate {
    let mut aggregate = CodeElementsAggregate::default();
    for dir in dirs {
        for file_path in collect_source_files(dir) {
            let code = fs::read_to_string(&file_path)
                .unwrap_or_else(|_| panic!("Failed to read {:?}", file_path));
            let path_str = file_path.to_str().expect("Non-UTF8 path");
            if let Some(elements) = dispatch::dispatch(&code, path_str)
                .await
                .unwrap_or_else(|e| panic!("Dispatch failed for {:?}: {:?}", file_path, e))
            {
                merge_aggregates(&mut aggregate, elements);
            }
        }
    }
    aggregate
}
