use std::collections::HashMap;

use models::{
    ParsedCallable,
    ir::{language::Language, project::TypedFileRecord},
};
use statix::{
    go::matcher::go_convert_full_header_to_mangled_name,
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
        Language::Go => go_convert_full_header_to_mangled_name(name),
    }
}
