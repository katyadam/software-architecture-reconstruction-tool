use std::{collections::HashMap, path::PathBuf};

use models::{
    Import,
    ir::project::{ImportGraph, ImportKind, ResolvedImport},
};

/// Build a minimal single-file `ImportGraph` from raw Python imports.
///
/// For each named import, maps `(file_path, codeword)` to the path-style fully-qualified name
/// constructed from the importer file path and the module path. This replicates the single-file
/// resolution that the old `&[Import]`-based evaluator provided, without needing cross-file
/// `FileRecord`s.
pub fn imports_to_graph(file_path: &str, imports: &[Import]) -> ImportGraph {
    let resolved_imports = imports
        .iter()
        .filter(|imp| !imp.orig_name.is_empty())
        .map(|imp| {
            let fqn = import_fqn(file_path, &imp.orig_module, &imp.orig_name);
            let ri = ResolvedImport {
                source_file: String::new(),
                fully_qualified_name: fqn,
                kind: ImportKind::Entity,
            };
            ((file_path.to_string(), imp.codeword.clone()), ri)
        })
        .collect::<HashMap<_, _>>();
    ImportGraph { resolved_imports }
}

/// Compute the path-style fully-qualified name for a Python imported symbol.
///
/// Starting from `file_path`, navigates up by the number of `.`-separated segments in
/// `module_path`, then descends through those segments and appends `symbol`.
///
/// Example: `file_path = "a/b/c.py"`, `module_path = "x.y"`, `symbol = "User"`
/// -> `"a/x/y/User"`
fn import_fqn(file_path: &str, module_path: &str, symbol: &str) -> String {
    let mut path = PathBuf::from(file_path);
    path.pop();
    let parts: Vec<&str> = module_path.split('.').collect();
    for _ in 0..parts.len() {
        path.pop();
    }
    parts.iter().for_each(|part| path.push(part));
    format!("{}/{}", path.display(), symbol)
}
