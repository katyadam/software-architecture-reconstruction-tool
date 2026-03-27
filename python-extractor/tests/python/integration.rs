use python_extractor::extraction::parse::parse;

use crate::python::utils::load_file;

/// Verifies that parsing an empty Python file returns a fully empty `CodeElementsAggregate`
/// with no errors — the pipeline must handle the zero-content edge case gracefully.
#[tokio::test]
async fn should_return_empty_aggregate_for_empty_python_file() {
    let result = parse("", "empty.py").await;
    assert!(result.is_ok(), "parse() must not error on empty input");
    let agg = result.unwrap();
    assert!(agg.imports.is_empty(), "no imports expected for empty file");
    assert!(agg.entities.is_empty(), "no entities expected for empty file");
    assert!(agg.endpoints.is_empty(), "no endpoints expected for empty file");
    assert!(agg.restcalls.is_empty(), "no restcalls expected for empty file");
    assert!(agg.callables.is_empty(), "no callables expected for empty file");
    assert!(agg.call_statements.is_empty(), "no call statements expected for empty file");
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

    assert_eq!(agg.endpoints.len(), 9, "endpoints.py defines exactly 9 FastAPI routes");
    // 9 endpoint functions + Item2.__init__ + Item2.do_something = 11
    assert_eq!(agg.callables.len(), 11, "endpoints.py defines 9 endpoint functions plus 2 Item2 class methods");
    assert!(
        agg.restcalls.is_empty(),
        "endpoints.py contains no outbound HTTP calls, got: {:?}",
        agg.restcalls.iter().map(|r| &r.target_uri).collect::<Vec<_>>()
    );

    // Spot-check: FastAPI imports must be captured
    assert!(
        agg.imports.iter().any(|i| i.codeword == "FastAPI" || i.codeword == "fastapi"),
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
