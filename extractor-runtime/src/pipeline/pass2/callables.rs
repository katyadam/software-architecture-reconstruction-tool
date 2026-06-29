use std::collections::HashMap;

use models::{
    ParsedCallable,
    ir::{language::Language, project::TypedFileRecord},
};

/// Index every callable by `(file_path, content-hash)`.
///
/// Companion to [`build_project_global_callables`]: that map is keyed by mangled
/// name (the lookup when you know the symbol); this one answers "which callable
/// encloses this call site?" from a `(file_path, function_hash)` pair. The hash
/// is a *content* hash (see the extractors), so it is not unique across files --
/// pairing it with `file_path` scopes the lookup and prevents an identical body
/// in another file from being returned. Empty hashes are skipped; first
/// definition wins on a same-file collision, matching the mangled-name map.
pub fn build_callables_by_file_hash(
    files: &[TypedFileRecord],
) -> HashMap<(String, String), ParsedCallable> {
    let mut map = HashMap::new();
    for file in files {
        for pc in &file.callables {
            if pc.metadata.hash.is_empty() {
                continue;
            }
            map.entry((file.file_path.clone(), pc.metadata.hash.clone()))
                .or_insert_with(|| pc.clone());
        }
    }
    map
}
use statix::{
    java::matcher::java_convert_full_header_to_mangled_name,
    python::matcher::python_convert_full_header_to_mangled_name,
};

/// Build a merged callable map across all files; first definition wins on collisions.
pub fn build_project_global_callables(
    files: &[TypedFileRecord],
) -> HashMap<String, ParsedCallable> {
    let mut merged = HashMap::new();
    for file in files {
        for pc in &file.callables {
            let mangled = mangle_callable_name(&pc.metadata.name, file.language);
            merged.entry(mangled).or_insert_with(|| pc.clone());
        }
    }
    merged
}

/// Mangle a callable's full-header name to the key form used in the callable map.
pub fn mangle_callable_name(name: &str, language: Language) -> String {
    match language {
        Language::Java => java_convert_full_header_to_mangled_name(name),
        Language::Python => python_convert_full_header_to_mangled_name(name),
        Language::Unknown => "Unknown language for mangling!".to_string(),
    }
}
