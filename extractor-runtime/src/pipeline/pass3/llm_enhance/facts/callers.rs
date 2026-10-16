//! `Callers` action: call sites of the enclosing function.

use models::Argument;

use super::{FactCtx, fact_path, scoped_files};
use crate::pipeline::pass3::llm_enhance::residual_edge_filter::strip_args;

pub(super) fn enclosing_name<'a>(ctx: &FactCtx<'a>) -> Option<&'a str> {
    ctx.project_ir
        .enclosing_callable(&ctx.rc.file_path, &ctx.rc.function_hash)
        .map(|c| c.metadata.name.as_str())
}

pub(super) fn callers_lines(ctx: &FactCtx) -> Vec<String> {
    let Some(name) = enclosing_name(ctx) else {
        return vec![];
    };
    let mut lines = Vec::new();
    for f in scoped_files(ctx) {
        for c in f.call_statements.iter().map(|c| &c.metadata) {
            let callee = strip_args(&c.function_name);
            if callee.rsplit('.').next() != Some(name) {
                continue;
            }
            lines.push(format!(
                "{}: in {}: {callee}({})",
                fact_path(&f.file_path, ctx.config),
                c.enclosing_function_name.as_deref().unwrap_or("<module>"),
                render_args(&c.arguments)
            ));
        }
    }
    lines.sort();
    lines
}

fn render_args(args: &[Argument]) -> String {
    args.iter()
        .map(|a| {
            if a.assigned_variable.is_empty() {
                a.value.clone()
            } else {
                format!("{}={}", a.assigned_variable, a.value)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::super::{Action, Explored, fetch};
    use crate::pipeline::pass3::llm_enhance::test_fixtures::{ORIGIN, world};

    #[test]
    fn callers_show_call_sites_with_args() {
        let w = world();
        let ctx = w.ctx();
        let fact = fetch(&ctx, &Action::Callers, &mut Explored::new(ORIGIN));
        assert_eq!(
            fact.body,
            "proj/caller/api.py: in get_case: self.client.fetch_case(case_id)"
        );
    }
}
