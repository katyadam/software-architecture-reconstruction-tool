use std::sync::Arc;

use models::{CodeElementsAggregate, api::ExtractionError};
use tokio::task;
use tree_sitter::Parser;

use crate::extraction::{entities::extractor::EntitiesExtractor, extractor::Extractor};
pub mod entities;
pub mod extractor;
pub mod imports;
mod queries;

pub async fn extract(
    code: &str,
    file_name: &str,
) -> Result<CodeElementsAggregate, ExtractionError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .map_err(|_| ExtractionError::Process("Error loading Java Grammar".to_string()))?;

    let tree = parser.parse(code, None).ok_or(ExtractionError::Process(
        "Error parsing code into Concrete Syntax Tree (CST)".to_string(),
    ))?;

    let owned_code = code.to_owned();
    let owned_file_name = file_name.to_owned();

    let tree_arc = Arc::new(tree);
    let code_arc = Arc::new(owned_code);
    let file_name_arc = Arc::new(owned_file_name);

    let entities_handle = task::spawn_blocking({
        let tree = Arc::clone(&tree_arc);
        let code = Arc::clone(&code_arc);
        let file_name = Arc::clone(&file_name_arc);
        move || EntitiesExtractor.extract(&code, &tree, &file_name)
    });

    let mut entities = entities_handle
        .await
        .map_err(|_| ExtractionError::Process("Entities parsing failed".to_string()))?;

    Ok(CodeElementsAggregate::new(
        vec![],
        entities,
        vec![],
        vec![],
        vec![],
        vec![],
    ))
}
