use models::ir::{language::Language, project::TypedFileRecord};

use self::language_backend::strategy;

mod language_backend;

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
        strategy(&file.language).identify(file);
    }
    for language in [Language::Java, Language::Python, Language::Go] {
        strategy(&language).resolve_project_edges(files);
    }
}
