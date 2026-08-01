use models::{
    ParsedCallable,
    api::ExtractionError,
    ir::{language::Language, syntax::FileRecord},
};
use statix::parse_java;
use tree_sitter::Parser;

use crate::extraction::{
    assignments::map::get_assignments_map,
    callables::extractor::CallablesExtractor,
    calls::extractor::CallStatementsExtractor,
    endpoints::extractor::EndpointsExtractor,
    entities::extractor::EntitiesExtractor,
    extractor::Extractor,
    imports::extractor::ImportsExtractor,
    restcalls::identification::{
        spring::SpringIdentificationStrategy, strategy::IdentificationStrategy,
    },
};
pub mod assignments;
pub mod callables;
pub mod calls;
mod enclosing_lookup;
pub mod endpoints;
pub mod entities;
pub mod extractor;
pub mod grpc;
pub mod imports;
mod queries;
pub mod restcalls;

pub fn extract_syntactic(code: &str, file_name: &str) -> Result<FileRecord, ExtractionError> {
    use statix::java::matcher::java_convert_full_header_to_mangled_name;

    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .map_err(|_| ExtractionError::Process("Error loading Java grammar".into()))?;

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
    let entities =
        entities.ok_or_else(|| ExtractionError::Process("Entities extraction failed".into()))?;
    let callables =
        callables.ok_or_else(|| ExtractionError::Process("Callables extraction failed".into()))?;
    let calls = calls.ok_or_else(|| ExtractionError::Process("Calls extraction failed".into()))?;

    // Build ParsedCallable list: combine rich Callable metadata with parsed ASTs
    let mut parsed_callables_map = parse_java(&tree, code);
    let parsed_callables: Vec<ParsedCallable> = callables
        .into_iter()
        .filter_map(|callable| {
            let mangled = java_convert_full_header_to_mangled_name(&callable.name);
            let ast = parsed_callables_map.remove(&mangled)?.ast;
            Some(ParsedCallable {
                metadata: callable,
                ast,
            })
        })
        .collect();

    // Identification-only: no symbolic evaluation or URI resolution
    let identification_strategy = SpringIdentificationStrategy::new();
    let raw_restcalls: Vec<models::RestCall> = calls
        .iter()
        .filter_map(|call| identification_strategy.identify_restcall(call, file_name))
        .collect();
    let (grpc_endpoints, grpc_calls) = grpc::extract(code, &tree, file_name, &calls);
    let mut endpoints = endpoints;
    endpoints.extend(grpc_endpoints);
    let mut raw_restcalls = raw_restcalls;
    raw_restcalls.extend(grpc_calls);

    Ok(FileRecord {
        file_path: file_name.to_string(),
        language: Language::Java,
        imports,
        entities,
        endpoints,
        callables: parsed_callables,
        call_statements: calls,
        assignments,
        enums: vec![],
        raw_restcalls,
    })
}
