pub mod imports;
pub mod matcher;
pub mod parser;

/// Tree-sitter node kinds that carry an identifier in the Java grammar.
pub const IDENTIFIER_KINDS: &[&str] = &["identifier", "type_identifier"];

/// Construct the Java tree-sitter `Language` handle. Wrapper so callers
/// outside `statix` do not have to import `tree_sitter_java` directly.
pub fn ts_language() -> tree_sitter::Language {
    tree_sitter_java::LANGUAGE.into()
}
