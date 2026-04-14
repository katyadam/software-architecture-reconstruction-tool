use python_extractor::extraction::parse::extract_syntactic;

use crate::python::utils::load_file;

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
    // FastAPI import must be captured
    assert!(
        record
            .imports
            .iter()
            .any(|i| i.codeword == "FastAPI" || i.codeword == "fastapi"),
        "expected a FastAPI import, got: {:?}",
        record
            .imports
            .iter()
            .map(|i| &i.codeword)
            .collect::<Vec<_>>()
    );
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
    // asyncio and httpx must appear in imports
    assert!(
        record.imports.iter().any(|i| i.codeword == "asyncio"),
        "expected 'asyncio' import, got: {:?}",
        record
            .imports
            .iter()
            .map(|i| &i.codeword)
            .collect::<Vec<_>>()
    );
    assert!(
        record.imports.iter().any(|i| i.codeword == "httpx"),
        "expected 'httpx' import, got: {:?}",
        record
            .imports
            .iter()
            .map(|i| &i.codeword)
            .collect::<Vec<_>>()
    );
}
