use std::path::PathBuf;

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

/// Convert a Python relative-import module path (e.g. `"....singletons"`) into
/// candidate source file paths anchored at the importer file's directory.
///
/// Python semantics: N leading dots mean "go up N-1 packages from the package
/// containing the importer file". `from . import x` stays in the current package,
/// `from .. import x` goes one up, etc. Any non-dot segments after the dots are
/// appended as nested package components.
///
/// Returns `None` if `module` has no leading dots, or if the dot count walks
/// past the project root.
fn python_relative_module_to_file_paths(
    importer_file_path: &str,
    module: &str,
) -> Option<Vec<String>> {
    let leading_dots = module.bytes().take_while(|&b| b == b'.').count();
    if leading_dots == 0 {
        return None;
    }
    let rest = &module[leading_dots..];

    let mut base = PathBuf::from(importer_file_path);
    // Strip the importer's filename, leaving the directory of its package.
    base.pop();
    // 1 dot = current package (no further pops); each extra dot pops one ancestor.
    for _ in 0..leading_dots.saturating_sub(1) {
        if !base.pop() {
            return None;
        }
    }
    if !rest.is_empty() {
        for segment in rest.split('.') {
            base.push(segment);
        }
    }

    let base_str = base.to_string_lossy().into_owned();
    Some(vec![
        format!("{base_str}.py"),
        format!("{base_str}/__init__.py"),
    ])
}

/// Resolve a Python import against the project definition index.
///
/// Handles two import forms:
/// - `import myapp.config` — `orig_name` is empty; resolves to the module file
/// - `from myapp.services import UserService` — resolves `orig_name` in the module file
///
/// Relative imports (`from ....singletons import settings`) are anchored at
/// `importer_file_path` since their dot prefix only makes sense relative to
/// the importing file's package.
///
/// Returns `None` for third-party or unresolvable imports.
pub fn resolve_python_import(
    import: &Import,
    index: &FileDefinitionsIndex,
    importer_file_path: &str,
) -> Option<ResolvedImport> {
    let candidates = python_relative_module_to_file_paths(importer_file_path, &import.orig_module)
        .unwrap_or_else(|| python_module_to_file_paths(&import.orig_module));

    if import.orig_name.is_empty() {
        // Plain module import: `import some.module`
        for candidate in candidates {
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
        for candidate in candidates {
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
