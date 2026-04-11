use std::collections::HashMap;

use models::{
    ParsedCallable,
    ir::{
        ast::Expr,
        project::{ConstantValue, TypedFileRecord},
    },
};

use super::language_backend::mangle_callable_name;

/// Build a merged callable map across all files; first definition wins on collisions.
pub(super) fn build_project_global_callables(
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

/// Build a per-file callable map where local definitions override globals.
pub(super) fn build_file_local_callables(
    file: &TypedFileRecord,
    global_callables: &HashMap<String, ParsedCallable>,
) -> HashMap<String, ParsedCallable> {
    let mut map = global_callables.clone();
    for pc in &file.callables {
        let mangled = mangle_callable_name(&pc.metadata.name, file.language);
        map.insert(mangled, pc.clone());
    }
    map
}

/// Merge enum variant maps across all files; first definition wins on name collisions.
pub(super) fn build_merged_enums(files: &[TypedFileRecord]) -> HashMap<String, Vec<String>> {
    let mut enums = HashMap::new();
    for file in files {
        for e in &file.enums {
            enums
                .entry(e.name.clone())
                .or_insert_with(|| e.variants.clone());
        }
    }
    enums
}

/// Convert project constants to the initial symbolic evaluation environment.
///
/// Quoted string values have their surrounding quotes stripped so that
/// `BASE_URL = "/api/v1"` becomes `Expr::Literal("/api/v1")`.
pub(super) fn constants_to_env(
    constants: &HashMap<String, ConstantValue>,
) -> HashMap<String, (Option<String>, Expr)> {
    constants
        .iter()
        .map(|(name, cv)| {
            let value = cv.value.trim_matches(|c| c == '"' || c == '\'').to_string();
            (
                name.clone(),
                (Some("String".to_string()), Expr::Literal(value)),
            )
        })
        .collect()
}
