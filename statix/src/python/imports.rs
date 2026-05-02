use models::{
    Import,
    ir::project::{ImportKind, ResolvedImport},
};

use crate::import_graph::FileDefinitionsIndex;

/// Convert a Python dotted module path to candidate source file paths.
///
/// A module `"myapp.services.user"` can be implemented as either:
/// - `myapp/services/user.py` (regular module file)
/// - `myapp/services/user/__init__.py` (package init)
pub fn python_module_to_file_paths(module: &str) -> Vec<String> {
    let base = module.replace('.', "/");
    vec![format!("{base}.py"), format!("{base}/__init__.py")]
}

/// Resolve a Python import against the project definition index.
///
/// Handles two import forms:
/// - `import myapp.config` — `orig_name` is empty; resolves to the module file
/// - `from myapp.services import UserService` — resolves `orig_name` in the module file
///
/// Returns `None` for third-party or unresolvable imports.
pub fn resolve_python_import(
    import: &Import,
    index: &FileDefinitionsIndex,
) -> Option<ResolvedImport> {
    if import.orig_name.is_empty() {
        // Plain module import: `import some.module`
        for candidate in python_module_to_file_paths(&import.orig_module) {
            if let Some((actual_path, _)) = index.find_by_module_path(&candidate) {
                return Some(ResolvedImport {
                    source_file: actual_path.to_string(),
                    fully_qualified_name: import.orig_module.clone(),
                    kind: ImportKind::Module,
                });
            }
        }
        None
    } else {
        // Named import: `from some.module import SomeName`
        for candidate in python_module_to_file_paths(&import.orig_module) {
            if let Some((actual_path, defs)) = index.find_by_module_path(&candidate)
                && let Some((fqn, kind)) = defs.lookup(&import.orig_name)
            {
                return Some(ResolvedImport {
                    source_file: actual_path.to_string(),
                    fully_qualified_name: fqn,
                    kind,
                });
            }
        }
        None
    }
}
