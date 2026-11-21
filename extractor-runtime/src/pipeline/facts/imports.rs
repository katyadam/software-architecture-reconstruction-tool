//! `imports`: what the origin file imports, and who imports it.
//! Offered by the agentic tools only, not as an `Action`, so DDFA stays as it was.

use super::{FactCtx, fact_path};

/// Resolved imports only, sorted, project-relative. Importers keep the
/// origin's language, as every fact does; other services may show.
pub fn imports_lines(ctx: &FactCtx) -> Vec<String> {
    let (origin, ir) = (ctx.origin, ctx.project_ir);
    let rel = |p: &str| fact_path(p, ctx.config);
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

#[cfg(test)]
mod tests {
    use super::imports_lines;
    use crate::pipeline::facts::test_fixtures::{ORIGIN, world};

    #[test]
    fn both_directions_same_language() {
        let mut w = world();
        // Another language importing the origin stays out.
        w.link("/proj/caller/Stub.java", "client", ORIGIN);
        let lines = imports_lines(&w.ctx());
        assert!(lines.contains(&"imports settings from proj/caller/settings.py".to_string()));
        assert!(lines.contains(&"imported by proj/caller/settings.py as client".to_string()));
        assert!(!lines.iter().any(|l| l.contains("Stub.java as client")));
    }
}
