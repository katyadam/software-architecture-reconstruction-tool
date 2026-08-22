use models::ir::{language::Language, project::TypedFileRecord};

/// Identify REST calls and message edges, once types are resolved.
///
/// Identification is a Pass 2 stage for every language: Java's Spring strategy
/// needs `CallStatement::invoked_on`, which `resolve_call_argument_types`
/// populates. Each extractor owns its own rules behind a single `identify`
/// entry point, so this function holds no language-specific logic.
///
/// To add a language: implement `identify` in its extractor crate and add one
/// arm here.
pub fn identify_edges(files: &mut [TypedFileRecord]) {
    for file in files.iter_mut() {
        match file.language {
            Language::Java => java_extractor::extraction::identify(file),
            Language::Python => python_extractor::extraction::parse::identify(file),
            Language::Go => {
                if let Some(code) = go_extractor::extraction::source_from_assignments(file) {
                    let owned = code.to_string();
                    go_extractor::extraction::identify(file, &owned);
                } else if let Ok(code) = std::fs::read_to_string(&file.file_path) {
                    go_extractor::extraction::identify(file, &code);
                }
            }
        }
    }
}
