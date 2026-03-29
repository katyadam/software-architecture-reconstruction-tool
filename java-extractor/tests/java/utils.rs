use once_cell::sync::Lazy;
use tree_sitter::{Language, Parser, Tree};

use std::fs;

#[allow(dead_code)]
pub fn load_file(filename: &str) -> Result<String, std::io::Error> {
    fs::read_to_string(filename)
}

// Lazily-initialized variable, prevents initializing language over and over again, which results in better performance.
static JAVA_LANGUAGE: Lazy<Language> = Lazy::new(|| tree_sitter_java::LANGUAGE.into());

pub fn get_tree(code: &str) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(&JAVA_LANGUAGE)
        .expect("Failed to set language");
    parser.parse(code, None).expect("Failed to parse code")
}

#[allow(dead_code)]
pub fn parse_file(filename: &str) -> (String, Tree) {
    let code =
        load_file(filename).unwrap_or_else(|e| panic!("fixture not found: {filename} — {e}"));
    let tree = get_tree(&code);
    (code, tree)
}
