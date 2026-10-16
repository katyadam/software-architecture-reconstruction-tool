//! DDFA facts: what Jev may acquire about a residual, and fetching it.
//! Lookups stay in the origin file's language; non-file lookups also stay
//! in the origin service.

mod callers;
mod constant;
mod define;
mod open_file;
mod receiver;

use std::collections::{BTreeSet, HashMap};

use clients::typesafe::{
    question::{MAX_CHOICE_OPTIONS, resolve::Opt},
    state::{Fact, ResidualCallState},
};
use models::{
    Assignment, AssignmentKey, ConfigurationData, Entity, RestCall, Scope,
    ir::project::{ProjectIR, TypedFileRecord},
};

use crate::pipeline::pass3::llm_enhance::residual_edge_filter::relative_file;
use crate::pipeline::pass3::restcalls::OperandBindings;
use callers::{callers_lines, enclosing_name};
use constant::constant_lines;
use define::{define_lines, symbols};
use open_file::{file_options, open_file};
use receiver::{receiver_entity, receiver_lines};

/// Max `OpenFile` options per step.
pub(super) const MAX_FILE_OPTIONS: usize = 20;
/// Max lines of one fact body.
const MAX_FACT_LINES: usize = 40;
const NOTHING: &str = "(nothing found)";

pub(super) struct FactCtx<'a> {
    pub project_ir: &'a ProjectIR,
    pub config: &'a ConfigurationData,
    /// Constants file + scraped env, as fed to symbolic evaluation.
    pub external_constants: &'a HashMap<String, String>,
    pub rc: &'a RestCall,
    pub bindings: &'a OperandBindings,
    pub state: &'a ResidualCallState,
    /// File of the residual; its language bounds every lookup.
    pub origin: &'a TypedFileRecord,
    /// `base_dir_path` of the origin service; empty -> origin file only.
    pub service_dir: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Action {
    /// Where a name from the operand bindings is assigned or declared.
    Define(String),
    /// Call sites of the enclosing function.
    Callers,
    /// Class of the client object, else the enclosing class.
    Receiver,
    /// Constants and env vars named like this.
    Constant(String),
    /// Summary of an imported file.
    OpenFile { path: String, codeword: String },
}

impl Action {
    /// Choice option: stable id + description for Jev.
    pub(super) fn option(&self, ctx: &FactCtx) -> Opt {
        match self {
            Action::Define(name) => (
                format!("define:{name}"),
                format!("Where `{name}` is assigned or declared as a class field."),
            ),
            Action::Callers => (
                "callers".to_string(),
                format!(
                    "Call sites of `{}` and the arguments they pass.",
                    ctx.state.enclosing.function
                ),
            ),
            Action::Receiver => (
                "receiver".to_string(),
                "Class of the client object or enclosing class: fields, parents, constructor."
                    .to_string(),
            ),
            Action::Constant(name) => (
                format!("constant:{name}"),
                format!("Constants and deployment env vars named like `{name}`."),
            ),
            Action::OpenFile { path, codeword } => {
                let rel = fact_path(path, ctx.config);
                (
                    format!("open:{rel}#{codeword}"),
                    format!("Open {rel} (imported as `{codeword}`): settings, classes, functions."),
                )
            }
        }
    }
}

/// What was acquired so far.
#[derive(Debug)]
pub(super) struct Explored {
    taken: BTreeSet<String>,
    /// Files whose imports are offered; starts with the origin.
    opened: Vec<String>,
}

impl Explored {
    pub(super) fn new(origin: &str) -> Self {
        Self {
            taken: BTreeSet::new(),
            opened: vec![origin.to_string()],
        }
    }
}

fn same_language(origin: &TypedFileRecord, target: &TypedFileRecord) -> bool {
    origin.language == target.language
}

fn find_file<'a>(ir: &'a ProjectIR, path: &str) -> Option<&'a TypedFileRecord> {
    ir.files.iter().find(|f| f.file_path == path)
}

/// Not yet taken actions, best first; one Choice slot left for `stop`.
pub(super) fn available(ctx: &FactCtx, explored: &Explored) -> Vec<Action> {
    let mut actions = Vec::new();
    for name in symbols(ctx.bindings) {
        let has_constant = !constant_lines(ctx, &name).is_empty();
        actions.push(Action::Define(name.clone()));
        if has_constant {
            actions.push(Action::Constant(name));
        }
    }
    if enclosing_name(ctx).is_some() {
        actions.push(Action::Callers);
    }
    if receiver_entity(ctx).is_some() {
        actions.push(Action::Receiver);
    }
    actions.extend(file_options(ctx, explored));
    actions.retain(|a| !explored.taken.contains(&a.option(ctx).0));
    actions.truncate(MAX_CHOICE_OPTIONS - 1);
    actions
}

/// Marks `action` taken; an empty lookup yields `(nothing found)`.
pub(super) fn fetch(ctx: &FactCtx, action: &Action, explored: &mut Explored) -> Fact {
    let (id, _) = action.option(ctx);
    explored.taken.insert(id.clone());
    let lines = match action {
        Action::Define(name) => define_lines(ctx, name),
        Action::Callers => callers_lines(ctx),
        Action::Receiver => receiver_lines(ctx),
        Action::Constant(name) => constant_lines(ctx, name),
        Action::OpenFile { path, codeword } => open_file(ctx, path, codeword, explored),
    };
    Fact {
        action: id,
        language: ctx.origin.language.to_string(),
        body: render(lines),
    }
}

fn render(lines: Vec<String>) -> String {
    if lines.is_empty() {
        return NOTHING.to_string();
    }
    let more = lines.len().saturating_sub(MAX_FACT_LINES);
    let mut body = lines
        .into_iter()
        .take(MAX_FACT_LINES)
        .collect::<Vec<_>>()
        .join("\n");
    if more > 0 {
        body.push_str(&format!("\n... {more} more"));
    }
    body
}

fn assignment_line(key: &AssignmentKey, a: &Assignment) -> String {
    match &key.scope {
        Scope::Global => format!("{} = {}", key.variable_name, a.value),
        Scope::Class(c) => format!("{c}.{} = {}", key.variable_name, a.value),
        Scope::Function(f) => format!("in {f}: {} = {}", key.variable_name, a.value),
    }
}

/// `class Settings(BaseSettings): mds_url: str, timeout = 10`
fn entity_line(e: &Entity) -> String {
    let parents = if e.superclasses.is_empty() {
        String::new()
    } else {
        format!("({})", e.superclasses.join(", "))
    };
    let fields: Vec<String> = e
        .fields
        .iter()
        .map(|f| {
            let mut s = f.name.clone();
            if let Some(t) = &f.datatype {
                s.push_str(&format!(": {t}"));
            }
            if let Some(v) = &f.initial_value {
                s.push_str(&format!(" = {v}"));
            }
            s
        })
        .collect();
    format!("class {}{parents}: {}", e.name, fields.join(", "))
}

/// `relative_file` minus leading `./` / `../` segments (cwd-relative base dirs).
fn fact_path(path: &str, config: &ConfigurationData) -> String {
    let rel = relative_file(path, config);
    let mut s = rel.as_str();
    while let Some(rest) = s.strip_prefix("../").or_else(|| s.strip_prefix("./")) {
        s = rest;
    }
    s.to_string()
}

/// Same language, and the origin file or the origin service.
fn scoped_files<'a>(ctx: &FactCtx<'a>) -> impl Iterator<Item = &'a TypedFileRecord> {
    let (origin, dir) = (ctx.origin, ctx.service_dir);
    ctx.project_ir.files.iter().filter(move |f| {
        same_language(origin, f)
            && (f.file_path == origin.file_path || (!dir.is_empty() && f.file_path.contains(dir)))
    })
}
#[cfg(test)]
fn ids(ctx: &FactCtx, actions: &[Action]) -> Vec<String> {
    actions.iter().map(|a| a.option(ctx).0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::pass3::llm_enhance::test_fixtures::{ORIGIN, world};

    #[test]
    fn first_step_offers_every_kind() {
        let w = world();
        let ctx = w.ctx();
        let actions = available(&ctx, &Explored::new(ORIGIN));
        assert_eq!(
            ids(&ctx, &actions),
            vec![
                "define:mds_url",
                "constant:mds_url",
                "callers",
                "receiver",
                "open:proj/caller/settings.py#settings",
            ]
        );
    }

    #[test]
    fn taken_actions_are_not_offered_again() {
        let w = world();
        let ctx = w.ctx();
        let mut explored = Explored::new(ORIGIN);
        fetch(&ctx, &Action::Callers, &mut explored);
        assert!(!ids(&ctx, &available(&ctx, &explored)).contains(&"callers".to_string()));
    }

    #[test]
    fn fact_path_drops_leading_parent_dirs() {
        let mut w = world();
        w.config.service_descriptions[0].base_dir_path = "../empaia/caller".to_string();
        let ctx = w.ctx();
        let open = Action::OpenFile {
            path: "../empaia/caller/settings.py".to_string(),
            codeword: "settings".to_string(),
        };
        assert_eq!(
            open.option(&ctx).0,
            "open:empaia/caller/settings.py#settings"
        );
    }
}
