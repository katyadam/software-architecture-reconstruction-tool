use models::{
    CallStatement, ParsedCallable,
    api::ExtractionError,
    ir::{ast::CallableAst, language::Language, syntax::FileRecord},
};
use statix::parse_python;
use tree_sitter::Parser;

use crate::extraction::{
    assignments::map::get_assignments_map,
    callables::extractor::CallablesExtractor,
    calls::{PythonCallStatement, extractor::CallsExtractor},
    endpoints::extractor::EndpointsExtractor,
    entities::extractor::EntitiesExtractor,
    enums::identification::EnumIdentificator,
    extractor::{ExtractParams, Extractor},
    imports::extractor::ImportsExtractor,
    restcalls::identification::{
        method_call::MethodCallIdentificationStrategy, strategy::IdentificationStrategy,
    },
};

pub fn extract_syntactic(code: &str, file_name: &str) -> Result<FileRecord, ExtractionError> {
    use statix::python::matcher::python_convert_full_header_to_mangled_name;

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
    let mut entities = None;
    let mut callables = None;
    let mut calls = None;

    rayon::scope(|s| {
        s.spawn(|_| assignments = Some(get_assignments_map(&tree, code)));
        s.spawn(|_| imports = Some(ImportsExtractor.extract(params)));
        s.spawn(|_| endpoints = Some(EndpointsExtractor.extract(params)));
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
    let entities =
        entities.ok_or_else(|| ExtractionError::Process("Entities extraction failed".into()))?;
    let callables =
        callables.ok_or_else(|| ExtractionError::Process("Callables extraction failed".into()))?;
    let calls = calls.ok_or_else(|| ExtractionError::Process("Calls extraction failed".into()))?;

    let enums = EnumIdentificator::identify_from_entities(&entities);

    // Build ParsedCallable list: combine rich Callable metadata with parsed ASTs.
    // Fall back to an empty AST when no match is found — this preserves callable
    // metadata (hash, signature) needed by the IMCG builder for functions whose
    // mangled name can't be matched (e.g. nested `_` handlers with inline comments
    // in the parameter list that cause parse_python and parse_parameters to produce
    // different mangled keys).
    let mut parsed_callables_map = parse_python(&tree, code);
    let parsed_callables: Vec<ParsedCallable> = callables
        .into_iter()
        .map(|callable| {
            let full_header = format!(
                "{} -> {}",
                callable.name,
                callable.return_type.as_deref().unwrap_or("Any")
            );
            let mangled = python_convert_full_header_to_mangled_name(&full_header);
            let ast = parsed_callables_map
                .remove(&mangled)
                .map(|pc| pc.ast)
                .unwrap_or_else(|| CallableAst { statements: vec![] });
            ParsedCallable {
                metadata: callable,
                ast,
            }
        })
        .collect();

    // Identification-only: no symbolic evaluation or URI resolution
    let identification_strategy = MethodCallIdentificationStrategy::new();
    let raw_restcalls = calls
        .iter()
        .filter_map(|call| identification_strategy.identify_restcall(call, file_name))
        .collect();

    let call_statements = calls
        .into_iter()
        .map(PythonCallStatement::to_language_agnostic)
        .collect::<Vec<CallStatement>>();

    Ok(FileRecord {
        file_path: file_name.to_string(),
        language: Language::Python,
        imports,
        entities,
        endpoints,
        callables: parsed_callables,
        call_statements,
        assignments,
        enums,
        raw_restcalls,
    })
}
