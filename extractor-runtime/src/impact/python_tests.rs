use models::python_impact::PythonTestCase;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tree_sitter::{Node, Parser};

/// Discovers pytest-compatible and unittest test functions/methods in a Python project.
///
/// Scans for files matching `test_*.py` or `*_test.py` under the project root and
/// identifies functions/methods whose name starts with `test_`.
pub fn discover_python_tests(project_root: &Path) -> Vec<PythonTestCase> {
    let mut tests = Vec::new();
    collect_tests(project_root, project_root, &mut tests);
    tests.sort_by(|a, b| (&a.source_path, &a.qualified_name).cmp(&(&b.source_path, &b.qualified_name)));
    tests
}

fn collect_tests(project_root: &Path, dir: &Path, tests: &mut Vec<PythonTestCase>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue,
        };
        if path.is_dir() {
            if name.starts_with('.') || name == "__pycache__" || name == ".git" {
                continue;
            }
            collect_tests(project_root, &path, tests);
        } else if is_test_file(&name) {
            tests.extend(discover_tests_in_file(project_root, &path));
        }
    }
}

fn is_test_file(name: &str) -> bool {
    name.ends_with(".py") && (name.starts_with("test_") || name.ends_with("_test.py"))
}

fn discover_tests_in_file(project_root: &Path, path: &PathBuf) -> Vec<PythonTestCase> {
    let code = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let source_path = path
        .strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let module_path = source_path
        .trim_end_matches(".py")
        .replace('/', ".")
        .trim_start_matches('.')
        .to_string();

    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .is_err()
    {
        return vec![];
    }
    let Some(tree) = parser.parse(&code, None) else {
        return vec![];
    };

    extract_test_cases(tree.root_node(), &code, &source_path, &module_path)
}

fn extract_test_cases(
    node: Node<'_>,
    code: &str,
    source_path: &str,
    module_path: &str,
) -> Vec<PythonTestCase> {
    let mut tests = Vec::new();
    collect_from_node(node, code, source_path, module_path, None, &mut tests);
    tests
}

fn collect_from_node(
    node: Node<'_>,
    code: &str,
    source_path: &str,
    module_path: &str,
    enclosing_class: Option<&str>,
    tests: &mut Vec<PythonTestCase>,
) {
    let kind = node.kind();
    match kind {
        "class_definition" => {
            let class_name = node
                .child_by_field_name("name")
                .map(|n| text(n, code))
                .unwrap_or_default();
            let body = node.child_by_field_name("body");
            if let Some(body) = body {
                let mut cursor = body.walk();
                for child in body.children(&mut cursor) {
                    collect_from_node(
                        child,
                        code,
                        source_path,
                        module_path,
                        Some(&class_name),
                        tests,
                    );
                }
            }
        }
        "function_definition" | "decorated_definition" => {
            let func_node = if kind == "decorated_definition" {
                find_function_in_decorated(node)
            } else {
                Some(node)
            };
            if let Some(func) = func_node {
                if let Some(name_node) = func.child_by_field_name("name") {
                    let name = text(name_node, code);
                    if name.starts_with("test") {
                        let qualified_name = if let Some(class) = enclosing_class {
                            format!("{class}::{name}")
                        } else {
                            name.clone()
                        };
                        tests.push(PythonTestCase {
                            source_path: source_path.to_owned(),
                            module_path: module_path.to_owned(),
                            qualified_name,
                            function_name: name,
                            class_name: enclosing_class.map(str::to_owned),
                        });
                    }
                }
            }
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_from_node(child, code, source_path, module_path, enclosing_class, tests);
            }
        }
    }
}

fn find_function_in_decorated(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_definition" {
            return Some(child);
        }
    }
    None
}

fn text(node: Node<'_>, code: &str) -> String {
    code[node.start_byte()..node.end_byte()].to_string()
}
