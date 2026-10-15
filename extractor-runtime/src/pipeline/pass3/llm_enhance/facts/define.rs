//! `Define` action: where a binding name is assigned or declared.

use std::collections::BTreeSet;

use super::{FactCtx, assignment_line, entity_line, scoped_files};
use crate::pipeline::pass3::llm_enhance::residual_edge_filter::relative_file;
use crate::pipeline::pass3::restcalls::OperandBindings;

/// Names worth defining: last segment of identifiers in binding values
/// (`settings.mds_url` -> `mds_url`); skips `unresolved` and `: type`.
// ponytail: tokens inside string literals count too; strip quotes if noise bites.
pub(super) fn symbols(bindings: &OperandBindings) -> BTreeSet<String> {
    bindings
        .values()
        .map(|v| v.split(": ").next().unwrap_or(v))
        .flat_map(|v| v.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.')))
        .filter(|t| !t.is_empty() && *t != "unresolved")
        .filter_map(|t| t.rsplit('.').next())
        .filter(|s| s.starts_with(|c: char| c.is_alphabetic() || c == '_'))
        .map(str::to_string)
        .collect()
}

pub(super) fn define_lines(ctx: &FactCtx, name: &str) -> Vec<String> {
    let dotted = format!(".{name}");
    let mut lines = Vec::new();
    for f in scoped_files(ctx) {
        let rel = relative_file(&f.file_path, ctx.config);
        for (k, a) in &f.assignments {
            if k.variable_name == name || k.variable_name.ends_with(&dotted) {
                lines.push(format!("{rel}: {}", assignment_line(k, a)));
            }
        }
        for e in f
            .entities
            .iter()
            .filter(|e| e.fields.iter().any(|fl| fl.name == name))
        {
            lines.push(format!("{rel}: {}", entity_line(e)));
        }
    }
    lines.sort();
    lines
}

#[cfg(test)]
mod tests {
    use super::super::{Action, Explored, fetch};
    use crate::pipeline::pass3::llm_enhance::test_fixtures::{ORIGIN, entity, file, world};
    use models::ir::language::Language;

    #[test]
    fn define_finds_field_across_service() {
        let w = world();
        let ctx = w.ctx();
        let fact = fetch(
            &ctx,
            &Action::Define("mds_url".to_string()),
            &mut Explored::new(ORIGIN),
        );
        assert_eq!(
            fact.body,
            "proj/caller/settings.py: class Settings(BaseSettings): mds_url: str"
        );
    }

    #[test]
    fn lookups_skip_other_languages() {
        let mut w = world();
        let mut java = file("/proj/caller/Settings.java", Language::Java);
        java.entities.push(entity(
            "JSettings",
            "/proj/caller/Settings.java",
            &[],
            &[("mds_url", Some("String"), None)],
        ));
        w.ir.files.push(java);
        let ctx = w.ctx();
        let fact = fetch(
            &ctx,
            &Action::Define("mds_url".to_string()),
            &mut Explored::new(ORIGIN),
        );
        assert!(!fact.body.contains("JSettings"));
    }
}
