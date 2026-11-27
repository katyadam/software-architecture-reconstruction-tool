use once_cell::sync::Lazy;
use tree_sitter::{Language, Node, Parser, Tree};

use std::fs;

use crate::error::ParseError;

#[allow(dead_code)]
pub fn load_file(filename: &str) -> Result<String, std::io::Error> {
    fs::read_to_string(filename)
}

/// Deepest syntax tree the Java and Python extractors walk. Their AST walks are
/// recursive; in a debug build they overflow a 2 MiB stack at ~400 levels.
pub const MAX_TREE_DEPTH: usize = 256;

/// True when `tree` nests deeper than `max` levels. Iterative, so it is safe on
/// any input.
pub fn tree_depth_exceeds(tree: &Tree, max: usize) -> bool {
    let mut cursor = tree.walk();
    let mut depth = 1;
    loop {
        if depth > max {
            return true;
        }
        if cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return false;
            }
            depth -= 1;
        }
    }
}

/// Extract the text of a named field child of `node`.
/// Replaces the repeated 4-line pattern:
/// `node.child_by_field_name(field).ok_or(...)?.utf8_text(...)?.to_string()`
pub fn node_field_text(node: Node, field: &str, source: &str) -> Result<String, ParseError> {
    node.child_by_field_name(field)
        .ok_or_else(|| ParseError::FieldNotFound(field.to_string()))?
        .utf8_text(source.as_bytes())
        .map(str::to_string)
        .map_err(|err| ParseError::Utf8Encoding(err.to_string()))
}

/// Extract the text of `node` itself.
pub fn node_text(node: Node, source: &str) -> Result<String, ParseError> {
    node.utf8_text(source.as_bytes())
        .map(str::to_string)
        .map_err(|err| ParseError::Utf8Encoding(err.to_string()))
}

// Lazily-initialized variable, prevents initializing language over and over again, which results in better performance.
static JAVA_LANGUAGE: Lazy<Language> = Lazy::new(|| tree_sitter_java::LANGUAGE.into());

pub fn get_java_tree(code: &str) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(&JAVA_LANGUAGE)
        .expect("Failed to set language");
    parser.parse(code, None).expect("Failed to parse code")
}

// Lazily-initialized variable, prevents initializing language over and over again, which results in better performance.
static PYTHON_LANGUAGE: Lazy<Language> = Lazy::new(|| tree_sitter_python::LANGUAGE.into());

pub fn get_python_tree(code: &str) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(&PYTHON_LANGUAGE)
        .expect("Failed to set language");
    parser.parse(code, None).expect("Failed to parse code")
}

/// Load a Java source file and parse it into a tree in one call.
/// Panics with a clear message if the file cannot be read.
#[allow(dead_code)]
pub fn parse_java_file(filename: &str) -> (String, Tree) {
    let code =
        load_file(filename).unwrap_or_else(|e| panic!("fixture not found: {filename} — {e}"));
    let tree = get_java_tree(&code);
    (code, tree)
}

/// Load a Python source file and parse it into a tree in one call.
/// Panics with a clear message if the file cannot be read.
#[allow(dead_code)]
pub fn parse_python_file(filename: &str) -> (String, Tree) {
    let code =
        load_file(filename).unwrap_or_else(|e| panic!("fixture not found: {filename} — {e}"));
    let tree = get_python_tree(&code);
    (code, tree)
}
