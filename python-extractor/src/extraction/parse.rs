use std::sync::Arc;

use models::{CodeElementsAggregate, api::ExtractionError};
use tokio::task;
use tree_sitter::Parser;

use crate::extraction::{
    assignments::map::get_assignments_map,
    callables::extractor::CallablesExtractor,
    calls::{evaluator::evaluate_invocations, extractor::CallsExtractor},
    endpoints::extractor::EndpointsExtractor,
    entities::{evaluator::evaluate_entity_fields, extractor::EntitiesExtractor},
    extractor::{ExtractParams, Extractor},
    imports::extractor::ImportsExtractor,
    restcalls::{evaluator::evaluate_restcalls, extractor::RestcallsExtractor},
};

pub async fn parse(code: &str, file_name: &str) -> Result<CodeElementsAggregate, ExtractionError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|_| ExtractionError::Process("Error loading Python Grammar".to_string()))?;

    let tree = parser
        .parse(code, None)
        .ok_or(ExtractionError::Process("Error parsing code".to_string()))?;

    let owned_code = code.to_owned();
    let owned_file_name = file_name.to_owned();

    let tree_arc = Arc::new(tree);
    let code_arc = Arc::new(owned_code);
    let file_name_arc = Arc::new(owned_file_name);

    // Running parsing function in parallel
    let assignments_handle = task::spawn_blocking({
        let tree = Arc::clone(&tree_arc);
        let code = Arc::clone(&code_arc);
        move || get_assignments_map(&tree, &code)
    });

    let imports_handle = task::spawn_blocking({
        let tree = Arc::clone(&tree_arc);
        let code = Arc::clone(&code_arc);
        move || ImportsExtractor.extract(ExtractParams::new(&Arc::clone(&tree), &Arc::clone(&code)))
    });

    let endpoints_handle = task::spawn_blocking({
        let tree = Arc::clone(&tree_arc);
        let code = Arc::clone(&code_arc);
        let file_name = Arc::clone(&file_name_arc);
        move || {
            EndpointsExtractor.extract(
                ExtractParams::new(&Arc::clone(&tree), &Arc::clone(&code)).file_name(&file_name),
            )
        }
    });

    let restcalls_handle = task::spawn_blocking({
        let tree = Arc::clone(&tree_arc);
        let code = Arc::clone(&code_arc);
        let file_name = Arc::clone(&file_name_arc);
        move || {
            RestcallsExtractor.extract(
                ExtractParams::new(&Arc::clone(&tree), &Arc::clone(&code)).file_name(&file_name),
            )
        }
    });

    let entities_handle = task::spawn_blocking({
        let tree = Arc::clone(&tree_arc);
        let code = Arc::clone(&code_arc);
        let file_name = Arc::clone(&file_name_arc);
        move || {
            EntitiesExtractor.extract(
                ExtractParams::new(&Arc::clone(&tree), &Arc::clone(&code)).file_name(&file_name),
            )
        }
    });

    let callables_handle = task::spawn_blocking({
        let tree = Arc::clone(&tree_arc);
        let code = Arc::clone(&code_arc);
        let file_name = Arc::clone(&file_name_arc);
        move || {
            CallablesExtractor.extract(
                ExtractParams::new(&Arc::clone(&tree), &Arc::clone(&code)).file_name(&file_name),
            )
        }
    });

    let calls_handle = task::spawn_blocking({
        let tree = Arc::clone(&tree_arc);
        let code = Arc::clone(&code_arc);
        move || CallsExtractor.extract(ExtractParams::new(&Arc::clone(&tree), &Arc::clone(&code)))
    });

    let assignments_map = assignments_handle
        .await
        .map_err(|_| ExtractionError::Process("Assignments parsing failed".to_string()))?;
    let imports = imports_handle
        .await
        .map_err(|_| ExtractionError::Process("Imports parsing failed".to_string()))?;
    let endpoints = endpoints_handle
        .await
        .map_err(|_| ExtractionError::Process("Endpoints parsing failed".to_string()))?;
    let mut restcalls = restcalls_handle
        .await
        .map_err(|_| ExtractionError::Process("REST calls parsing failed".to_string()))?;
    let mut entities = entities_handle
        .await
        .map_err(|_| ExtractionError::Process("Entities parsing failed".to_string()))?;
    let callables = callables_handle
        .await
        .map_err(|_| ExtractionError::Process("Callables parsing failed".to_string()))?;
    let mut call_statements = calls_handle
        .await
        .map_err(|_| ExtractionError::Process("Call Statements parsing failed".to_string()))?;

    // MAYBE: Evaluate together with extraction?
    evaluate_restcalls(&mut restcalls, &assignments_map);
    evaluate_entity_fields(&imports, &mut entities, file_name);
    evaluate_invocations(&mut call_statements, &assignments_map);

    Ok(CodeElementsAggregate::new(
        imports,
        entities,
        endpoints,
        restcalls,
        callables,
        call_statements,
    ))
}
