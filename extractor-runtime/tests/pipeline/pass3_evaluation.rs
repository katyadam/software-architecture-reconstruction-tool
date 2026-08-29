use std::collections::HashMap;

use extractor_runtime::pipeline::{build_project_ir, evaluate, pass2::re_identify_restcalls};
use java_extractor::extraction::extract_syntactic as java_extract;
use models::{HttpMethod, RestCall};
use python_extractor::extraction::parse::extract_syntactic as python_extract;
use go_extractor::extraction::extract_syntactic as go_extract;

// ── helpers ───────────────────────────────────────────────────────────────────

/// Minimal Java RestCall for pass 3 testing.
///
/// Java `extract_syntactic` does not run `evaluate_invocations`, so `invoked_on`
/// is never set and `SpringIdentificationStrategy` produces no raw_restcalls.
/// Tests add them manually instead.
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

    let mut record = java_extract(code, "UserClient.java").expect("Java extraction should succeed");
    record
        .raw_restcalls
        .push(java_restcall("void fetchUsers()", "url", "UserClient.java"));

    let project_ir = build_project_ir(vec![record]);
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
    let mut client_record =
        java_extract(client_code, "UserClient.java").expect("UserClient.java should parse");
    client_record
        .raw_restcalls
        .push(java_restcall("void fetchUsers()", "url", "UserClient.java"));

    let project_ir = build_project_ir(vec![base_record, client_record]);
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
    let mut local_record =
        java_extract(local_code, "LocalClient.java").expect("LocalClient.java should parse");
    local_record.raw_restcalls.push(java_restcall(
        "void fetchItems()",
        "url",
        "LocalClient.java",
    ));

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

#[test]
fn go_cross_file_package_restcall_reidentification() {
    let rest_go = r#"
package main

import "fmt"

const PRODUCT_CATALOG_SERVICE_ADDR = "PRODUCT_CATALOG_SERVICE_ADDR"

var defaultServiceName = map[string]string{
    PRODUCT_CATALOG_SERVICE_ADDR: "product-catalog-service",
}

type RestClient struct {
    ProductCatalogService string
}

var client = &RestClient{}

func getService(serviceEnv string, port int) string {
    serviceHost := defaultServiceName[serviceEnv]
    return fmt.Sprintf("%s:%d", serviceHost, port)
}

func init() {
    client.ProductCatalogService = getService(PRODUCT_CATALOG_SERVICE_ADDR, 60000)
}
"#;

    let client_go = r#"
package main

import (
    "fmt"
    "net/http"
)

func (c *RestClient) GetProduct(productID string) {
    url := fmt.Sprintf("http://%s/%s?product_id=%s", c.ProductCatalogService, "get-product", productID)
    _, _ = http.Get(url)
}
"#;

    let rest_record = go_extract(rest_go, "checkoutservice/rest.go").expect("rest.go should parse");
    let client_record =
        go_extract(client_go, "checkoutservice/rest_client.go").expect("rest_client.go should parse");
    let mut typed = vec![rest_record.into(), client_record.into()];

    re_identify_restcalls(&mut typed);

    let client_file = typed
        .iter()
        .find(|file| file.file_path.ends_with("rest_client.go"))
        .expect("rest_client.go should exist");
    let uris = client_file
        .raw_restcalls
        .iter()
        .map(|call| call.target_uri.as_str())
        .collect::<Vec<_>>();

    assert!(
        uris.contains(&"http://product-catalog-service:60000/get-product?product_id=productID"),
        "resolved URIs: {uris:?}"
    );
}

#[test]
fn go_cross_file_receiver_alias_prefers_matching_client_type() {
    let rest_go = r#"
package main

import "fmt"

const CART_SERVICE_ADDR = "CART_SERVICE_ADDR"

var defaultServiceName = map[string]string{
    CART_SERVICE_ADDR: "cart-service",
}

type RestClient struct {
    CartService string
}

type ThriftClient struct {
    CartService string
}

var client = NewRestClient()
var thriftClient = &ThriftClient{}

func NewRestClient() *RestClient {
    return &RestClient{}
}

func getService(serviceEnv string, port int) string {
    serviceHost := defaultServiceName[serviceEnv]
    return fmt.Sprintf("%s:%d", serviceHost, port)
}

func init() {
    client.CartService = getService(CART_SERVICE_ADDR, 60000)
    thriftClient.CartService = getService(CART_SERVICE_ADDR, 50000)
}
"#;

    let rest_record = go_extract(rest_go, "checkoutservice/rest.go").expect("rest.go should parse");
    let client_go = r#"
package main

import (
    "fmt"
    "net/http"
)

func (c *RestClient) GetCart(userID string) {
    url := fmt.Sprintf("http://%s/%s/user_id/%s", c.CartService, "cart", userID)
    request, _ := http.NewRequest("GET", url, nil)
    _, _ = http.DefaultClient.Do(request)
}
"#;
    let client_record =
        go_extract(client_go, "checkoutservice/rest_client.go").expect("rest_client.go should parse");
    let mut typed = vec![rest_record.into(), client_record.into()];

    re_identify_restcalls(&mut typed);

    let client_file = typed
        .iter()
        .find(|file| file.file_path.ends_with("rest_client.go"))
        .expect("rest_client.go should exist");
    let uris = client_file
        .raw_restcalls
        .iter()
        .map(|call| call.target_uri.as_str())
        .collect::<Vec<_>>();

    assert!(
        uris.contains(&"http://cart-service:60000/cart/user_id/userID"),
        "resolved URIs: {uris:?}"
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

/// Files with no raw_restcalls produce no output and do not panic.
///
/// A Python file containing only a helper function (no `requests.*` calls)
/// has an empty `raw_restcalls` list. Pass 3 must short-circuit and produce
/// zero RestCalls rather than attempting symbolic evaluation on an empty set.
#[test]
fn empty_raw_restcalls_produces_no_output() {
    let code = r#"
def helper():
    return "/api/v1"
"#;

    let record = python_extract(code, "helper.py").expect("helper.py should parse");
    // raw_restcalls is empty — no requests.get/post/etc. calls in the file
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
