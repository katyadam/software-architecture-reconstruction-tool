#[macro_use]
pub mod macros;
pub mod extraction;
pub mod parsing;

use extraction::{callables::extractor::CallablesExtractor, extractor::Extractor};
use models::{Callable, api::ExtractionError};
use tree_sitter::Parser;

/// Extracts Java callable metadata without requiring the method body to support symbolic parsing.
pub fn extract_syntactic_callables(
    code: &str,
    file_name: &str,
) -> Result<Vec<Callable>, ExtractionError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .map_err(|_| ExtractionError::Process("Error loading Java grammar".into()))?;
    let tree = parser
        .parse(code, None)
        .ok_or_else(|| ExtractionError::Process("Error parsing code".into()))?;
    Ok(CallablesExtractor.extract(code, &tree, file_name))
}
