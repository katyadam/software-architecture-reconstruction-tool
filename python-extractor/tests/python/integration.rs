use python_extractor::extraction::parse::{extract_syntactic, parse};

use crate::python::utils::load_file;

/// Verifies that parsing an empty Python file returns a fully empty `CodeElementsAggregate`
/// with no errors — the pipeline must handle the zero-content edge case gracefully.
#[tokio::test]
async fn should_return_empty_aggregate_for_empty_python_file() {
    let result = parse("", "empty.py").await;
    assert!(result.is_ok(), "parse() must not error on empty input");
    let agg = result.unwrap();
    assert!(agg.imports.is_empty(), "no imports expected for empty file");
    assert!(
        agg.entities.is_empty(),
        "no entities expected for empty file"
    );
    assert!(
        agg.endpoints.is_empty(),
        "no endpoints expected for empty file"
    );
    assert!(
        agg.restcalls.is_empty(),
        "no restcalls expected for empty file"
    );
    assert!(
        agg.callables.is_empty(),
        "no callables expected for empty file"
    );
    assert!(
        agg.call_statements.is_empty(),
        "no call statements expected for empty file"
    );
}

/// Verifies that parsing a FastAPI endpoint file populates endpoints and callables
/// while leaving restcalls empty (this file defines server routes, not HTTP clients).
#[tokio::test]
async fn should_parse_endpoint_file_and_populate_aggregate_correctly() {
    let filename = "./examples/python/endpoints.py";
    let code = load_file(filename).expect("fixture not found");
    let result = parse(&code, filename).await;
    assert!(result.is_ok(), "parse() must not error on endpoints.py");
    let agg = result.unwrap();

    assert_eq!(
        agg.endpoints.len(),
        9,
        "endpoints.py defines exactly 9 FastAPI routes"
    );
    // 9 endpoint functions + Item2.__init__ + Item2.do_something = 11
    assert_eq!(
        agg.callables.len(),
        11,
        "endpoints.py defines 9 endpoint functions plus 2 Item2 class methods"
    );
    assert!(
        agg.restcalls.is_empty(),
        "endpoints.py contains no outbound HTTP calls, got: {:?}",
        agg.restcalls
            .iter()
            .map(|r| &r.target_uri)
            .collect::<Vec<_>>()
    );

    // Spot-check: FastAPI imports must be captured
    assert!(
        agg.imports
            .iter()
            .any(|i| i.codeword == "FastAPI" || i.codeword == "fastapi"),
        "expected a FastAPI import, got: {:?}",
        agg.imports.iter().map(|i| &i.codeword).collect::<Vec<_>>()
    );
}

/// Verifies that parsing an HTTP client file populates restcalls and callables correctly
/// while leaving endpoints empty (this file makes outbound calls, not server routes).
#[tokio::test]
async fn should_parse_restcall_file_and_populate_aggregate_correctly() {
    let filename = "./examples/python/restcalls/large_example.py";
    let code = load_file(filename).expect("fixture not found");
    let result = parse(&code, filename).await;
    assert!(result.is_ok(), "parse() must not error on large_example.py");
    let agg = result.unwrap();

    assert_eq!(
        agg.restcalls.len(),
        6,
        "large_example.py has 6 outbound HTTP calls"
    );
    // 6 client functions + `main` = 7 callables
    assert_eq!(
        agg.callables.len(),
        7,
        "large_example.py defines 6 client functions + main = 7 callables"
    );
    assert!(
        agg.endpoints.is_empty(),
        "large_example.py has no @app route decorators, got: {:?}",
        agg.endpoints.iter().map(|e| &e.uri).collect::<Vec<_>>()
    );

    // Spot-check: asyncio and httpx must appear in imports
    assert!(
        agg.imports.iter().any(|i| i.codeword == "asyncio"),
        "expected 'asyncio' import, got: {:?}",
        agg.imports.iter().map(|i| &i.codeword).collect::<Vec<_>>()
    );
    assert!(
        agg.imports.iter().any(|i| i.codeword == "httpx"),
        "expected 'httpx' import, got: {:?}",
        agg.imports.iter().map(|i| &i.codeword).collect::<Vec<_>>()
    );
}

#[test]
fn should_return_empty_file_record_for_empty_python_file() {
    let result = extract_syntactic("", "empty.py");
    assert!(
        result.is_ok(),
        "extract_syntactic() must not error on empty input"
    );
    let record = result.unwrap();
    assert!(
        record.imports.is_empty(),
        "no imports expected for empty file"
    );
    assert!(
        record.entities.is_empty(),
        "no entities expected for empty file"
    );
    assert!(
        record.endpoints.is_empty(),
        "no endpoints expected for empty file"
    );
    assert!(
        record.raw_restcalls.is_empty(),
        "no raw restcalls expected for empty file"
    );
    assert!(
        record.callables.is_empty(),
        "no callables expected for empty file"
    );
    assert!(
        record.call_statements.is_empty(),
        "no call statements expected for empty file"
    );
    assert!(record.enums.is_empty(), "no enums expected for empty file");
}

#[test]
fn should_extract_syntactic_from_endpoint_file_without_evaluation() {
    let filename = "./examples/python/endpoints.py";
    let code = load_file(filename).expect("fixture not found");

    let record = extract_syntactic(&code, filename)
        .expect("extract_syntactic() must not error on endpoints.py");

    assert_eq!(
        record.endpoints.len(),
        9,
        "endpoints.py defines exactly 9 FastAPI routes"
    );
    assert_eq!(
        record.callables.len(),
        11,
        "endpoints.py defines 9 endpoint functions plus 2 Item2 class methods"
    );
    assert!(
        record.raw_restcalls.is_empty(),
        "endpoints.py contains no outbound HTTP calls"
    );
    // All callables must have proper metadata
    for pc in &record.callables {
        assert!(
            !pc.metadata.name.is_empty(),
            "Each ParsedCallable must have a non-empty callable name"
        );
    }
}

#[test]
fn should_extract_syntactic_raw_restcalls_with_template_uris() {
    let filename = "./examples/python/restcalls/large_example.py";
    let code = load_file(filename).expect("fixture not found");

    let record = extract_syntactic(&code, filename)
        .expect("extract_syntactic() must not error on large_example.py");

    // Python identification uses function name suffix — works without evaluate_invocations
    assert_eq!(
        record.raw_restcalls.len(),
        6,
        "large_example.py has 6 outbound HTTP calls identified by method name"
    );
    assert_eq!(
        record.callables.len(),
        7,
        "large_example.py defines 6 client functions + main = 7 callables"
    );
    assert!(
        record.endpoints.is_empty(),
        "large_example.py has no @app route decorators"
    );
    // raw_restcalls should have template URIs (not resolved)
    for rc in &record.raw_restcalls {
        assert!(
            !rc.target_uri.is_empty(),
            "Each raw RestCall must have a non-empty target_uri"
        );
    }
}
