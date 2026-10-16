//! `Receiver` action: class of the client object, else the enclosing class.

use models::{Entity, Namespace, ir::project::TypedFileRecord};

use super::{FactCtx, entity_line, fact_path, scoped_files};

/// Receiver datatype if it is a project class, else the enclosing class.
pub(super) fn receiver_entity<'a>(ctx: &FactCtx<'a>) -> Option<(&'a TypedFileRecord, &'a Entity)> {
    let datatype = ctx
        .state
        .receiver
        .as_ref()
        .and_then(|r| r.datatype.as_deref())
        .and_then(|t| t.rsplit('.').next());
    [datatype, ctx.state.enclosing.class.as_deref()]
        .into_iter()
        .flatten()
        .find_map(|name| {
            scoped_files(ctx)
                .find_map(|f| f.entities.iter().find(|e| e.name == name).map(|e| (f, e)))
        })
}

pub(super) fn receiver_lines(ctx: &FactCtx) -> Vec<String> {
    let Some((file, entity)) = receiver_entity(ctx) else {
        return vec![];
    };
    let mut lines = vec![format!(
        "{}: {}",
        fact_path(&file.file_path, ctx.config),
        entity_line(entity)
    )];
    if let Some(parents) = ctx
        .project_ir
        .class_hierarchy
        .parents
        .get(&entity.signature)
    {
        lines.push(format!("parents: {}", parents.join(", ")));
    }
    lines.extend(
        file.callables
            .iter()
            .map(|c| &c.metadata)
            .filter(|c| {
                c.is_constructor && matches!(&c.namespace, Namespace::Class(n) if *n == entity.name)
            })
            .map(|c| format!("constructor: {}", c.signature)),
    );
    lines
}

#[cfg(test)]
mod tests {
    use super::super::{Action, Explored, fetch};
    use crate::pipeline::pass3::llm_enhance::test_fixtures::{ORIGIN, world};

    #[test]
    fn receiver_falls_back_to_enclosing_class() {
        let w = world();
        let ctx = w.ctx();
        // `httpx.AsyncClient` is not a project entity -> enclosing `MdsClient`.
        let fact = fetch(&ctx, &Action::Receiver, &mut Explored::new(ORIGIN));
        assert_eq!(
            fact.body,
            "proj/caller/client.py: class MdsClient: _mds_url = settings.mds_url"
        );
    }
}
