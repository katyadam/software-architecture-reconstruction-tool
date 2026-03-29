use java_extractor::extraction::extract;
use java_extractor::s;

use crate::java::utils::load_file;

#[tokio::test]
async fn should_return_empty_aggregate_for_empty_java_file() {
    let agg = extract("", "empty.java")
        .await
        .expect("extract() should not fail on empty input");

    assert!(agg.imports.is_empty(), "Empty file should have no imports");
    assert!(
        agg.entities.is_empty(),
        "Empty file should have no entities"
    );
    assert!(
        agg.endpoints.is_empty(),
        "Empty file should have no endpoints"
    );
    assert!(
        agg.restcalls.is_empty(),
        "Empty file should have no REST calls"
    );
    assert!(
        agg.callables.is_empty(),
        "Empty file should have no callables"
    );
    assert!(
        agg.call_statements.is_empty(),
        "Empty file should have no call statements"
    );
}

#[tokio::test]
async fn should_parse_spring_controller_and_populate_aggregate_correctly() {
    let filename = s!("./examples/FoodController.java");
    let code = load_file(&filename).expect("FoodController.java fixture not found");

    let agg = extract(&code, &filename)
        .await
        .expect("extract() should not fail on FoodController.java");

    assert_eq!(
        agg.endpoints.len(),
        9,
        "FoodController should have 9 Spring endpoints"
    );
    assert_eq!(
        agg.callables.len(),
        10,
        "FoodController should have 10 callables (9 endpoint methods + dontCountThisMethod)"
    );
    assert!(
        agg.restcalls.is_empty(),
        "Controller should have no outbound REST calls"
    );
    assert!(
        !agg.imports.is_empty(),
        "FoodController should have imports"
    );

    // Spot-check: a wildcard import is present (e.g., import org.springframework.web.bind.annotation.*)
    assert!(
        agg.imports.iter().any(|i| i.orig_name == "*"),
        "FoodController should have a wildcard import"
    );
}

#[tokio::test]
async fn should_parse_restcall_service_and_populate_aggregate_correctly() {
    let filename = s!("./examples/CancelServiceImpl.java");
    let code = load_file(&filename).expect("CancelServiceImpl.java fixture not found");

    let agg = extract(&code, &filename)
        .await
        .expect("extract() should not fail on CancelServiceImpl.java");

    assert_eq!(
        agg.restcalls.len(),
        7,
        "CancelServiceImpl should have 7 outbound REST calls"
    );
    assert!(
        agg.endpoints.is_empty(),
        "Service class should have no endpoints"
    );
    assert!(
        !agg.callables.is_empty(),
        "CancelServiceImpl should have callables"
    );
}
