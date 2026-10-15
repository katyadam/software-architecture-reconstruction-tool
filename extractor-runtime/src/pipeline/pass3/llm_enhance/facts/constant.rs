//! `Constant` action: constants and env vars named like a binding.

use super::FactCtx;
use crate::pipeline::pass3::llm_enhance::residual_edge_filter::relative_file;

/// Every matching key, sorted: `MDS_URL`, `*_MDS_URL`, `*.MDS_URL`, project constant.
pub(super) fn constant_lines(ctx: &FactCtx, name: &str) -> Vec<String> {
    let upper = name.to_ascii_uppercase();
    let (suffix, dotted) = (format!("_{upper}"), format!(".{upper}"));
    let mut lines: Vec<String> = ctx
        .external_constants
        .iter()
        .filter(|(k, _)| {
            let k = k.to_ascii_uppercase();
            k == upper || k.ends_with(&suffix) || k.ends_with(&dotted)
        })
        .map(|(k, v)| format!("{k} = {v} (env/constants file)"))
        .collect();
    if let Some(c) = ctx.project_ir.constants.get(&upper) {
        lines.push(format!(
            "{} = {} ({})",
            c.name,
            c.value,
            relative_file(&c.source_file, ctx.config)
        ));
    }
    lines.sort();
    lines
}

#[cfg(test)]
mod tests {
    use super::super::{Action, Explored, fetch};
    use crate::pipeline::pass3::llm_enhance::test_fixtures::{ORIGIN, world};

    #[test]
    fn constant_lists_every_match_sorted() {
        let w = world();
        let ctx = w.ctx();
        let fact = fetch(
            &ctx,
            &Action::Constant("mds_url".to_string()),
            &mut Explored::new(ORIGIN),
        );
        assert_eq!(
            fact.body,
            "MDS_URL = http://mds:8000 (env/constants file)\n\
             PYTEST_MDS_URL = http://localhost:8000 (env/constants file)"
        );
    }
}
