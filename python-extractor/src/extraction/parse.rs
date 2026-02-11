use models::{CallStatement, CodeElementsAggregate, api::ExtractionError};
use statix::parse_python;
use tree_sitter::Parser;

use crate::extraction::{
    assignments::map::get_assignments_map,
    callables::extractor::CallablesExtractor,
    calls::{PythonCallStatement, evaluator::evaluate_invocations, extractor::CallsExtractor},
    endpoints::extractor::EndpointsExtractor,
    entities::{evaluator::evaluate_entity_fields, extractor::EntitiesExtractor},
    extractor::{ExtractParams, Extractor},
    imports::extractor::ImportsExtractor,
    restcalls::{
        evaluation::method_call::MethodCallEvaluationStrategy,
        extractor::RestcallsExtractor,
        identification::method_call::MethodCallIdentificationStrategy,
        selection::{method_call::MethodCallSelector, selector::Selector},
    },
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

    let mut entities =
        entities.ok_or_else(|| ExtractionError::Process("Entities extraction failed".into()))?;

    let callables =
        callables.ok_or_else(|| ExtractionError::Process("Callables extraction failed".into()))?;

    let mut calls =
        calls.ok_or_else(|| ExtractionError::Process("Calls extraction failed".into()))?;

    // Post-processing / evaluation
    evaluate_entity_fields(&imports, &mut entities, file_name);
    evaluate_invocations(&mut calls, &assignments);
    let function_asts = parse_python(&tree, code);

    let restcalls = MethodCallSelector::new(
        MethodCallIdentificationStrategy::new(),
        MethodCallEvaluationStrategy::new(function_asts),
    )
    .select_restcall_statements(&calls, file_name)
    .map_err(|e| {
        ExtractionError::SymbolicEvaluation(format!("Restcall evaluation error: {:?}", e))
    })?;

    let unified_calls = calls
        .into_iter()
        .map(PythonCallStatement::to_language_agnostic)
        .collect::<Vec<CallStatement>>();

    Ok(CodeElementsAggregate::new(
        imports,
        entities,
        endpoints,
        restcalls,
        callables,
        unified_calls,
    ))
}
