use std::collections::HashMap;

use models::ir::project::{ClassHierarchy, ImportGraph, TypedFileRecord};

/// Build a `ClassHierarchy` from all `TypedFileRecord`s and the resolved `ImportGraph`.
///
/// For each entity, each raw name in `Entity.superclasses` is resolved to a
/// fully-qualified signature — first via the import graph (cross-file), then via
/// a same-file fallback. Unresolvable names (stdlib, third-party, missing imports)
/// are silently skipped.
pub fn build_class_hierarchy(
    files: &[TypedFileRecord],
    import_graph: &ImportGraph,
) -> ClassHierarchy {
    let same_file_index = build_same_file_index(files);

    let mut parents: HashMap<String, Vec<String>> = HashMap::new();
    let mut children: HashMap<String, Vec<String>> = HashMap::new();

    for file in files {
        for entity in &file.entities {
            for raw_name in &entity.superclasses {
                if let Some(parent_sig) =
                    resolve_superclass(raw_name, &file.file_path, import_graph, &same_file_index)
                {
                    register_relationship(
                        &mut parents,
                        &mut children,
                        &entity.signature,
                        parent_sig,
                    );
                }
            }
        }
    }

    ClassHierarchy { parents, children }
}

// ── Internal helpers ──────────────────────────────────────────────────

/// Build a `(file_path, simple_name) -> entity_signature` index for same-file lookups.
///
/// Used as a fallback when a superclass is defined in the same file and therefore
/// has no import entry.
fn build_same_file_index(files: &[TypedFileRecord]) -> HashMap<(String, String), String> {
    files
        .iter()
        .flat_map(|f| {
            f.entities
                .iter()
                .map(|e| ((f.file_path.clone(), e.name.clone()), e.signature.clone()))
        })
        .collect()
}

/// Resolve a raw superclass name to its fully-qualified signature.
///
/// Resolution order:
/// 1. Import graph lookup — handles cross-file (and cross-service) references.
/// 2. Same-file index — handles superclasses defined in the same file with no import.
///
/// Returns `None` if the name cannot be resolved (stdlib, third-party, missing import).
fn resolve_superclass(
    raw_name: &str,
    file_path: &str,
    import_graph: &ImportGraph,
    same_file_index: &HashMap<(String, String), String>,
) -> Option<String> {
    import_graph
        .lookup(file_path, raw_name)
        .map(|r| r.fully_qualified_name.clone())
        .or_else(|| {
            same_file_index
                .get(&(file_path.to_string(), raw_name.to_string()))
                .cloned()
        })
}

/// Insert a `child -> parent` edge and its inverse `parent -> child` into the maps.
fn register_relationship(
    parents: &mut HashMap<String, Vec<String>>,
    children: &mut HashMap<String, Vec<String>>,
    child_sig: &str,
    parent_sig: String,
) {
    parents
        .entry(child_sig.to_string())
        .or_default()
        .push(parent_sig.clone());

    children
        .entry(parent_sig)
        .or_default()
        .push(child_sig.to_string());
}
