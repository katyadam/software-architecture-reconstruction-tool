use super::inert::is_inert_path;
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
        if is_inert_path(path) {
            continue;
        }
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
            let mapped = if hunk.new_count == 0 {
                match &file.old_source {
                    Some(old_src) => map_deletion_hunk(
                        old_src,
                        path,
                        &file.old_path,
                        hunk.old_start,
                        hunk.old_count,
                    ),
                    None => vec![broad(path, &file.old_path, "deletion hunk: old source unavailable")],
                }
            } else {
                map_hunk(tree.root_node(), &code, path, &file.old_path, hunk)
            };
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
        "decorated_definition" => {
            // Try normal descent first — handles lines inside the function/class body.
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(inner) = find_owner(child, code, line, enclosing.clone()) {
                    return Some(inner);
                }
            }
            // Line is in a decorator (@cache, @property, @app.route …) rather than the
            // body. Attribute it to the decorated function or class so the BFS can trace
            // from it instead of falling back to module/class level.
            node.child_by_field_name("definition")
                .and_then(|def| find_owner(def, code, def.start_position().row, enclosing))
        }
        "class_definition" => {
            let name = node
                .child_by_field_name("name")
                .map(|n| code[n.start_byte()..n.end_byte()].to_string())
                .unwrap_or_default();
            let qualified = if let Some(enc) = &enclosing {
                format!("{enc}.{name}")
            } else {
                name.clone()
            };
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                // Pass the bare class name (not the full qualified path) so that nested
                // method names stay one level deep — matching pyan3's flat node format.
                if let Some(inner) = find_owner(child, code, line, Some(name.clone())) {
                    return Some(inner);
                }
            }
            // Line is inside this class but outside any method (e.g. class variable,
            // docstring, decorator). Return the class name so the BFS can trace from
            // it rather than falling back to module level.
            Some(qualified)
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

/// Maps a deletion hunk by parsing the baseline-side source and attributing
/// the removed lines to their owning callable. This mirrors Go's approach of
/// reading both diff sides so that deleted methods are attributed precisely
/// rather than falling back to module level.
fn map_deletion_hunk(
    old_source: &str,
    path: &str,
    old_path: &Option<String>,
    old_start: u32,
    old_count: u32,
) -> Vec<PythonChangedElement> {
    if old_count == 0 {
        return vec![];
    }
    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .is_err()
    {
        return vec![broad(path, old_path, "Python grammar init failed (old source)")];
    }
    let Some(tree) = parser.parse(old_source, None) else {
        return vec![broad(path, old_path, "Python parse failed (old source)")];
    };
    let start_line = old_start as usize;
    let end_line = (old_start + old_count - 1) as usize;
    let mut owners: Vec<String> = Vec::new();
    for line in start_line..=end_line {
        if let Some(owner) = owning_callable(tree.root_node(), old_source, line.saturating_sub(1)) {
            if !owners.contains(&owner) {
                owners.push(owner);
            }
        } else {
            return vec![broad(path, old_path, "deleted hunk outside any callable")];
        }
    }
    if owners.is_empty() {
        return vec![broad(path, old_path, "empty deletion hunk")];
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

/// A single hunk from a Python file diff.
#[derive(Debug)]
pub struct PythonHunk {
    pub new_start: u32,
    pub new_count: u32,
    pub old_start: u32,
    pub old_count: u32,
}

/// Describes one file's changes in a Python-relevant git diff.
#[derive(Debug)]
pub struct PythonFileDiff {
    pub status: String,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub hunks: Vec<PythonHunk>,
    /// Baseline-side source for this file, populated when any hunk is a pure deletion.
    pub old_source: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(code: &str, line: usize) -> Option<String> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        owning_callable(tree.root_node(), code, line - 1)
    }

    #[test]
    fn method_inside_class() {
        let code = "class MyClass:\n    def my_method(self):\n        pass\n";
        assert_eq!(owner(code, 3).unwrap(), "MyClass.my_method");
    }

    #[test]
    fn class_level_line_returns_class_name() {
        // Line 2 is a class variable — inside MyClass but outside any method.
        let code = "class MyClass:\n    count = 0\n    def method(self): pass\n";
        assert_eq!(
            owner(code, 2).unwrap(),
            "MyClass",
            "class-level line should resolve to the class, not fall back to module"
        );
    }

    #[test]
    fn module_level_function() {
        let code = "def top_level():\n    pass\n";
        assert_eq!(owner(code, 2).unwrap(), "top_level");
    }

    #[test]
    fn module_level_code_returns_none() {
        let code = "import os\nx = 1\n";
        assert!(owner(code, 2).is_none());
    }

    #[test]
    fn nested_class_class_level_line() {
        // The docstring on line 2 is inside Outer but outside any method.
        let code = "class Outer:\n    \"\"\"outer doc\"\"\"\n    class Inner:\n        pass\n";
        assert_eq!(
            owner(code, 2).unwrap(),
            "Outer",
            "class-level line in Outer should resolve to Outer"
        );
    }

    #[test]
    fn nested_class_method() {
        // pyan3 uses flat node names (module.Inner.go), not module.Outer.Inner.go,
        // so method names inside a nested class stay one level deep.
        let code = "class Outer:\n    class Inner:\n        def go(self):\n            pass\n";
        assert_eq!(owner(code, 4).unwrap(), "Inner.go");
    }

    #[test]
    fn decorator_on_method_attributed_to_method() {
        let code = "class MyClass:\n    @property\n    def value(self):\n        return 1\n";
        // Line 2 is the @property decorator — should resolve to the method, not fall back.
        assert_eq!(owner(code, 2).unwrap(), "MyClass.value");
    }

    #[test]
    fn decorator_on_module_level_function() {
        let code = "@cache\ndef compute():\n    pass\n";
        assert_eq!(owner(code, 1).unwrap(), "compute");
    }

    #[test]
    fn decorator_with_arguments() {
        let code = "@app.route(\"/orders\", methods=[\"GET\"])\ndef list_orders():\n    pass\n";
        assert_eq!(owner(code, 1).unwrap(), "list_orders");
    }

    #[test]
    fn deletion_hunk_attributes_to_callable() {
        let old_source = "class PaymentService:\n    def create(self, amount):\n        pass\n";
        let result = map_deletion_hunk(old_source, "services.py", &None, 2, 2);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].target.as_deref(), Some("PaymentService.create"));
        assert_eq!(result[0].kind, PythonChangeKind::Callable);
    }

    #[test]
    fn deletion_hunk_outside_callable_falls_back() {
        let old_source = "import os\nX = 1\n";
        let result = map_deletion_hunk(old_source, "mod.py", &None, 2, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].kind, PythonChangeKind::Module);
    }
}
