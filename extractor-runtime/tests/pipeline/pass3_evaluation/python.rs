use std::collections::HashMap;

use extractor_runtime::pipeline::{build_project_ir, evaluate};
use python_extractor::extraction::parse::extract_syntactic as python_extract;

#[test]
fn python_single_file_restcall_with_enum() {
    let code = r#"
class Status(Enum):
    ACTIVE = "active"
    INACTIVE = "inactive"

def fetch_by_status(status: Status):
    url = f"/api/items/{status.value}"
    requests.get(url)
"#;

    let record = python_extract(code, "client.py").expect("Python extraction should succeed");

    let project_ir = build_project_ir(vec![record]);
    let evaluated = evaluate(
        project_ir,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );

    assert_eq!(
        evaluated.restcalls.len(),
        2,
        "one restcall per enum variant expected"
    );

    let uris: Vec<&str> = evaluated
        .restcalls
        .iter()
        .map(|r| r.target_uri.as_str())
        .collect();
    assert!(
        uris.contains(&"/api/items/active"),
        "should contain /api/items/active, got: {uris:?}"
    );
    assert!(
        uris.contains(&"/api/items/inactive"),
        "should contain /api/items/inactive, got: {uris:?}"
    );
}

#[test]
fn python_cross_file_constant_injection_single_quotes() {
    let constants_code = r#"
BASE_URL = '/api/v1'
"#;

    let client_code = r#"
def fetch_items():
    url = BASE_URL + "/items"
    requests.get(url)
"#;

    let constants_record =
        python_extract(constants_code, "constants.py").expect("constants.py should parse");
    let client_record = python_extract(client_code, "client.py").expect("client.py should parse");

    let project_ir = build_project_ir(vec![constants_record, client_record]);
    let evaluated = evaluate(
        project_ir,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "/api/v1/items",
        "single-quoted constant should be stripped and resolved identically"
    );
}

#[test]
fn python_cross_file_constant_injection() {
    let constants_code = r#"
BASE_URL = "/api/v1"
"#;

    let client_code = r#"
def fetch_items():
    url = BASE_URL + "/items"
    requests.get(url)
"#;

    let constants_record =
        python_extract(constants_code, "constants.py").expect("constants.py should parse");
    let client_record = python_extract(client_code, "client.py").expect("client.py should parse");

    let project_ir = build_project_ir(vec![constants_record, client_record]);
    let evaluated = evaluate(
        project_ir,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "/api/v1/items",
        "URI should resolve via cross-file constant injection"
    );
}

#[test]
fn empty_raw_restcalls_produces_no_output() {
    let code = r#"
def helper():
    return "/api/v1"
"#;

    let record = python_extract(code, "helper.py").expect("helper.py should parse");
    assert!(
        record.raw_restcalls.is_empty(),
        "helper.py should have no raw_restcalls, got: {:?}",
        record.raw_restcalls
    );

    let project_ir = build_project_ir(vec![record]);
    let evaluated = evaluate(
        project_ir,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );

    assert_eq!(
        evaluated.restcalls.len(),
        0,
        "no restcalls expected from a file with no raw_restcalls"
    );
}

#[test]
fn callable_collision_local_priority() {
    let global_code = r#"
def get_prefix():
    return "/global"
"#;

    let local_code = r#"
def get_prefix():
    return "/local"

def fetch():
    url = get_prefix()
    requests.get(url)
"#;

    let global_record =
        python_extract(global_code, "global_service.py").expect("global_service.py should parse");
    let local_record =
        python_extract(local_code, "local_service.py").expect("local_service.py should parse");

    let project_ir = build_project_ir(vec![global_record, local_record]);
    let evaluated = evaluate(
        project_ir,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "/local",
        "local callable definition must take priority over global"
    );
}
