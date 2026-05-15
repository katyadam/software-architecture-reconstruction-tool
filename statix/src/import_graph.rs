use std::collections::HashMap;

use models::{
    Scope,
    ir::{
        language::Language,
        project::{ImportGraph, ImportKind, ResolvedImport},
        syntax::FileRecord,
    },
};

use crate::{java::imports::resolve_java_import, python::imports::resolve_python_import};

// ── Public API ────────────────────────────────────────────────────────

/// Build an `ImportGraph` from all `FileRecord`s produced by Pass 1.
///
/// For each `Import` in each file we attempt to resolve its `codeword` to the
/// project-internal `FileRecord` that defines the referenced symbol.
/// Wildcard imports, third-party packages, and relative imports that cannot
/// be resolved are silently dropped — only intra-project resolutions appear
/// in the graph.
pub fn build_import_graph(file_records: &[FileRecord]) -> ImportGraph {
    let index = FileDefinitionsIndex::build(file_records);
    let mut resolved_imports: HashMap<(String, String), ResolvedImport> = HashMap::new();

    for record in file_records {
        for import in &record.imports {
            if import.orig_name == "*" {
                continue; // Wildcard imports cannot be resolved to a single symbol.
            }
            let resolved = match record.language {
                Language::Python => resolve_python_import(import, &index, &record.file_path),
                Language::Java => resolve_java_import(import, &index),
                Language::Unknown => None,
            };
            if let Some(ri) = resolved {
                resolved_imports
                    .entry((record.file_path.clone(), import.codeword.clone()))
                    .or_insert(ri);
            }
        }
    }

    ImportGraph { resolved_imports }
}

// ── Definition index ──────────────────────────────────────────────────

/// Per-file definitions, keyed by simple name, used for import resolution.
#[derive(Debug)]
pub struct FileDefinitions {
    /// Entity / class name -> entity signature.
    entities: HashMap<String, String>,
    /// Callable simple name -> signature.
    callables: HashMap<String, String>,
    /// Global-scope constant names.
    constants: HashMap<String, String>,
    /// Re-exported names: codeword -> (orig_module, orig_name).
    /// Populated from the file's own import statements so that symbols
    /// imported-and-re-exported can be followed across module boundaries.
    reexports: HashMap<String, (String, String)>,
}

impl FileDefinitions {
    /// Look up a symbol name among entities, callables, and constants.
    pub fn lookup(&self, name: &str) -> Option<(String, ImportKind)> {
        if let Some(sig) = self.entities.get(name) {
            return Some((sig.clone(), ImportKind::Entity));
        }
        if let Some(sig) = self.callables.get(name) {
            return Some((sig.clone(), ImportKind::Callable));
        }
        if self.constants.contains_key(name) {
            return Some((name.to_string(), ImportKind::Constant));
        }
        None
    }

    /// If `name` is not locally defined but was imported by this file,
    /// return `(orig_module, orig_name)` so the caller can follow the chain.
    pub fn reexport(&self, name: &str) -> Option<(&str, &str)> {
        self.reexports
            .get(name)
            .map(|(m, n)| (m.as_str(), n.as_str()))
    }
}

/// Project-wide index: normalized file path -> per-file definitions.
pub struct FileDefinitionsIndex {
    by_file: HashMap<String, FileDefinitions>,
}

impl FileDefinitionsIndex {
    /// Build the index from all `FileRecord`s.
    pub fn build(file_records: &[FileRecord]) -> Self {
        let mut by_file = HashMap::new();

        for record in file_records {
            let mut defs = FileDefinitions {
                entities: HashMap::new(),
                callables: HashMap::new(),
                constants: HashMap::new(),
                reexports: HashMap::new(),
            };

            for entity in &record.entities {
                defs.entities
                    .insert(entity.name.clone(), entity.signature.clone());
            }
            // Enums resolve the same way as entities from an import perspective.
            for enum_def in &record.enums {
                defs.entities
                    .entry(enum_def.name.clone())
                    .or_insert(enum_def.name.clone());
            }
            for pc in &record.callables {
                // Strip return type and parameter list — use only the simple function name.
                let simple = callable_simple_name(&pc.metadata.name);
                defs.callables
                    .entry(simple)
                    .or_insert(pc.metadata.signature.clone());
            }
            for (key, assignment) in &record.assignments {
                if matches!(key.scope, Scope::Global) {
                    defs.constants
                        .insert(assignment.variable_name.clone(), assignment.value.clone());
                }
            }
            for import in &record.imports {
                if !import.orig_name.is_empty() && import.orig_name != "*" {
                    defs.reexports
                        .entry(import.codeword.clone())
                        .or_insert_with(|| (import.orig_module.clone(), import.orig_name.clone()));
                }
            }

            by_file.insert(normalize_path(&record.file_path), defs);
        }

        Self { by_file }
    }

    /// Find definitions for a given module file path.
    ///
    /// Performs suffix matching so a stored path like `"src/myapp/services.py"` matches
    /// a module candidate `"myapp/services.py"` (handles project-root prefixes).
    ///
    /// Returns `(canonical_key, definitions)` where `canonical_key` is the actual key
    /// stored in the index (lifetime tied to `&self`).
    pub fn find_by_module_path<'a>(
        &'a self,
        module_file_path: &str,
    ) -> Option<(&'a str, &'a FileDefinitions)> {
        self.by_file
            .iter()
            .find(|(stored, _)| {
                stored.as_str() == module_file_path || stored.ends_with(module_file_path)
            })
            .map(|(k, v)| (k.as_str(), v))
    }
}

// ── Utilities ─────────────────────────────────────────────────────────

/// Normalize a file path to forward slashes for cross-platform consistency.
pub fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// Extract the simple callable name from a full callable name string.
///
/// - Java: `"String getUser(String id)"` -> `"getUser"`
/// - Python: `"get_user(name: str)"` -> `"get_user"`
pub fn callable_simple_name(name: &str) -> String {
    // Strip parameter list.
    let before_paren = name.split('(').next().unwrap_or(name).trim();
    // For Java, the last whitespace-delimited token is the method name;
    // the preceding tokens are the return type (e.g. `"String"`).
    before_paren
        .split_whitespace()
        .last()
        .unwrap_or(before_paren)
        .to_string()
}
