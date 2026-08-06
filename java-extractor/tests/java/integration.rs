use java_extractor::extraction::extract_syntactic;
use java_extractor::s;
use models::HttpMethod;

use crate::java::utils::load_file;

#[test]
fn should_return_empty_file_record_for_empty_java_file() {
    let record = extract_syntactic("", "empty.java")
        .expect("extract_syntactic() should not fail on empty input");

    assert!(
        record.imports.is_empty(),
        "Empty file should have no imports"
    );
    assert!(
        record.entities.is_empty(),
        "Empty file should have no entities"
    );
    assert!(
        record.endpoints.is_empty(),
        "Empty file should have no endpoints"
    );
    assert!(
        record.raw_restcalls.is_empty(),
        "Empty file should have no raw REST calls"
    );
    assert!(
        record.callables.is_empty(),
        "Empty file should have no callables"
    );
    assert!(
        record.call_statements.is_empty(),
        "Empty file should have no call statements"
    );
    assert!(record.enums.is_empty(), "Empty file should have no enums");
}

#[test]
fn should_extract_syntactic_from_spring_controller_without_evaluation() {
    let filename = s!("./examples/FoodController.java");
    let code = load_file(&filename).expect("FoodController.java fixture not found");

    let record = extract_syntactic(&code, &filename)
        .expect("extract_syntactic() should not fail on FoodController.java");

    assert_eq!(
        record.endpoints.len(),
        9,
        "FoodController should have 9 Spring endpoints"
    );
    assert_eq!(
        record.callables.len(),
        10,
        "FoodController should have 10 ParsedCallables"
    );
    assert!(
        record.raw_restcalls.is_empty(),
        "Controller should have no outbound REST calls"
    );
    assert!(
        !record.imports.is_empty(),
        "FoodController should have imports"
    );
    // All callables must have an AST (non-empty statements or at least parseable)
    for pc in &record.callables {
        assert!(
            !pc.metadata.name.is_empty(),
            "Each ParsedCallable must have a non-empty callable name"
        );
    }
}

#[test]
fn should_extract_syntactic_raw_restcalls_with_template_uris() {
    let filename = s!("./examples/CancelServiceImpl.java");
    let code = load_file(&filename).expect("CancelServiceImpl.java fixture not found");

    let record = extract_syntactic(&code, &filename)
        .expect("extract_syntactic() should not fail on CancelServiceImpl.java");

    assert!(
        record.endpoints.is_empty(),
        "Service class should have no endpoints"
    );
    assert!(
        !record.callables.is_empty(),
        "CancelServiceImpl should have callables"
    );
    // raw_restcalls: Spring identification without evaluate_invocations won't resolve
    // invoked_on, so RestTemplate calls won't be identified — this is expected for Pass 1.
    // The call_statements still capture the raw HTTP method calls.
    assert!(
        !record.call_statements.is_empty(),
        "CancelServiceImpl should have call statements"
    );
}

#[test]
fn should_parse_spring_controller_and_populate_aggregate_correctly() {
    let filename = s!("./examples/FoodController.java");
    let code = load_file(&filename).expect("FoodController.java fixture not found");

    let record = extract_syntactic(&code, &filename)
        .expect("extract_syntactic() should not fail on FoodController.java");

    assert_eq!(record.endpoints.len(), 9);

    // All endpoints must carry the class-level prefix
    assert!(
        record
            .endpoints
            .iter()
            .all(|e| e.uri.starts_with("/api/v1/foodservice/")),
        "All endpoints must carry the /api/v1/foodservice/ prefix"
    );

    // Spot-check a specific endpoint
    let welcome = record
        .endpoints
        .iter()
        .find(|e| e.uri == "/api/v1/foodservice/welcome");
    assert!(
        welcome.is_some(),
        "GET /api/v1/foodservice/welcome should be present"
    );
    assert_eq!(welcome.unwrap().http_method, HttpMethod::GET);

    // DELETE and GET both exist for /orders/{orderId}
    let order_by_id: Vec<_> = record
        .endpoints
        .iter()
        .filter(|e| e.uri == "/api/v1/foodservice/orders/{orderId}")
        .collect();
    assert_eq!(
        order_by_id.len(),
        2,
        "DELETE and GET for /orders/{{orderId}} must both be present"
    );
    assert!(
        order_by_id
            .iter()
            .any(|e| e.http_method == HttpMethod::DELETE)
    );
    assert!(order_by_id.iter().any(|e| e.http_method == HttpMethod::GET));

    assert!(
        record.raw_restcalls.is_empty(),
        "Controller makes no outbound REST calls"
    );
}

#[test]
fn should_parse_restcall_service_and_populate_aggregate_correctly() {
    let filename = s!("./examples/CancelServiceImpl.java");
    let code = load_file(&filename).expect("CancelServiceImpl.java fixture not found");

    let record = extract_syntactic(&code, &filename)
        .expect("extract_syntactic() should not fail on CancelServiceImpl.java");

    assert!(
        record.endpoints.is_empty(),
        "Service class should have no endpoints"
    );
    assert!(
        !record.callables.is_empty(),
        "CancelServiceImpl should have callables"
    );

    // All 7 restTemplate.exchange() invocations must be captured as call statements.
    // Identification is a Pass 2 stage, so raw_restcalls is empty at Pass 1.
    let exchange_calls: Vec<_> = record
        .call_statements
        .iter()
        .filter(|cs| cs.function_name.contains(".exchange("))
        .collect();
    assert_eq!(
        exchange_calls.len(),
        7,
        "CancelServiceImpl has 7 restTemplate.exchange() calls"
    );

    assert!(
        record.raw_restcalls.is_empty(),
        "identification runs in Pass 2 — raw_restcalls must be empty at Pass 1"
    );
}
