//! Models and selection logic for change-aware test execution.
//!
//! This is intentionally separate from the architectural Context Map.  The
//! Context Map describes domain entities; this graph describes executable
//! symbols and the tests that may reach them.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
};

use serde::{Deserialize, Serialize};
use tree_sitter::{Language as TsLanguage, Node, Parser};

use crate::{Callable, CodeElementsAggregate};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Method,
    Test,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resolution {
    Exact,
    NameMatch,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactSymbol {
    pub id: String,
    pub name: String,
    pub signature: String,
    pub file_path: String,
    pub body_hash: String,
    pub kind: SymbolKind,
    pub test_selector: Option<String>,
    pub source_range: Option<SourceRange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactEdge {
    pub caller_id: String,
    pub callee_id: Option<String>,
    pub raw_name: String,
    pub resolution: Resolution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestImpactMap {
    pub symbols: Vec<ImpactSymbol>,
    pub edges: Vec<ImpactEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    None,
    CommentsOnly,
    Code,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    pub kind: ChangeKind,
    pub changed_lines: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedTest {
    pub test_id: String,
    pub selector: String,
    pub reasons: Vec<String>,
}

impl TestImpactMap {
    /// Build a conservative graph from the currently exported aggregate.
    /// Existing callable hashes are used as stable IDs; a future range-aware
    /// extractor can populate `source_range` without changing this API.
    pub fn from_aggregate(aggregate: &CodeElementsAggregate) -> Self {
        let mut symbols = Vec::new();
        let mut by_hash = HashMap::new();

        for callable in &aggregate.callables {
            let is_test = is_test_callable(callable);
            let id = symbol_id(callable);
            let kind = if is_test {
                SymbolKind::Test
            } else if callable.namespace.to_string().starts_with("class::") {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            };
            let selector = is_test.then(|| test_selector(callable));
            by_hash.insert(callable.hash.clone(), id.clone());
            symbols.push(ImpactSymbol {
                id,
                name: callable.name.clone(),
                signature: callable.signature.clone(),
                file_path: callable.file_path.clone(),
                body_hash: callable.hash.clone(),
                kind,
                test_selector: selector,
                source_range: None,
            });
        }

        let callable_ids: Vec<(String, String, String)> = symbols
            .iter()
            .map(|s| (s.id.clone(), s.name.clone(), s.signature.clone()))
            .collect();
        let mut edges = Vec::new();
        for call in &aggregate.call_statements {
            let Some(caller_id) = call
                .enclosing_function_hash
                .as_ref()
                .and_then(|hash| by_hash.get(hash))
                .cloned()
            else {
                continue;
            };
            let raw_name = call.function_name.clone();
            let target_name = raw_name
                .split('(')
                .next()
                .unwrap_or(raw_name.as_str())
                .rsplit('.')
                .next()
                .unwrap_or(raw_name.as_str());
            let matches: Vec<&(String, String, String)> = callable_ids
                .iter()
                .filter(|(_, name, signature)| {
                    callable_base_name(name) == target_name
                        || signature.contains(&format!("/{target_name}("))
                })
                .collect();
            let (callee_id, resolution) = match matches.as_slice() {
                [one] => (Some(one.0.clone()), Resolution::NameMatch),
                _ => (None, Resolution::Unresolved),
            };
            edges.push(ImpactEdge {
                caller_id,
                callee_id,
                raw_name,
                resolution,
            });
        }
        populate_source_ranges(&mut symbols);
        Self { symbols, edges }
    }

    /// Select tests reaching any changed symbol.  Unknown or changed files
    /// without symbol-level information conservatively select tests in those
    /// files; callers may choose a broader fallback for unresolved changes.
    pub fn select_tests(
        &self,
        changed_symbol_ids: &HashSet<String>,
        changed_files: &[ChangedFile],
    ) -> Vec<SelectedTest> {
        if changed_files
            .iter()
            .all(|f| f.kind == ChangeKind::CommentsOnly)
            && changed_symbol_ids.is_empty()
        {
            return Vec::new();
        }

        let mut reverse: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in &self.edges {
            if let Some(callee) = &edge.callee_id {
                reverse
                    .entry(callee)
                    .or_default()
                    .push(edge.caller_id.as_str());
            }
        }

        let mut impacted = changed_symbol_ids.clone();
        let mut queue: VecDeque<String> = changed_symbol_ids.iter().cloned().collect();
        while let Some(symbol) = queue.pop_front() {
            for caller in reverse.get(symbol.as_str()).into_iter().flatten() {
                if impacted.insert((*caller).to_string()) {
                    queue.push_back((*caller).to_string());
                }
            }
        }

        let mut result = Vec::new();
        let code_files_without_range_match: HashSet<&str> = changed_files
            .iter()
            .filter(|file| file.kind == ChangeKind::Code)
            .filter(|file| {
                !self.symbols.iter().any(|symbol| {
                    path_matches(&symbol.file_path, &file.path)
                        && symbol.source_range.is_some_and(|range| {
                            file.changed_lines
                                .iter()
                                .any(|line| *line >= range.start.line && *line <= range.end.line)
                        })
                })
            })
            .map(|file| file.path.as_str())
            .collect();
        for symbol in &self.symbols {
            if symbol.kind != SymbolKind::Test {
                continue;
            }
            let file_changed = changed_files.iter().any(|f| {
                path_matches(&symbol.file_path, &f.path)
                    && f.kind == ChangeKind::Code
                    && f.changed_lines.is_empty()
            });
            let range_changed = changed_files.iter().any(|f| {
                path_matches(&symbol.file_path, &f.path)
                    && f.kind == ChangeKind::Code
                    && symbol.source_range.is_some_and(|range| {
                        f.changed_lines
                            .iter()
                            .any(|line| *line >= range.start.line && *line <= range.end.line)
                    })
            });
            let fallback_changed = code_files_without_range_match
                .iter()
                .any(|path| same_parent(path, &symbol.file_path));
            if impacted.contains(&symbol.id) || file_changed || range_changed || fallback_changed {
                result.push(SelectedTest {
                    test_id: symbol.id.clone(),
                    selector: symbol
                        .test_selector
                        .clone()
                        .unwrap_or_else(|| symbol.signature.clone()),
                    reasons: if impacted.contains(&symbol.id) {
                        vec!["reaches a changed symbol".to_string()]
                    } else if fallback_changed {
                        vec!["conservative module fallback for an unresolved change".to_string()]
                    } else {
                        vec!["test file contains executable changes".to_string()]
                    },
                });
            }
        }
        result
    }
}

fn symbol_id(callable: &Callable) -> String {
    format!("{}:{}", callable.file_path, callable.signature)
}

fn normalized_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

fn path_matches(symbol_path: &str, changed_path: &str) -> bool {
    let symbol = normalized_path(symbol_path);
    let changed = normalized_path(changed_path);
    symbol == changed
        || symbol.ends_with(&format!("/{changed}"))
        || changed.ends_with(&format!("/{symbol}"))
}

fn same_parent(left: &str, right: &str) -> bool {
    let left = normalized_path(left);
    let right = normalized_path(right);
    let left_parent = left
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    let right_parent = right
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    left_parent == right_parent
}

fn populate_source_ranges(symbols: &mut [ImpactSymbol]) {
    let mut by_file: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, symbol) in symbols.iter().enumerate() {
        by_file
            .entry(symbol.file_path.clone())
            .or_default()
            .push(index);
    }
    for (path, indices) in by_file {
        let Ok(code) = fs::read_to_string(&path) else {
            continue;
        };
        let Some(language) = language_for_path(&path) else {
            continue;
        };
        let mut parser = Parser::new();
        if parser.set_language(&language).is_err() {
            continue;
        }
        let Some(tree) = parser.parse(&code, None) else {
            continue;
        };
        let mut nodes = Vec::new();
        collect_callable_nodes(tree.root_node(), &mut nodes);
        for index in indices {
            let wanted = callable_base_name(&symbols[index].name);
            let wanted_params = callable_parameter_count(&symbols[index].name);
            let wanted_class = callable_class_name(&symbols[index].signature);
            if let Some(node) = nodes.iter().find(|node| {
                node_name(**node, &code).as_deref() == Some(wanted.as_str())
                    && node_parameter_count(**node) == wanted_params
                    && wanted_class.as_deref() == node_class_name(**node, &code).as_deref()
            }) {
                symbols[index].source_range = Some(range_for_node(*node));
            }
        }
    }
}

fn language_for_path(path: &str) -> Option<TsLanguage> {
    if path.ends_with(".py") {
        Some(tree_sitter_python::LANGUAGE.into())
    } else if path.ends_with(".java") {
        Some(tree_sitter_java::LANGUAGE.into())
    } else {
        None
    }
}

fn collect_callable_nodes<'a>(node: Node<'a>, output: &mut Vec<Node<'a>>) {
    if matches!(
        node.kind(),
        "function_definition" | "method_declaration" | "constructor_declaration"
    ) {
        output.push(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_callable_nodes(child, output);
    }
}

fn node_name(node: Node<'_>, code: &str) -> Option<String> {
    node.child_by_field_name("name")
        .map(|name| code[name.byte_range()].to_string())
}

fn node_parameter_count(node: Node<'_>) -> usize {
    node.child_by_field_name("parameters")
        .map(|parameters| parameters.named_child_count())
        .unwrap_or(0)
}

fn node_class_name(mut node: Node<'_>, code: &str) -> Option<String> {
    while let Some(parent) = node.parent() {
        if matches!(
            parent.kind(),
            "class_definition"
                | "class_declaration"
                | "interface_declaration"
                | "record_declaration"
                | "enum_declaration"
        ) {
            return node_name(parent, code);
        }
        node = parent;
    }
    None
}

fn callable_base_name(name: &str) -> String {
    let before_params = name.split('(').next().unwrap_or(name).trim();
    before_params
        .split_whitespace()
        .last()
        .unwrap_or(before_params)
        .to_string()
}

fn callable_parameter_count(name: &str) -> usize {
    let Some(start) = name.find('(') else {
        return 0;
    };
    let Some(end) = name.rfind(')') else {
        return 0;
    };
    let params = name[start + 1..end].trim();
    if params.is_empty() {
        return 0;
    }

    let mut depth = 0usize;
    let mut count = 1usize;
    for character in params.chars() {
        match character {
            '(' | '[' | '<' => depth += 1,
            ')' | ']' | '>' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => count += 1,
            _ => {}
        }
    }
    count
}

fn callable_class_name(signature: &str) -> Option<String> {
    let namespace = signature.strip_prefix("class:")?.split('/').next()?;
    Some(namespace.to_string())
}

fn range_for_node(node: Node<'_>) -> SourceRange {
    SourceRange {
        start: SourcePosition {
            line: node.start_position().row + 1,
            column: node.start_position().column + 1,
        },
        end: SourcePosition {
            line: node.end_position().row + 1,
            column: node.end_position().column + 1,
        },
    }
}

fn is_test_callable(callable: &Callable) -> bool {
    let file = callable.file_path.replace('\\', "/");
    let name = callable.name.to_ascii_lowercase();
    file.split('/')
        .any(|part| part == "tests" || part == "test")
        || file.rsplit('/').next().is_some_and(|f| {
            f.starts_with("test_") || f.ends_with("_test.py") || f.ends_with("Test.java")
        })
        || name.starts_with("test_")
        || name.contains("test") && callable.namespace.to_string().contains("Test")
}

fn test_selector(callable: &Callable) -> String {
    let file = callable.file_path.replace('\\', "/");
    if file.ends_with(".py") {
        format!(
            "{}::{}",
            file,
            callable.name.split('(').next().unwrap_or(&callable.name)
        )
    } else {
        callable.signature.clone()
    }
}

/// Classify a unified diff conservatively. Added/deleted lines that contain
/// only whitespace or common line comments are treated as non-executable.
pub fn classify_unified_diff(diff: &str) -> Vec<ChangedFile> {
    let mut files: HashMap<String, (ChangeKind, Vec<usize>)> = HashMap::new();
    let mut current: Option<String> = None;
    let mut new_line = 0usize;
    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ b/") {
            current = Some(path.to_string());
            files
                .entry(path.to_string())
                .or_insert((ChangeKind::None, Vec::new()));
            continue;
        }
        if line.starts_with("@@") {
            new_line = parse_new_line(line).unwrap_or(0);
            continue;
        }
        if line.starts_with("--- ") || line.starts_with("diff ") {
            continue;
        }
        let Some(path) = current.as_ref() else {
            continue;
        };
        let Some(content) = line.strip_prefix('+').or_else(|| line.strip_prefix('-')) else {
            continue;
        };
        let comment = content.trim().is_empty() || is_comment_line(content);
        if line.starts_with('+') {
            if comment {
                if files
                    .get(path)
                    .is_some_and(|(kind, _)| *kind == ChangeKind::None)
                {
                    files.get_mut(path).unwrap().0 = ChangeKind::CommentsOnly;
                }
            } else {
                let entry = files.get_mut(path).unwrap();
                entry.0 = ChangeKind::Code;
                entry.1.push(new_line);
            }
            new_line += 1;
        } else if line.starts_with('-') && !comment {
            files.get_mut(path).unwrap().0 = ChangeKind::Code;
        }
    }
    files
        .into_iter()
        .map(|(path, (kind, changed_lines))| ChangedFile {
            path,
            kind,
            changed_lines,
        })
        .collect()
}

fn parse_new_line(header: &str) -> Option<usize> {
    let plus = header.find('+')?;
    header[plus + 1..]
        .split_whitespace()
        .next()?
        .split(',')
        .next()?
        .parse()
        .ok()
}

fn is_comment_line(line: &str) -> bool {
    let s = line.trim();
    s.starts_with('#')
        || s.starts_with("//")
        || s.starts_with("/*")
        || s.starts_with('*')
        || s.starts_with("<!--")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comment_only_diff_selects_nothing() {
        let files = classify_unified_diff("+++ b/src/a.py\n@@\n+# note\n");
        assert_eq!(files[0].kind, ChangeKind::CommentsOnly);
    }

    #[test]
    fn executable_diff_is_code() {
        let files = classify_unified_diff("+++ b/src/a.py\n@@\n+return value\n");
        assert_eq!(files[0].kind, ChangeKind::Code);
    }

    #[test]
    fn code_after_comment_wins() {
        let files = classify_unified_diff("+++ b/src/a.py\n@@\n+# note\n+return value\n");
        assert_eq!(files[0].kind, ChangeKind::Code);
    }

    #[test]
    fn reverse_edges_select_reaching_test() {
        let map = TestImpactMap {
            symbols: vec![
                ImpactSymbol {
                    id: "prod".into(),
                    name: "save()".into(),
                    signature: "module::save()".into(),
                    file_path: "src/service.py".into(),
                    body_hash: "p".into(),
                    kind: SymbolKind::Function,
                    test_selector: None,
                    source_range: None,
                },
                ImpactSymbol {
                    id: "test".into(),
                    name: "test_save()".into(),
                    signature: "module::test_save()".into(),
                    file_path: "tests/test_service.py".into(),
                    body_hash: "t".into(),
                    kind: SymbolKind::Test,
                    test_selector: Some("tests/test_service.py::test_save".into()),
                    source_range: None,
                },
            ],
            edges: vec![ImpactEdge {
                caller_id: "test".into(),
                callee_id: Some("prod".into()),
                raw_name: "save".into(),
                resolution: Resolution::Exact,
            }],
        };
        let selected = map.select_tests(&HashSet::from(["prod".to_string()]), &[]);
        assert_eq!(selected[0].selector, "tests/test_service.py::test_save");
    }

    #[test]
    fn relative_diff_path_matches_absolute_symbol_path() {
        let map = TestImpactMap {
            symbols: vec![ImpactSymbol {
                id: "test".into(),
                name: "test_save()".into(),
                signature: "module::test_save()".into(),
                file_path: "/workspace/project/tests/test_service.py".into(),
                body_hash: "t".into(),
                kind: SymbolKind::Test,
                test_selector: Some("tests/test_service.py::test_save".into()),
                source_range: None,
            }],
            edges: vec![],
        };
        let selected = map.select_tests(
            &HashSet::new(),
            &[ChangedFile {
                path: "tests/test_service.py".into(),
                kind: ChangeKind::Code,
                changed_lines: vec![],
            }],
        );
        assert_eq!(selected.len(), 1);
    }

    #[test]
    fn unresolved_module_change_selects_tests_in_same_directory() {
        let map = TestImpactMap {
            symbols: vec![ImpactSymbol {
                id: "test".into(),
                name: "test_save()".into(),
                signature: "module::test_save()".into(),
                file_path: "src/tests/test_service.py".into(),
                body_hash: "t".into(),
                kind: SymbolKind::Test,
                test_selector: Some("src/tests/test_service.py::test_save".into()),
                source_range: None,
            }],
            edges: vec![],
        };
        let selected = map.select_tests(
            &HashSet::new(),
            &[ChangedFile {
                path: "src/tests/service.py".into(),
                kind: ChangeKind::Code,
                changed_lines: vec![1],
            }],
        );
        assert_eq!(selected.len(), 1);
        assert!(selected[0].reasons[0].contains("fallback"));
    }

    #[test]
    fn callable_matching_uses_parameters_and_namespace() {
        assert_eq!(callable_base_name("void save(Order order)"), "save");
        assert_eq!(
            callable_parameter_count("void save(Order order, int retry)"),
            2
        );
        assert_eq!(
            callable_class_name("class:OrderService/void save(Order)"),
            Some("OrderService".into())
        );
        assert_eq!(callable_class_name("module:service/save(Order)"), None);
    }
}
