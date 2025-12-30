use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("field not found during parsing: {0}")]
    FieldNotFound(String),

    #[error("string is not utf-8: {0}")]
    Utf8Encoding(String),

    #[error("node: {0} is not supported in the tree-sitter concrete syntax tree")]
    UnsupportedNode(String),

    #[error("operator: {0} is not supported in the tree-sitter concrete syntax tree")]
    UnsupportedOperator(String),
}
