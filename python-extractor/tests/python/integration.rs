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
    assert_eq!(
        record.callables.len(),
        1,
        "empty file still gets the synthetic <module> callable"
    );
    assert_eq!(
        record.callables[0].metadata.name, "<module>()",
        "the sole callable for an empty file must be the synthetic <module>"
    );
    assert!(
        record.callables[0].ast.statements.is_empty(),
        "synthetic <module> for an empty file has no statements"
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
        12,
        "endpoints.py defines 9 endpoint functions + 2 Item2 methods + 1 synthetic <module>"
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

    assert_eq!(
        record.callables.len(),
        8,
        "large_example.py defines 6 client functions + main + 1 synthetic <module> = 8"
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
