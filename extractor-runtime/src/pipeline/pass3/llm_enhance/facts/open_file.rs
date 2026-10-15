//! `OpenFile` action: offer imported files and summarize the opened one.

use std::collections::BTreeSet;

use models::{Scope, ir::project::TypedFileRecord};

use super::{
    Action, Explored, FactCtx, MAX_FILE_OPTIONS, assignment_line, entity_line, find_file,
    same_language,
};

/// Imports of every opened file, same language, not yet opened, present in IR.
/// Imports the call site mentions come first.
pub(super) fn file_options(ctx: &FactCtx, explored: &Explored) -> Vec<Action> {
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
pub(super) fn open_file(
    ctx: &FactCtx,
    path: &str,
    codeword: &str,
    explored: &mut Explored,
) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::super::{Action, Explored, MAX_FILE_OPTIONS, available, fetch, ids};
    use crate::pipeline::pass3::llm_enhance::test_fixtures::{ORIGIN, file, world};
    use models::ir::language::Language;

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
