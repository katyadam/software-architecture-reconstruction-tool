use models::python_impact::{PythonChangeKind, PythonChangedElement};
use std::{fs, path::Path};
use tree_sitter::{Node, Parser};

/// Maps one diff hunk line range in a Python source file to its owning callable (if any).
pub fn map_python_changes(
    project_root: &Path,
    files: &[PythonFileDiff],
) -> Vec<PythonChangedElement> {
    let mut out = Vec::new();
    for file in files {
        let path = file
            .new_path
            .as_ref()
            .or(file.old_path.as_ref())
            .expect("diff path");
        // Non-Python files, renames, and deletions fall back to module-level.
        if file.status != "M" || !path.ends_with(".py") || file.hunks.is_empty() {
            out.push(broad(path, &file.old_path, "file/rename/delete change requires module fallback"));
            continue;
        }
        let full = project_root.join(path);
        let code = match fs::read_to_string(&full) {
            Ok(c) => c,
            Err(e) => {
                out.push(broad(path, &file.old_path, &format!("cannot read source: {e}")));
                continue;
            }
        };
        let mut parser = Parser::new();
        if parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .is_err()
        {
            out.push(broad(path, &file.old_path, "Python grammar init failed"));
            continue;
        }
        let Some(tree) = parser.parse(&code, None) else {
            out.push(broad(path, &file.old_path, "Python parse failed"));
            continue;
        };
        for hunk in &file.hunks {
            let mapped = map_hunk(tree.root_node(), &code, path, &file.old_path, hunk);
            for elem in mapped {
                if !out.contains(&elem) {
                    out.push(elem);
                }
            }
        }
    }
    out.sort_by(|a, b| (&a.source_path, &a.target).cmp(&(&b.source_path, &b.target)));
    out
}

fn broad(path: &str, old_path: &Option<String>, why: &str) -> PythonChangedElement {
    PythonChangedElement {
        source_path: path.to_owned(),
        old_path: old_path.clone(),
        kind: PythonChangeKind::Module,
        target: None,
        diagnostics: vec![why.into()],
    }
}

fn map_hunk(
    root: Node<'_>,
    code: &str,
    path: &str,
    old_path: &Option<String>,
    hunk: &PythonHunk,
) -> Vec<PythonChangedElement> {
    if hunk.new_count == 0 {
        return vec![broad(path, old_path, "deletion hunk requires module fallback")];
    }
    let start_line = hunk.new_start as usize;
    let end_line = (hunk.new_start + hunk.new_count - 1) as usize;
    let mut owners: Vec<String> = Vec::new();
    for line in start_line..=end_line {
        if let Some(owner) = owning_callable(root, code, line.saturating_sub(1)) {
            if !owners.contains(&owner) {
                owners.push(owner);
            }
        } else {
            // Line has no owning callable → fall back to module level for this hunk.
            return vec![broad(path, old_path, "hunk outside any callable")];
        }
    }
    if owners.is_empty() {
        return vec![broad(path, old_path, "empty hunk")];
    }
    owners
        .into_iter()
        .map(|target| PythonChangedElement {
            source_path: path.to_owned(),
            old_path: old_path.clone(),
            kind: PythonChangeKind::Callable,
            target: Some(target),
            diagnostics: vec![],
        })
        .collect()
}

/// Returns the dotted qualified name of the innermost function/method that contains
/// the given 0-based line index, or `None` if the line is outside any callable.
fn owning_callable(root: Node<'_>, code: &str, line: usize) -> Option<String> {
    find_owner(root, code, line, None)
}

fn find_owner(node: Node<'_>, code: &str, line: usize, enclosing: Option<String>) -> Option<String> {
    let start = node.start_position().row;
    let end = node.end_position().row;
    if line < start || line > end {
        return None;
    }
    match node.kind() {
        "function_definition" => {
            let name = node
                .child_by_field_name("name")
                .map(|n| code[n.start_byte()..n.end_byte()].to_string())
                .unwrap_or_default();
            let qualified = if let Some(enc) = &enclosing {
                format!("{enc}.{name}")
            } else {
                name
            };
            // Try to narrow into a nested function.
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(inner) = find_owner(child, code, line, Some(qualified.clone())) {
                    return Some(inner);
                }
            }
            Some(qualified)
        }
        "class_definition" => {
            let name = node
                .child_by_field_name("name")
                .map(|n| code[n.start_byte()..n.end_byte()].to_string())
                .unwrap_or_default();
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(inner) = find_owner(child, code, line, Some(name.clone())) {
                    return Some(inner);
                }
            }
            None
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(inner) = find_owner(child, code, line, enclosing.clone()) {
                    return Some(inner);
                }
            }
            None
        }
    }
}

/// A single hunk from a Python file diff.
#[derive(Debug)]
pub struct PythonHunk {
    pub new_start: u32,
    pub new_count: u32,
}

/// Describes one file's changes in a Python-relevant git diff.
#[derive(Debug)]
pub struct PythonFileDiff {
    pub status: String,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub hunks: Vec<PythonHunk>,
}
