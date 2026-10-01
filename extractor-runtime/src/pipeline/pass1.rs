use models::{
    api::ExtractionError,
    ir::{language::Language, syntax::FileRecord},
};

/// Counterpart to [`dispatch`]: extracts a single file into a [`FileRecord`]
/// (Pass 1 only — no cross-file resolution).
pub fn dispatch_syntactic(
    text: &str,
    file_path: &str,
) -> Result<Option<FileRecord>, ExtractionError> {
    let path = std::path::Path::new(file_path);
    match Language::from_path(file_path) {
        Language::Java => java_extractor::extraction::extract_syntactic(text, file_path).map(Some),
        Language::Python => {
            python_extractor::extraction::parse::extract_syntactic(text, file_path).map(Some)
        }
        Language::Go => {
            if !go_extractor::extraction::should_extract_file(path) {
                return Ok(None);
            }
            go_extractor::extraction::extract_syntactic(text, file_path).map(Some)
        }
        // Config files are not a source language but still feed Java's config extraction.
        Language::Unknown => match path.extension().and_then(|e| e.to_str()) {
            Some("yml") | Some("yaml") | Some("properties") => Ok(
                java_extractor::extraction::config::extract_syntactic(text, file_path),
            ),
            _ => Ok(None),
        },
    }
}
