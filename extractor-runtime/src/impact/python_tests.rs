use models::python_impact::PythonTestCase;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};
use tree_sitter::{Node, Parser};

/// Discovers pytest-compatible and unittest test functions/methods in a Python project.
///
/// Uses two signals to locate tests, mirroring the Java TIA approach:
///
/// 1. **Candidate scope** — every `.py` file in the project is parsed. Files whose
///    name matches the conventional test patterns (`test_*.py`, `*_test.py`, `tests.py`)
///    or that live under a directory named `tests` or `test` are called *convention
///    candidates*; all others are *content candidates*.
///
/// 2. **Framework detection** — within each candidate file the AST is inspected for
///    import statements and class base-lists (`FileTestContext`) to decide which
///    functions qualify as tests:
///    - Convention candidates: any `test_*` function/method qualifies (backward-
///      compatible with the old filename-only approach).
///    - Content candidates: a function only qualifies when the file `import pytest` /
///      `from pytest import …` (bare `test_*` functions), or when the method belongs
///      to a class that directly or transitively subclasses `TestCase`.
///    - Either way, a `@decorator class TestFoo(…):` is recursed into so its methods
///      are not silently skipped.
pub fn discover_python_tests(project_root: &Path) -> Vec<PythonTestCase> {
    let mut tests = Vec::new();
    collect_tests(project_root, project_root, false, &mut tests);
    tests.sort_by(|a, b| {
        (&a.source_path, &a.qualified_name).cmp(&(&b.source_path, &b.qualified_name))
    });
    tests
}

fn collect_tests(
    project_root: &Path,
    dir: &Path,
    in_test_dir: bool,
    tests: &mut Vec<PythonTestCase>,
) {
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
            let child_in_test_dir = in_test_dir || name == "tests" || name == "test";
            collect_tests(project_root, &path, child_in_test_dir, tests);
        } else if name.ends_with(".py") {
            let convention = in_test_dir || is_test_filename(&name);
            tests.extend(discover_tests_in_file(project_root, &path, convention));
        }
    }
}

/// True when the filename alone marks this as a test file (original convention rules).
fn is_test_filename(name: &str) -> bool {
    name.starts_with("test_") || name.ends_with("_test.py") || name == "tests.py"
}

// ---------------------------------------------------------------------------
// Per-file context derived from a single shallow AST pass over top-level nodes
// ---------------------------------------------------------------------------

struct FileTestContext {
    /// True when the file contains `import pytest` or `from pytest import …`.
    imports_pytest: bool,
    /// True when the file contains `import unittest` or `from unittest import …`.
    imports_unittest: bool,
    /// Simple class names whose base list includes `TestCase`, `unittest.TestCase`,
    /// or a name already in this set (handles intra-file inheritance chains).
    unittest_subclasses: HashSet<String>,
}

impl FileTestContext {
    /// Whether a bare module-level `test_` function qualifies as a test in this file.
    fn module_level_qualifies(&self) -> bool {
        self.imports_pytest || self.imports_unittest
    }

    /// Whether a method of `class_name` qualifies as a test.
    fn method_qualifies(&self, class_name: &str) -> bool {
        self.unittest_subclasses.contains(class_name)
    }
}

/// Performs a single shallow pass over the AST's top-level statements to build
/// import flags and resolve `TestCase` subclass names.
fn scan_file_context(root: Node<'_>, code: &str) -> FileTestContext {
    let mut imports_pytest = false;
    let mut imports_unittest = false;
    // (class_name, [base_name, …]) collected before resolution
    let mut class_bases: Vec<(String, Vec<String>)> = Vec::new();

    let mut cursor = root.walk();
    for node in root.children(&mut cursor) {
        match node.kind() {
            "import_statement" => {
                // `import pytest` / `import unittest` / `import pytest, unittest`
                let mut c = node.walk();
                for child in node.children(&mut c) {
                    match child.kind() {
                        "dotted_name" | "identifier" => {
                            let t = text(child, code);
                            if t == "pytest" { imports_pytest = true; }
                            if t == "unittest" { imports_unittest = true; }
                        }
                        _ => {}
                    }
                }
            }
            "import_from_statement" => {
                // `from pytest import …` / `from unittest import …`
                // The first named child is `dotted_name` or `relative_import` — the module.
                let mut c = node.walk();
                for child in node.children(&mut c) {
                    match child.kind() {
                        "dotted_name" | "relative_import" => {
                            let t = text(child, code);
                            if t == "pytest" || t.starts_with("pytest.") { imports_pytest = true; }
                            if t == "unittest" || t.starts_with("unittest.") { imports_unittest = true; }
                            break;
                        }
                        _ => {}
                    }
                }
            }
            "class_definition" | "decorated_definition" => {
                if let Some(pair) = extract_class_name_and_bases(node, code) {
                    class_bases.push(pair);
                }
            }
            _ => {}
        }
    }

    // Resolve unittest subclasses: iterate until the set stabilises (handles
    // `class B(A)` where `A` is itself a subclass defined earlier in the same file).
    let mut unittest_subclasses: HashSet<String> = HashSet::new();
    loop {
        let before = unittest_subclasses.len();
        for (class_name, bases) in &class_bases {
            if !unittest_subclasses.contains(class_name)
                && bases.iter().any(|b| {
                    b == "TestCase"
                        || b == "unittest.TestCase"
                        || unittest_subclasses.contains(b.as_str())
                })
            {
                unittest_subclasses.insert(class_name.clone());
            }
        }
        if unittest_subclasses.len() == before {
            break;
        }
    }

    FileTestContext { imports_pytest, imports_unittest, unittest_subclasses }
}

/// Returns the simple class name and its base-class names for a `class_definition`
/// or a `decorated_definition` wrapping a `class_definition`.
fn extract_class_name_and_bases(node: Node<'_>, code: &str) -> Option<(String, Vec<String>)> {
    let class_node = if node.kind() == "class_definition" {
        node
    } else {
        let mut c = node.walk();
        node.children(&mut c)
            .find(|n| n.kind() == "class_definition")?
    };
    let name = class_node
        .child_by_field_name("name")
        .map(|n| text(n, code))?;
    let mut bases = Vec::new();
    if let Some(superclasses) = class_node.child_by_field_name("superclasses") {
        let mut c = superclasses.walk();
        for child in superclasses.children(&mut c) {
            match child.kind() {
                "identifier" | "attribute" => {
                    bases.push(text(child, code));
                }
                _ => {}
            }
        }
    }
    Some((name, bases))
}

// ---------------------------------------------------------------------------
// File-level test extraction
// ---------------------------------------------------------------------------

fn discover_tests_in_file(
    project_root: &Path,
    path: &PathBuf,
    convention: bool,
) -> Vec<PythonTestCase> {
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

    let ctx = scan_file_context(tree.root_node(), &code);

    // Content-only files with no test signals can be skipped entirely.
    if !convention
        && !ctx.imports_pytest
        && !ctx.imports_unittest
        && ctx.unittest_subclasses.is_empty()
    {
        return vec![];
    }

    let mut tests = Vec::new();
    collect_from_node(
        tree.root_node(),
        &code,
        &source_path,
        &module_path,
        None,
        &ctx,
        convention,
        &mut tests,
    );
    tests
}

fn collect_from_node(
    node: Node<'_>,
    code: &str,
    source_path: &str,
    module_path: &str,
    enclosing_class: Option<&str>,
    ctx: &FileTestContext,
    convention: bool,
    tests: &mut Vec<PythonTestCase>,
) {
    match node.kind() {
        "class_definition" => {
            let class_name = node
                .child_by_field_name("name")
                .map(|n| text(n, code))
                .unwrap_or_default();
            recurse_class_body(node, code, source_path, module_path, &class_name, ctx, convention, tests);
        }
        "decorated_definition" => {
            // May wrap either a function_definition or a class_definition.
            let mut c = node.walk();
            let inner = node
                .children(&mut c)
                .find(|n| matches!(n.kind(), "function_definition" | "class_definition"));
            match inner.map(|n| n.kind()) {
                Some("function_definition") => {
                    maybe_push_function(
                        inner.unwrap(), code, source_path, module_path,
                        enclosing_class, ctx, convention, tests,
                    );
                }
                Some("class_definition") => {
                    let class_node = inner.unwrap();
                    let class_name = class_node
                        .child_by_field_name("name")
                        .map(|n| text(n, code))
                        .unwrap_or_default();
                    recurse_class_body(
                        class_node, code, source_path, module_path,
                        &class_name, ctx, convention, tests,
                    );
                }
                _ => {}
            }
        }
        "function_definition" => {
            maybe_push_function(
                node, code, source_path, module_path,
                enclosing_class, ctx, convention, tests,
            );
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_from_node(
                    child, code, source_path, module_path,
                    enclosing_class, ctx, convention, tests,
                );
            }
        }
    }
}

fn recurse_class_body(
    class_node: Node<'_>,
    code: &str,
    source_path: &str,
    module_path: &str,
    class_name: &str,
    ctx: &FileTestContext,
    convention: bool,
    tests: &mut Vec<PythonTestCase>,
) {
    if let Some(body) = class_node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            collect_from_node(
                child, code, source_path, module_path,
                Some(class_name), ctx, convention, tests,
            );
        }
    }
}

fn maybe_push_function(
    func_node: Node<'_>,
    code: &str,
    source_path: &str,
    module_path: &str,
    enclosing_class: Option<&str>,
    ctx: &FileTestContext,
    convention: bool,
    tests: &mut Vec<PythonTestCase>,
) {
    let Some(name_node) = func_node.child_by_field_name("name") else {
        return;
    };
    let name = text(name_node, code);
    if !name.starts_with("test") {
        return;
    }

    // A test_ name qualifies when:
    // - convention file (filename / dir rule matched) — always accept
    // - content file — accept if framework evidence matches the call site
    let qualifies = convention
        || match enclosing_class {
            Some(class) => ctx.method_qualifies(class),
            None => ctx.module_level_qualifies(),
        };
    if !qualifies {
        return;
    }

    let qualified_name = match enclosing_class {
        Some(class) => format!("{class}::{name}"),
        None => name.clone(),
    };
    tests.push(PythonTestCase {
        source_path: source_path.to_owned(),
        module_path: module_path.to_owned(),
        qualified_name,
        function_name: name,
        class_name: enclosing_class.map(str::to_owned),
    });
}

fn text(node: Node<'_>, code: &str) -> String {
    code[node.start_byte()..node.end_byte()].to_string()
}
