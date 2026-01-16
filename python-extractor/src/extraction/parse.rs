use models::{CodeElementsAggregate, api::ExtractionError};
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
        .map_err(|_| ExtractionError::Process("Error loading Python grammar".into()))?;

    let tree = parser
        .parse(code, None)
        .ok_or_else(|| ExtractionError::Process("Error parsing code".into()))?;

    let params = ExtractParams::new(&tree, code).file_name(file_name);

    let mut assignments = None;
    let mut imports = None;
    let mut endpoints = None;
    let mut restcalls = None;
    let mut entities = None;
    let mut callables = None;
    let mut calls = None;

    rayon::scope(|s| {
        s.spawn(|_| assignments = Some(get_assignments_map(&tree, code)));
        s.spawn(|_| imports = Some(ImportsExtractor.extract(params)));
        s.spawn(|_| endpoints = Some(EndpointsExtractor.extract(params)));
        s.spawn(|_| restcalls = Some(RestcallsExtractor.extract(params)));
        s.spawn(|_| entities = Some(EntitiesExtractor.extract(params)));
        s.spawn(|_| callables = Some(CallablesExtractor.extract(params)));
        s.spawn(|_| calls = Some(CallsExtractor.extract(ExtractParams::new(&tree, code))));
    });

    let assignments = assignments
        .ok_or_else(|| ExtractionError::Process("Assignments extraction failed".into()))?;

    let imports =
        imports.ok_or_else(|| ExtractionError::Process("Imports extraction failed".into()))?;

    let endpoints =
        endpoints.ok_or_else(|| ExtractionError::Process("Endpoints extraction failed".into()))?;

    let restcalls =
        restcalls.ok_or_else(|| ExtractionError::Process("REST calls extraction failed".into()))?;

    let entities =
        entities.ok_or_else(|| ExtractionError::Process("Entities extraction failed".into()))?;

    let callables =
        callables.ok_or_else(|| ExtractionError::Process("Callables extraction failed".into()))?;

    let calls = calls.ok_or_else(|| ExtractionError::Process("Calls extraction failed".into()))?;

    let mut aggregate =
        CodeElementsAggregate::new(imports, entities, endpoints, restcalls, callables, calls);

    // Post-processing / evaluation
    evaluate_restcalls(&mut aggregate.restcalls, &assignments);
    evaluate_entity_fields(&aggregate.imports, &mut aggregate.entities, file_name);
    evaluate_invocations(&mut aggregate.call_statements, &assignments);

    Ok(aggregate)
}
