use std::collections::HashMap;

use extractor_runtime::pipeline::pass1::dispatch_syntactic;
use extractor_runtime::pipeline::{build_project_ir, evaluate};
use java_extractor::extraction::extract_syntactic as java_extract;
use models::{HttpMethod, MessageDestinationKind, RestCall};
use python_extractor::extraction::parse::extract_syntactic as python_extract;

// ── helpers ───────────────────────────────────────────────────────────────────

/// Minimal Java RestCall for pass 3 testing.
///
/// These tests exercise hand-written Java that has no real Spring/RestTemplate
/// shape for Pass 2 to identify, so they add the RestCall by hand onto the
/// `TypedFileRecord` after `build_project_ir` has run, to target Pass 3's
/// symbolic evaluation in isolation.
fn java_restcall(function_name: &str, target_uri: &str, file_path: &str) -> RestCall {
    RestCall {
        function_name: function_name.to_string(),
        function_hash: String::new(),
        call_arguments: vec![],
        http_method: HttpMethod::GET,
        target_uri: target_uri.to_string(),
        file_path: file_path.to_string(),
    }
}

#[test]
fn proto_contract_filters_undeclared_grpc_operations() {
    let client = python_extract(
        r#"
import document_service_pb2_grpc
class Client:
    def __init__(self, channel):
        self.stub = document_service_pb2_grpc.DocumentServiceStub(channel)
    async def load(self, request):
        return await self.stub.GetDocument(request)
    async def invalid(self, request):
        return await self.stub.DoesNotExist(request)
"#,
        "client.py",
    )
    .expect("Python extraction should succeed");
    let contract = dispatch_syntactic(
        "service DocumentService { rpc GetDocument (Request) returns (Document); }",
        "document.proto",
    )
    .unwrap()
    .expect("protobuf extraction should succeed");

    let evaluated = evaluate(
        build_project_ir(vec![client, contract]),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );
    assert_eq!(evaluated.restcalls.len(), 1);
    assert_eq!(
        evaluated.restcalls[0].target_uri,
        "grpc://DocumentService/GetDocument"
    );
}

/// Python: an exchange that resolves to an empty value uses queue semantics.
#[test]
fn python_empty_resolved_exchange_is_a_queue() {
    let code = r#"
EXCHANGE = ""

class Publisher:
    def publish(self, message):
        self.channel.basic_publish(
            exchange=EXCHANGE,
            routing_key="orders",
            body=message,
        )
"#;

    let record = python_extract(code, "publisher.py").expect("Python extraction should succeed");
    let evaluated = evaluate(
        build_project_ir(vec![record]),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );
    let edge = evaluated
        .message_edges
        .iter()
        .find(|edge| edge.role == models::MessageRole::Producer)
        .expect("producer edge should be present");
    assert_eq!(edge.destination_kind, MessageDestinationKind::Queue);
    assert_eq!(edge.destination, "orders");
}

// ── Java tests ────────────────────────────────────────────────────────────────

/// Java: single-file symbolic evaluation resolves a helper method call.
///
/// `fetchUsers()` declares `url = getBaseUrl()`, and `getBaseUrl()` returns
/// `"/api/users"`. After pass 3, the RestCall target_uri should be resolved
/// from the variable name "url" to the literal "/api/users".
#[test]
fn java_single_file_restcall_evaluation() {
    let code = r#"
class UserClient {
    String getBaseUrl() {
        return "/api/users";
    }
    void fetchUsers() {
        String url = getBaseUrl();
    }
}
"#;

    let record = java_extract(code, "UserClient.java").expect("Java extraction should succeed");

    let mut project_ir = build_project_ir(vec![record]);
    project_ir.files[0].raw_restcalls.push(java_restcall(
        "void fetchUsers()",
        "url",
        "UserClient.java",
    ));
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
        evaluated.restcalls[0].target_uri, "/api/users",
        "URI should resolve via single-file helper method"
    );
}

/// Java: cross-file evaluation resolves a callable defined in a separate file.
///
/// `getBaseUrl()` lives in `BaseService.java` and returns `"/api"`.
/// `fetchUsers()` in `UserClient.java` calls it and stores the result in `url`.
/// The global callable merge must make `getBaseUrl` visible during evaluation
/// of `fetchUsers`.
#[test]
fn java_cross_file_restcall_evaluation() {
    let base_code = r#"
class BaseService {
    String getBaseUrl() {
        return "/api";
    }
}
"#;

    let client_code = r#"
class UserClient {
    void fetchUsers() {
        String url = getBaseUrl();
    }
}
"#;

    let base_record =
        java_extract(base_code, "BaseService.java").expect("BaseService.java should parse");
    let client_record =
        java_extract(client_code, "UserClient.java").expect("UserClient.java should parse");

    let mut project_ir = build_project_ir(vec![base_record, client_record]);
    project_ir.files[1].raw_restcalls.push(java_restcall(
        "void fetchUsers()",
        "url",
        "UserClient.java",
    ));
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
        evaluated.restcalls[0].target_uri, "/api",
        "URI should resolve via cross-file helper method"
    );
}

/// Java: local callable takes priority over an identically-named global callable.
///
/// `GlobalService.java` defines `getBaseUrl()` returning `"/global"`.
/// `LocalClient.java` also defines `getBaseUrl()` returning `"/local"` and a `fetchItems()`
/// that calls it. Pass 3 must use the local definition so the resolved URI is `"/local"`.
#[test]
fn java_callable_collision_local_priority() {
    let global_code = r#"
class GlobalService {
    String getBaseUrl() {
        return "/global";
    }
}
"#;

    let local_code = r#"
class LocalClient {
    String getBaseUrl() {
        return "/local";
    }
    void fetchItems() {
        String url = getBaseUrl();
    }
}
"#;

    let global_record =
        java_extract(global_code, "GlobalService.java").expect("GlobalService.java should parse");
    let local_record =
        java_extract(local_code, "LocalClient.java").expect("LocalClient.java should parse");

    let mut project_ir = build_project_ir(vec![global_record, local_record]);
    project_ir.files[1].raw_restcalls.push(java_restcall(
        "void fetchItems()",
        "url",
        "LocalClient.java",
    ));
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

// ── Python tests ──────────────────────────────────────────────────────────────

/// Python: f-string REST call with an enum parameter expands to one URI per variant.
///
/// `Status` has two variants: `"active"` and `"inactive"`.
/// `fetch_by_status(status: Status)` calls `requests.get(url)` where
/// `url = f"/api/items/{status.value}"`. Pass 3 must produce two RestCalls,
/// one for each variant.
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

/// Same as `python_cross_file_constant_injection` but the constant uses single
/// quotes (`BASE_URL = '/api/v1'`). Both quote styles must be stripped identically.
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

/// Python: cross-file constant injection resolves BASE_URL defined in another file.
///
/// File A declares `BASE_URL = "/api/v1"` at module scope.
/// File B's `fetch_items()` builds `url = BASE_URL + "/items"` then calls
/// `requests.get(url)`. Pass 3 injects `BASE_URL` from the constants map so
/// the concat evaluates to `"/api/v1/items"`.
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

/// Files with no identifiable REST calls produce no output and do not panic.
///
/// A Python file containing only a helper function (no `requests.*` calls)
/// has nothing for Pass 2 to identify. Pass 3 must short-circuit and produce
/// zero RestCalls rather than attempting symbolic evaluation on an empty set.
#[test]
fn empty_raw_restcalls_produces_no_output() {
    let code = r#"
def helper():
    return "/api/v1"
"#;

    let record = python_extract(code, "helper.py").expect("helper.py should parse");

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

/// Python: local callable takes priority over an identically-named global callable.
///
/// `global_service.py` defines `get_prefix()` returning `"/global"`.
/// `local_service.py` defines `get_prefix()` returning `"/local"` and a `fetch()`
/// function that calls it. Pass 3 must use the local definition so the resolved
/// URI is `"/local"`, not `"/global"`.
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
