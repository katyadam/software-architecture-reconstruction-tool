use models::{RouteFact, Scope, ir::project::TypedFileRecord};

/// `X.method(child, prefix)` attaching `child` under `X`; args as `(keyword, position)`.
pub(super) struct AttachCall {
    pub method: &'static str,
    pub child: (&'static str, Option<usize>),
    pub prefix: (&'static str, Option<usize>),
}

/// `r = ctor(..., prefix_kw=...)` giving router `r` its own prefix.
pub(super) struct OwnCtor {
    pub ctor: &'static str,
    pub prefix_kw: &'static str,
}

/// Route facts for one framework's attach calls and router constructors.
pub(super) fn facts_from_idioms(
    file: &TypedFileRecord,
    attach: &[AttachCall],
    own: &[OwnCtor],
) -> Vec<RouteFact> {
    let calls = file.call_statements.iter().filter_map(|c| {
        let c = &c.metadata;
        let (parent, method) = c.function_name.rsplit_once('.')?;
        let idiom = attach.iter().find(|i| i.method == method)?;
        let arg = |(kw, pos): (&str, Option<usize>)| {
            let keyword = c
                .arguments
                .iter()
                .find(|a| a.assigned_variable.trim() == kw);
            let positional = || {
                c.arguments
                    .iter()
                    .filter(|a| a.assigned_variable.trim().is_empty())
                    .nth(pos?)
            };
            keyword
                .or_else(positional)
                .map(|a| a.value.trim().to_string())
        };
        Some(RouteFact::Attach {
            scope: c
                .enclosing_function_name
                .clone()
                .map_or(Scope::Global, Scope::Function),
            parent: parent.trim().to_string(),
            child: arg(idiom.child)?,
            // No prefix -> empty literal, which the resolver evaluates to `""`.
            prefix: arg(idiom.prefix).unwrap_or_else(|| "\"\"".to_string()),
        })
    });
    // ponytail: naive `,` split of the constructor text; commas inside literals break it.
    let owns = file.assignments.iter().filter_map(|(key, a)| {
        let (head, args) = a.value.trim().split_once('(')?;
        // `APIRouter(` or qualified `fastapi.APIRouter(`.
        let ctor = head.trim().rsplit('.').next()?;
        let idiom = own.iter().find(|i| i.ctor == ctor)?;
        let prefix = args.strip_suffix(')')?.split(',').find_map(|arg| {
            arg.trim()
                .strip_prefix(idiom.prefix_kw)?
                .trim_start()
                .strip_prefix('=')
        })?;
        Some(RouteFact::Own {
            scope: key.scope.clone(),
            router: key.variable_name.clone(),
            prefix: prefix.trim().to_string(),
        })
    });
    calls.chain(owns).collect()
}
