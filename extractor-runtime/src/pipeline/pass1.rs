use models::{api::ExtractionError, ir::syntax::FileRecord};

/// Counterpart to [`dispatch`]: extracts a single file into a [`FileRecord`]
/// (Pass 1 only — no cross-file resolution).
pub fn dispatch_syntactic(
    text: &str,
    file_path: &str,
) -> Result<Option<FileRecord>, ExtractionError> {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str());
    match ext {
        Some("java") => java_extractor::extraction::extract_syntactic(text, file_path).map(Some),
        Some("py") => {
            python_extractor::extraction::parse::extract_syntactic(text, file_path).map(Some)
        }
        _ => Ok(None),
    }
}
