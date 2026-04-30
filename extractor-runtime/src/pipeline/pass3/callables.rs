use std::collections::HashMap;

use models::{ParsedCallable, ir::project::TypedFileRecord};

use crate::pipeline::pass2::callables::mangle_callable_name;

/// Build a per-file callable map where local definitions override globals.
pub(crate) fn build_file_local_callables(
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
pub(crate) fn build_merged_enums(files: &[TypedFileRecord]) -> HashMap<String, Vec<String>> {
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
