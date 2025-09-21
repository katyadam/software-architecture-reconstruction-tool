use once_cell::sync::Lazy;
use tree_sitter::{Language, Parser, Tree};

// Lazily-initialized variable, prevents initializing language over and over again, which results in better performance.
static PYTHON_LANGUAGE: Lazy<Language> = Lazy::new(|| tree_sitter_python::LANGUAGE.into());

pub fn get_tree(code: &str) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(&PYTHON_LANGUAGE)
        .expect("Failed to set language");
    parser.parse(code, None).expect("Failed to parse code")
}
