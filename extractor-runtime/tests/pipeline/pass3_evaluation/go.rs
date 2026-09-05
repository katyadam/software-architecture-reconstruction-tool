use std::collections::HashMap;

use extractor_runtime::pipeline::{build_project_ir, evaluate};
use go_extractor::extraction::{extract_syntactic as go_extract, identify_project_restcalls};

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
    let client_record = go_extract(client_go, "checkoutservice/rest_client.go")
        .expect("rest_client.go should parse");
    let mut typed = vec![rest_record.into(), client_record.into()];

    identify_project_restcalls(&mut typed);

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
    let client_record = go_extract(client_go, "checkoutservice/rest_client.go")
        .expect("rest_client.go should parse");
    let mut typed = vec![rest_record.into(), client_record.into()];

    identify_project_restcalls(&mut typed);

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

#[test]
fn go_restcall_fallback_uses_external_env_for_config_selectors() {
    let code = r#"
package main

import (
    "fmt"
    "net/http"
)

type Client struct {
    hostURL string
}

func NewCustomerClient() *Client {
    return &Client{
        hostURL: config.AppConfig.CustomerServiceEndpoint,
    }
}

func (c *Client) GetBasketItems(customerID string) {
    resp, _ := http.Get(c.hostURL + fmt.Sprintf("/customers/%v/basketItems", customerID))
    _ = resp
}
"#;

    let project_ir = build_project_ir(vec![
        go_extract(code, "customer_http_client.go").expect("Go extraction should succeed"),
    ]);
    let mut external_constants = HashMap::new();
    external_constants.insert(
        "CUSTOMER_SERVICE_ENDPOINT".to_string(),
        "http://localhost:8082/api".to_string(),
    );

    let evaluated = evaluate(
        project_ir,
        &external_constants,
        &HashMap::new(),
        &HashMap::new(),
    );

    assert!(
        evaluated.restcalls.iter().any(|restcall| {
            restcall.target_uri == "http://localhost:8082/api/customers/customerID/basketItems"
        }),
        "resolved URIs: {:?}",
        evaluated
            .restcalls
            .iter()
            .map(|restcall| restcall.target_uri.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn go_cross_file_endpoint_handler_resolves_to_real_callable() {
    let routes_go = r#"
package api

func routes() {
    router := chi.NewRouter()
    router.Get("/items", listItems)
}
"#;
    let handlers_go = r#"
package api

import "net/http"

func listItems(writer http.ResponseWriter, request *http.Request) {
    _, _ = http.Get("http://inventory-service/items")
}
"#;

    let routes_record =
        go_extract(routes_go, "service/api/routes.go").expect("routes.go should parse");
    let handlers_record =
        go_extract(handlers_go, "service/api/handlers.go").expect("handlers.go should parse");
    let handler = handlers_record
        .callables
        .iter()
        .find(|callable| callable.metadata.name == "listItems")
        .expect("listItems callable should exist")
        .metadata
        .clone();

    let project = build_project_ir(vec![routes_record, handlers_record]);
    let routes_file = project
        .files
        .iter()
        .find(|file| file.file_path.ends_with("routes.go"))
        .expect("routes.go should exist");
    let endpoint = routes_file
        .endpoints
        .iter()
        .find(|endpoint| endpoint.uri == "/items")
        .expect("GET /items endpoint should exist");

    assert_eq!(endpoint.function_hash, handler.hash);
    assert_eq!(endpoint.function_name, handler.signature);
    assert!(
        routes_file
            .callables
            .iter()
            .all(|callable| !callable.metadata.signature.starts_with("handler ")),
        "resolved synthetic handlers should be removed"
    );
}
