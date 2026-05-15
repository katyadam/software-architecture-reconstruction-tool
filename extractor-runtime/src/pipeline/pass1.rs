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
    let language = decide_language(file_path);
    match language {
        Language::Java => java_extractor::extraction::extract_syntactic(text, file_path).map(Some),
        Language::Python => {
            python_extractor::extraction::parse::extract_syntactic(text, file_path).map(Some)
        }
        Language::Unknown => Ok(None),
    }
}

pub fn decide_language(file_path: &str) -> Language {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str());

    match ext {
        Some("java") => Language::Java,
        Some("py") => Language::Python,
        _ => Language::Unknown,
    }
}
