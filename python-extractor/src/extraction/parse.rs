use std::sync::Arc;

use models::CodeElementsAggregate;
use tokio::task;
use tree_sitter::Parser;

use crate::extraction::{
    assignments::map::get_assignments_map,
    callables::extractor::CallablesExtractor,
    calls::extractor::CallsExtractor,
    endpoints::extractor::EndpointsExtractor,
    entities::{evaluator::evaluate_entity_fields, extractor::EntitiesExtractor},
    extractor::{ExtractParams, Extractor},
    imports::extractor::ImportsExtractor,
    restcalls::{evaluator::evaluate_restcalls, extractor::RestcallsExtractor},
};

pub async fn parse(code: &str, file_name: &String, service_name: &String) -> CodeElementsAggregate {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .expect("Error loading Python grammar");

    let tree = parser.parse(code, None).expect("Error parsing code");
    let owned_code = code.to_string(); // TODO: use str instead of String (saving memory)
    let owned_file_name = file_name.clone(); // TODO: use str instead of String (saving memory)
    let owned_service_name = service_name.clone();

    let tree_arc = Arc::new(tree);
    let code_arc = Arc::new(owned_code);
    let file_name = Arc::new(owned_file_name);
    let service_name = Arc::new(owned_service_name);

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
        let service_name = Arc::clone(&service_name);
        move || {
            EndpointsExtractor.extract(
                ExtractParams::new(&Arc::clone(&tree), &Arc::clone(&code))
                    .service_name(&service_name),
            )
        }
    });

    let restcalls_handle = task::spawn_blocking({
        let tree = Arc::clone(&tree_arc);
        let code = Arc::clone(&code_arc);
        let service_name = Arc::clone(&service_name);
        move || {
            RestcallsExtractor.extract(
                ExtractParams::new(&Arc::clone(&tree), &Arc::clone(&code))
                    .service_name(&service_name),
            )
        }
    });

    let entities_handle = task::spawn_blocking({
        let tree = Arc::clone(&tree_arc);
        let code = Arc::clone(&code_arc);
        let file_name = Arc::clone(&file_name);
        let service_name = Arc::clone(&service_name);
        move || {
            EntitiesExtractor.extract(
                ExtractParams::new(&Arc::clone(&tree), &Arc::clone(&code))
                    .file_name(&file_name)
                    .service_name(&service_name),
            )
        }
    });

    let callables_handle = task::spawn_blocking({
        let tree = Arc::clone(&tree_arc);
        let code = Arc::clone(&code_arc);
        let file_name = Arc::clone(&file_name);
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

    let assignments_map = assignments_handle.await.unwrap();
    let imports = imports_handle.await.unwrap();
    let endpoints = endpoints_handle.await.unwrap();
    let mut restcalls = restcalls_handle.await.unwrap();
    let mut entities = entities_handle.await.unwrap();
    let callables = callables_handle.await.unwrap();
    let call_statements = calls_handle.await.unwrap();

    // MAYBE: Evaluate together with extraction?
    evaluate_restcalls(&mut restcalls, assignments_map);
    evaluate_entity_fields(&imports, &mut entities, &file_name);

    CodeElementsAggregate::new(
        imports,
        entities,
        endpoints,
        restcalls,
        callables,
        call_statements,
    )
}
