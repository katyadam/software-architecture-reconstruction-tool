//! `imports`: what the origin file imports, and who imports it.

use extractor_runtime::pipeline::facts::fact_path;
use models::{
    ConfigurationData,
    ir::project::{ProjectIR, TypedFileRecord},
};

/// Resolved imports only, sorted, project-relative. Importers keep the
/// origin's language, as every fact does; other services may show.
pub fn neighbours(
    origin: &TypedFileRecord,
    ir: &ProjectIR,
    config: &ConfigurationData,
) -> Vec<String> {
    let rel = |p: &str| fact_path(p, config);
    let imported = origin.imports.iter().filter_map(|i| {
        let resolved = ir.import_graph.lookup(&origin.file_path, &i.codeword)?;
        Some(format!(
            "imports {} from {}",
            i.codeword,
            rel(&resolved.source_file)
        ))
    });
    let importers = ir
        .import_graph
        .resolved_imports
        .iter()
        .filter(|((importer, _), r)| {
            r.source_file == origin.file_path && *importer != origin.file_path
        })
        .filter(|((importer, _), _)| {
            ir.files
                .iter()
                .any(|f| f.file_path == *importer && f.language == origin.language)
        })
        .map(|((importer, codeword), _)| format!("imported by {} as {codeword}", rel(importer)));
    let mut lines: Vec<String> = imported.chain(importers).collect();
    lines.sort();
    lines.dedup();
    lines
}
