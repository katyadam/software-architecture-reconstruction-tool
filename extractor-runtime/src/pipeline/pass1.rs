use models::{api::ExtractionError, ir::syntax::FileRecord};

/// Counterpart to [`dispatch`]: extracts a single file into a [`FileRecord`]
/// (Pass 1 only — no cross-file resolution).
pub fn dispatch_syntactic(
    text: &str,
    file_path: &str,
) -> Result<Option<FileRecord>, ExtractionError> {
    let path = std::path::Path::new(file_path);
    let ext = path.extension().and_then(|e| e.to_str());
    match ext {
        Some("java") => java_extractor::extraction::extract_syntactic(text, file_path).map(Some),
        Some("py") => {
            python_extractor::extraction::parse::extract_syntactic(text, file_path).map(Some)
        }
        Some("go") => {
            if should_skip_generated_go_file(path) {
                return Ok(None);
            }
            go_extractor::extraction::extract_syntactic(text, file_path).map(Some)
        }
        _ => Ok(None),
    }
}

fn should_skip_generated_go_file(path: &std::path::Path) -> bool {
    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    if file_name.ends_with(".pb.go") || file_name.ends_with("_grpc.pb.go") {
        return true;
    }

    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|value| value == "thriftgo")
    })
}
