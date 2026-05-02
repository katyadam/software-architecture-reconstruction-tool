use models::{
    Import,
    ir::project::{ImportKind, ResolvedImport},
};

use crate::import_graph::FileDefinitionsIndex;

/// Convert a Java fully-qualified import path to a candidate source file path.
///
/// Java stores the complete import in `orig_module`, e.g.:
/// `import com.example.service.UserService` -> `orig_module = "com.example.service.UserService"`
///
/// We convert dots to slashes and append `.java`:
/// `"com.example.service.UserService"` -> `"com/example/service/UserService.java"`
pub fn java_import_to_file_path(orig_module: &str) -> String {
    format!("{}.java", orig_module.replace('.', "/"))
}

/// Resolve a Java import against the project definition index.
///
/// Returns a `ResolvedImport` when the imported class is found in a known
/// source file, `None` for third-party or otherwise unresolvable imports.
pub fn resolve_java_import(
    import: &Import,
    index: &FileDefinitionsIndex,
) -> Option<ResolvedImport> {
    let candidate = java_import_to_file_path(&import.orig_module);

    let (actual_path, defs) = index.find_by_module_path(&candidate)?;

    if let Some((fqn, kind)) = defs.lookup(&import.orig_name) {
        return Some(ResolvedImport {
            source_file: actual_path.to_string(),
            fully_qualified_name: fqn,
            kind,
        });
    }

    // File found in the index but the specific name isn't there — still record
    // a module-level reference (e.g. the file was parsed but the class wasn't captured).
    Some(ResolvedImport {
        source_file: actual_path.to_string(),
        fully_qualified_name: import.orig_module.clone(),
        kind: ImportKind::Module,
    })
}
