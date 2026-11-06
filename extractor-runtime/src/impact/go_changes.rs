use super::go_git::*;
use models::go_impact::*;

pub fn map_go_changes(
    diff: &GoGitDiff,
    inventory: &GoInventoryOutcome,
    descriptions: &GoDescriptionOutcome,
) -> Vec<GoChangedElement> {
    let mut out = vec![];
    for f in &diff.files {
        let path = f
            .new_path
            .as_ref()
            .or(f.old_path.as_ref())
            .expect("Git status path");
        let broad = |why: &str| GoChangedElement {
            source_path: path.clone(),
            old_path: f.old_path.clone(),
            kind: GoChangeKind::Module,
            target: None,
            diagnostics: vec![why.into()],
        };
        if f.status != "M" || !path.ends_with(".go") || f.hunks.is_empty() {
            out.push(broad(
                "file/declaration/configuration change requires module fallback",
            ));
            continue;
        }
        let old = descriptions
            .files
            .iter()
            .find(|d| d.source_path == format!("baseline:{path}"));
        let new = descriptions
            .files
            .iter()
            .find(|d| d.source_path == format!("candidate:{path}"));
        let (Some(old), Some(new)) = (old, new) else {
            out.push(broad("source description unavailable"));
            continue;
        };
        if descriptions.status != GoStageStatus::Ok
            || !old.diagnostics.is_empty()
            || !new.diagnostics.is_empty()
            || old.package_name != new.package_name
        {
            out.push(broad("invalid or changed source package"));
            continue;
        }
        let own = |ds: &[GoSourceDeclaration],
                   start: u32,
                   count: u32|
         -> Option<GoSourceDeclaration> {
            if count == 0 {
                return None;
            }
            let end = start.checked_add(count - 1)?;
            let matches: Vec<_> = ds
                .iter()
                .filter(|d| {
                    d.body_start_line > 0 && start >= d.body_start_line && end <= d.body_end_line
                })
                .collect();
            (matches.len() == 1).then(|| matches[0].clone())
        };
        for h in &f.hunks {
            let a = own(&old.declarations, h.old_start, h.old_count);
            let b = own(&new.declarations, h.new_start, h.new_count);
            let candidate = match (a, b) {
                (Some(a), Some(b)) if a.header == b.header => Some(b),
                (Some(a), None) if h.new_count == 0 => {
                    let found: Vec<_> = new
                        .declarations
                        .iter()
                        .filter(|d| d.header == a.header)
                        .collect();
                    (found.len() == 1).then(|| found[0].clone())
                }
                (None, Some(b)) if h.old_count == 0 => {
                    let count = old
                        .declarations
                        .iter()
                        .filter(|d| d.header == b.header)
                        .count();
                    (count == 1).then_some(b)
                }
                _ => None,
            };
            let Some(c) = candidate else {
                let v = broad("unmapped range, signature, global, import, or declaration change");
                if !out.contains(&v) {
                    out.push(v)
                }
                continue;
            };
            let native: Vec<_> = inventory
                .declarations
                .iter()
                .filter(|d| {
                    d.syntax == c
                        && d.method
                            .source_path
                            .as_ref()
                            .is_some_and(|p| std::path::Path::new(p) == diff.module_root.join(path))
                })
                .collect();
            let value = if native.len() == 1 && c.member_name != "init" {
                let m = &native[0].method;
                GoChangedElement {
                    source_path: path.clone(),
                    old_path: f.old_path.clone(),
                    kind: GoChangeKind::Callable,
                    target: Some(format!("{}#{}", m.declaring_type, m.member_name)),
                    diagnostics: vec![],
                }
            } else {
                broad("source owner has no unique native target")
            };
            if !out.contains(&value) {
                out.push(value)
            }
        }
    }
    out.sort_by(|a, b| (&a.source_path, &a.target).cmp(&(&b.source_path, &b.target)));
    out
}
