use extractor_runtime::dispatch;
use models::CodeElementsAggregate;
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
