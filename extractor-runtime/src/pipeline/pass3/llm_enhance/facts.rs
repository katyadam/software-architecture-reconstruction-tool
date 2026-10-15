//! DDFA facts: what Jev may acquire about a residual, and fetching it.
//! Lookups stay in the origin file's language; non-file lookups also stay
//! in the origin service.

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
                let rel = relative_file(path, ctx.config);
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

/// Names worth defining: last segment of identifiers in binding values
/// (`settings.mds_url` -> `mds_url`); skips `unresolved` and `: type`.
// ponytail: tokens inside string literals count too; strip quotes if noise bites.
fn symbols(bindings: &OperandBindings) -> BTreeSet<String> {
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

/// Imports of every opened file, same language, not yet opened, present in IR.
/// Imports the call site mentions come first.
fn file_options(ctx: &FactCtx, explored: &Explored) -> Vec<Action> {
    let mut seen = BTreeSet::new();
    let mut options: Vec<(bool, Action)> = Vec::new();
    for importer in explored
        .opened
        .iter()
        .filter_map(|p| find_file(ctx.project_ir, p))
    {
        for import in &importer.imports {
            let Some(resolved) = ctx
                .project_ir
                .import_graph
                .lookup(&importer.file_path, &import.codeword)
            else {
                continue;
            };
            let Some(target) = find_file(ctx.project_ir, &resolved.source_file) else {
                continue;
            };
            if !same_language(ctx.origin, target)
                || explored.opened.contains(&target.file_path)
                || !seen.insert(target.file_path.clone())
            {
                continue;
            }
            options.push((
                !mentioned(ctx, &import.codeword),
                Action::OpenFile {
                    path: target.file_path.clone(),
                    codeword: import.codeword.clone(),
                },
            ));
        }
    }
    options.sort_by_key(|(unmentioned, _)| *unmentioned);
    options
        .into_iter()
        .map(|(_, a)| a)
        .take(MAX_FILE_OPTIONS)
        .collect()
}

fn mentioned(ctx: &FactCtx, codeword: &str) -> bool {
    ctx.rc.target_uri.contains(codeword)
        || ctx
            .state
            .snippet
            .as_deref()
            .unwrap_or_default()
            .contains(codeword)
        || ctx.bindings.values().any(|v| v.contains(codeword))
}

/// Re-checks the language guard: a stale option must not cross languages.
fn open_file(ctx: &FactCtx, path: &str, codeword: &str, explored: &mut Explored) -> Vec<String> {
    let Some(file) = find_file(ctx.project_ir, path).filter(|f| same_language(ctx.origin, f))
    else {
        return vec![];
    };
    explored.opened.push(file.file_path.clone());
    file_lines(file, codeword)
}

/// Global/class assignments, entities, callables, REST calls; lines naming
/// `symbol` first.
fn file_lines(file: &TypedFileRecord, symbol: &str) -> Vec<String> {
    let mut assignments: Vec<_> = file
        .assignments
        .iter()
        .filter(|(k, _)| matches!(k.scope, Scope::Global | Scope::Class(_)))
        .collect();
    assignments.sort_by(|a, b| a.0.variable_name.cmp(&b.0.variable_name));

    let mut lines: Vec<String> = assignments
        .into_iter()
        .map(|(k, a)| assignment_line(k, a))
        .collect();
    lines.extend(file.entities.iter().map(entity_line));
    lines.extend(
        file.callables
            .iter()
            .map(|c| format!("callable {}", c.metadata.signature)),
    );
    lines.extend(
        file.raw_restcalls
            .iter()
            .map(|r| format!("rest call {:?} {}", r.http_method, r.target_uri)),
    );
    lines.sort_by_key(|l| !l.contains(symbol));
    lines
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

// Filled in by Task 3.
fn define_lines(_ctx: &FactCtx, _name: &str) -> Vec<String> {
    vec![]
}
fn callers_lines(_ctx: &FactCtx) -> Vec<String> {
    vec![]
}
fn receiver_lines(_ctx: &FactCtx) -> Vec<String> {
    vec![]
}
fn constant_lines(_ctx: &FactCtx, _name: &str) -> Vec<String> {
    vec![]
}
fn enclosing_name<'a>(_ctx: &FactCtx<'a>) -> Option<&'a str> {
    None
}
fn receiver_entity<'a>(_ctx: &FactCtx<'a>) -> Option<(&'a TypedFileRecord, &'a Entity)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::pass3::llm_enhance::test_fixtures::{ORIGIN, file, world};
    use models::ir::language::Language;

    fn ids(ctx: &FactCtx, actions: &[Action]) -> Vec<String> {
        actions.iter().map(|a| a.option(ctx).0).collect()
    }

    #[test]
    #[ignore = "needs Task 3"]
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
    fn open_file_never_crosses_language() {
        let w = world();
        let ctx = w.ctx();
        let mut explored = Explored::new(ORIGIN);
        let offered = ids(&ctx, &available(&ctx, &explored));
        assert!(!offered.iter().any(|id| id.contains("Stub.java")));
        assert!(!offered.iter().any(|id| id.contains("gone.py")));

        // A stale or forged option is re-checked on fetch.
        let java = Action::OpenFile {
            path: "/proj/caller/Stub.java".to_string(),
            codeword: "Stub".to_string(),
        };
        let fact = fetch(&ctx, &java, &mut explored);
        assert_eq!(fact.body, "(nothing found)");
        assert_eq!(fact.language, "Python");
        assert!(!ids(&ctx, &available(&ctx, &explored)).is_empty());
    }

    #[test]
    fn open_file_summarizes_and_hops() {
        let w = world();
        let ctx = w.ctx();
        let mut explored = Explored::new(ORIGIN);
        let open = Action::OpenFile {
            path: "/proj/caller/settings.py".to_string(),
            codeword: "settings".to_string(),
        };
        let fact = fetch(&ctx, &open, &mut explored);
        assert_eq!(fact.action, "open:proj/caller/settings.py#settings");
        assert_eq!(
            fact.body,
            "TIMEOUT = 10\nclass Settings(BaseSettings): mds_url: str"
        );

        let offered = ids(&ctx, &available(&ctx, &explored));
        assert!(offered.contains(&"open:proj/caller/config.py#config".to_string()));
        assert!(!offered.iter().any(|id| id.contains("settings.py")));
    }

    #[test]
    fn opened_files_are_not_offered_again() {
        let w = world();
        let ctx = w.ctx();
        let mut explored = Explored::new(ORIGIN);
        let open = Action::OpenFile {
            path: "/proj/caller/settings.py".to_string(),
            codeword: "settings".to_string(),
        };
        fetch(&ctx, &open, &mut explored);
        // settings.py imports client.py back: the origin is never offered.
        let offered = ids(&ctx, &available(&ctx, &explored));
        assert!(!offered.iter().any(|id| id.contains("client.py")));
    }

    #[test]
    fn file_options_capped_and_ranked() {
        let mut w = world();
        // Two-digit codewords: no codeword is a substring of another.
        for i in 0..30 {
            let path = format!("/proj/caller/mod_{i:02}.py");
            w.ir.files.push(file(&path, Language::Python));
            w.link(ORIGIN, &format!("mod_{i:02}"), &path);
        }
        w.state.snippet = Some("use(mod_29)".to_string());
        let ctx = w.ctx();
        let opens: Vec<String> = ids(&ctx, &available(&ctx, &Explored::new(ORIGIN)))
            .into_iter()
            .filter(|id| id.starts_with("open:"))
            .collect();
        assert_eq!(opens.len(), MAX_FILE_OPTIONS);
        // Mentioned first, link order kept: `settings` (bindings), then `mod_29` (snippet).
        assert_eq!(
            opens[..2],
            [
                "open:proj/caller/settings.py#settings".to_string(),
                "open:proj/caller/mod_29.py#mod_29".to_string(),
            ]
        );
    }
}
