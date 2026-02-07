use models::{CodeElementsAggregate, api::ExtractionError};
use statix::parse_java;
use tree_sitter::Parser;

use crate::extraction::{
    assignments::map::get_assignments_map,
    callables::extractor::CallablesExtractor,
    calls::{evaluator::evaluate_invocations, extractor::CallStatementsExtractor},
    endpoints::extractor::EndpointsExtractor,
    entities::{evaluator::evaluate_entity_fields, extractor::EntitiesExtractor},
    extractor::Extractor,
    imports::extractor::ImportsExtractor,
    restcalls::{
        evaluation::spring::SpringEvaluationStrategy,
        identification::spring::SpringIdentificationStrategy,
        selection::{selector::Selector, spring::SpringSelector},
    },
};
pub mod assignments;
pub mod callables;
pub mod calls;
mod enclosing_lookup;
pub mod endpoints;
pub mod entities;
pub mod extractor;
pub mod imports;
mod queries;
pub mod restcalls;

pub async fn extract(
    code: &str,
    file_name: &str,
) -> Result<CodeElementsAggregate, ExtractionError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .map_err(|_| ExtractionError::Process("Error loading Python grammar".into()))?;

    let tree = parser
        .parse(code, None)
        .ok_or_else(|| ExtractionError::Process("Error parsing code".into()))?;

    let mut assignments = None;
    let mut imports = None;
    let mut endpoints = None;
    let mut entities = None;
    let mut callables = None;
    let mut calls = None;

    rayon::scope(|s| {
        s.spawn(|_| assignments = Some(get_assignments_map(&tree, code)));
        s.spawn(|_| imports = Some(ImportsExtractor.extract(code, &tree, file_name)));
        s.spawn(|_| endpoints = Some(EndpointsExtractor.extract(code, &tree, file_name)));
        s.spawn(|_| entities = Some(EntitiesExtractor.extract(code, &tree, file_name)));
        s.spawn(|_| callables = Some(CallablesExtractor.extract(code, &tree, file_name)));
        s.spawn(|_| calls = Some(CallStatementsExtractor.extract(code, &tree, file_name)));
    });

    let assignments = assignments
        .ok_or_else(|| ExtractionError::Process("Assignments extraction failed".into()))?;

    let imports =
        imports.ok_or_else(|| ExtractionError::Process("Imports extraction failed".into()))?;

    let endpoints =
        endpoints.ok_or_else(|| ExtractionError::Process("Endpoints extraction failed".into()))?;

    let mut entities =
        entities.ok_or_else(|| ExtractionError::Process("Entities extraction failed".into()))?;

    let callables =
        callables.ok_or_else(|| ExtractionError::Process("Callables extraction failed".into()))?;

    let mut calls =
        calls.ok_or_else(|| ExtractionError::Process("Calls extraction failed".into()))?;

    // Post-processing / evaluation
    evaluate_entity_fields(&imports, &mut entities);
    evaluate_invocations(&mut calls, &assignments);
    let methods_asts = parse_java(&tree, code);

    let restcalls = SpringSelector::new(
        SpringIdentificationStrategy::new(),
        SpringEvaluationStrategy::new(methods_asts),
    )
    .select_restcall_statements(&calls, file_name)
    .map_err(|e| {
        ExtractionError::SymbolicEvaluation(format!("Restcall evaluation error: {:?}", e))
    })?;

    Ok(CodeElementsAggregate::new(
        imports, entities, endpoints, restcalls, callables, calls,
    ))
}
