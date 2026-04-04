use std::collections::HashMap;

use models::{
    Import,
    ir::project::{ImportGraph, ImportKind, ResolvedImport},
};

/// Build a minimal single-file `ImportGraph` from raw Java imports.
///
/// For each import, maps `(file_path, codeword) -> orig_module` as the fully-qualified name.
/// This lets the evaluator resolve field types against the same import information that was
/// previously passed as `&[Import]`, without needing cross-file `FileRecord`s.
/// This will be most likely deprecated, since imports will be solely resolved in a cross-file manner.
pub fn imports_to_graph(file_path: &str, imports: &[Import]) -> ImportGraph {
    let resolved_imports = imports
        .iter()
        .filter(|imp| imp.orig_name != "*")
        .map(|imp| {
            let ri = ResolvedImport {
                source_file: String::new(),
                fully_qualified_name: imp.orig_module.clone(),
                kind: ImportKind::Entity,
            };
            ((file_path.to_string(), imp.codeword.clone()), ri)
        })
        .collect::<HashMap<_, _>>();
    ImportGraph { resolved_imports }
}
