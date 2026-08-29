mod endpoints;
mod identify;
mod ir;
mod shared;

use std::collections::HashMap;

use models::{
    api::ExtractionError,
    assignments::Scope,
    ir::{language::Language, project::TypedFileRecord, syntax::FileRecord},
};
use tree_sitter::{Parser, Tree};

pub fn extract_syntactic(text: &str, file_path: &str) -> Result<FileRecord, ExtractionError> {
    let tree = parse_go_tree(text)?;
    let root = tree.root_node();

    let mut callables = Vec::new();
    let mut callable_lookup = HashMap::new();
    let mut call_statements = Vec::new();
    let mut assignments = HashMap::new();

    ir::collect_global_assignments(root, text, &mut assignments);
    ir::collect_callable_ir(
        root,
        text,
        file_path,
        &mut callables,
        &mut callable_lookup,
        &mut call_statements,
        &mut assignments,
    );

    let mut synthetic_callables = Vec::new();
    let endpoints = endpoints::collect_endpoints(
        root,
        text,
        file_path,
        &assignments,
        &callable_lookup,
        &mut synthetic_callables,
    );
    callables.extend(synthetic_callables);

    Ok(FileRecord {
        file_path: file_path.to_string(),
        language: Language::Go,
        imports: vec![],
        entities: vec![],
        endpoints,
        callables,
        call_statements,
        assignments,
        enums: vec![],
        raw_restcalls: vec![],
    })
}

pub fn identify(file: &mut TypedFileRecord) {
    let globals = file
        .assignments
        .iter()
        .filter(|(key, _)| key.scope == Scope::Global)
        .map(|(_, assignment)| (assignment.variable_name.clone(), assignment.value.clone()))
        .collect::<HashMap<_, _>>();
    identify_with_package_globals(file, &globals);
}

pub fn identify_with_package_globals(
    file: &mut TypedFileRecord,
    package_globals: &HashMap<String, String>,
) {
    file.raw_restcalls = file
        .call_statements
        .iter()
        .filter_map(|call| identify::identify_restcall(file, call, Some(package_globals)))
        .collect();
}

fn parse_go_tree(code: &str) -> Result<Tree, ExtractionError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
        .map_err(|err| ExtractionError::Process(format!("failed to load Go grammar: {err}")))?;
    parser
        .parse(code, None)
        .ok_or_else(|| ExtractionError::Process("failed to parse Go source".to_string()))
}

#[cfg(test)]
mod tests {
    use super::{extract_syntactic, identify};

    #[test]
    fn extracts_train_ticket_routes_and_exchange_calls() {
        let code = r#"
const basePath = "/api/v1/stationservice"

func NewRouter() {
    mux.HandleFunc("GET "+basePath+"/stations", handler)
}

func handler() {}

const routeServiceName = "ts-route-service"

func (c *RouteClient) RoutesBetween(start, end string) {
    path := "/api/v1/routeservice/routes/" + url.PathEscape(start) + "/" + url.PathEscape(end)
    _ = c.transport.exchange(ctx, routeServiceName, http.MethodGet, path, nil, &response)
}
"#;

        let record = extract_syntactic(code, "router.go").expect("Go extraction should succeed");
        assert_eq!(record.endpoints.len(), 1);
        assert_eq!(record.endpoints[0].uri, "/api/v1/stationservice/stations");
        assert_eq!(record.call_statements.len(), 4);
        assert!(
            record
                .assignments
                .values()
                .any(|assignment| assignment.variable_name == "path"
                    && assignment.value == "/api/v1/routeservice/routes/{start}/{end}")
        );

        let mut typed = models::ir::project::TypedFileRecord::from(record);
        identify(&mut typed);
        assert_eq!(typed.raw_restcalls.len(), 1);
        assert_eq!(
            typed.raw_restcalls[0].target_uri,
            "http://ts-route-service/api/v1/routeservice/routes/{start}/{end}"
        );
    }

    #[test]
    fn extracts_gorilla_and_direct_http_calls() {
        let code = r#"
func UpdatePaymentStatus() {}

func Router() {
    r.HandleFunc("/payment/{order_id}", UpdatePaymentStatus).Methods("POST")
}

func invoke(url string) {
    req, err := http.NewRequest(http.MethodPost, url+"/ship-order", nil)
    _ = err
    _ = req
}
"#;

        let record = extract_syntactic(code, "router.go").expect("Go extraction should succeed");
        assert_eq!(record.endpoints.len(), 1);
        assert_eq!(record.endpoints[0].uri, "/payment/{order_id}");

        let mut typed = models::ir::project::TypedFileRecord::from(record);
        identify(&mut typed);
        assert_eq!(typed.raw_restcalls.len(), 1);
        assert_eq!(typed.raw_restcalls[0].target_uri, "url/ship-order");
    }

    #[test]
    fn extracts_chi_routes() {
        let code = r#"
package main

import "net/http"

func broker(http.ResponseWriter, *http.Request) {}
func submit(http.ResponseWriter, *http.Request) {}
func dynamic(http.ResponseWriter, *http.Request) {}

func routes() http.Handler {
    mux := chi.NewRouter()
    mux.Post("/", broker)
    mux.Post("/handle", submit)
    mux.Method("DELETE", "/items/{id}", http.HandlerFunc(dynamic))
    return mux
}
"#;

        let record = extract_syntactic(code, "routes.go").expect("Go extraction should succeed");
        assert_eq!(record.endpoints.len(), 3);
        assert!(record.endpoints.iter().any(|e| e.uri == "/" && e.http_method == models::HttpMethod::POST));
        assert!(record.endpoints.iter().any(|e| e.uri == "/handle" && e.http_method == models::HttpMethod::POST));
        assert!(record.endpoints.iter().any(|e| e.uri == "/items/{id}" && e.http_method == models::HttpMethod::DELETE));
    }

    #[test]
    fn extracts_serve_mux_handle_and_client_methods() {
        let code = r#"
package main

import (
    "fmt"
    "net/http"
)

func httpGetProduct() http.HandlerFunc {
    return func(w http.ResponseWriter, r *http.Request) {}
}

func routes() {
    mux := http.NewServeMux()
    mux.Handle("/get-product", httpGetProduct())
}

type RestClient struct {
    restClient *http.Client
    ProductCatalogService string
}

func (c *RestClient) GetProduct(productID string) {
    url := fmt.Sprintf("http://%s/%s?product_id=%s", c.ProductCatalogService, "get-product", productID)
    _, _ = c.restClient.Get(url)
}
"#;

        let record = extract_syntactic(code, "sample.go").expect("Go extraction should succeed");
        assert!(record.endpoints.iter().any(|e| e.uri == "/get-product"));

        let mut typed = models::ir::project::TypedFileRecord::from(record);
        identify(&mut typed);
        assert!(typed.raw_restcalls.iter().any(|call| call.target_uri.contains("/get-product?product_id=")));
    }

    #[test]
    fn extracts_gin_routes() {
        let code = r#"
package main

func checkout() {}

func startRest() {
    router := gin.Default()
    router.POST("/checkout", checkout)
}
"#;

        let record = extract_syntactic(code, "gin.go").expect("Go extraction should succeed");
        assert!(record.endpoints.iter().any(|e| e.uri == "/checkout" && e.http_method == models::HttpMethod::POST));
    }

    #[test]
    fn resolves_service_hosts_from_init_assignments() {
        let code = r#"
package main

import (
    "fmt"
    "net/http"
)

const PRODUCT_CATALOG_SERVICE_ADDR = "PRODUCT_CATALOG_SERVICE_ADDR"

var defaultServiceName = map[string]string{
    PRODUCT_CATALOG_SERVICE_ADDR: "product-catalog-service",
}

type RestClient struct {
    restClient *http.Client
    ProductCatalogService string
}

var client = &RestClient{}

func getService(serviceEnv string, port int) string {
    serviceHost := defaultServiceName[serviceEnv]
    service := fmt.Sprintf("%s:%d", serviceHost, port)
    return service
}

func init() {
    client.ProductCatalogService = getService(PRODUCT_CATALOG_SERVICE_ADDR, 60000)
}

func (c *RestClient) GetProduct(productID string) {
    url := fmt.Sprintf("http://%s/%s?product_id=%s", c.ProductCatalogService, "get-product", productID)
    _, _ = c.restClient.Get(url)
}
"#;

        let record = extract_syntactic(code, "sample.go").expect("Go extraction should succeed");
        let mut typed = models::ir::project::TypedFileRecord::from(record);
        identify(&mut typed);
        let uris = typed
            .raw_restcalls
            .iter()
            .map(|call| call.target_uri.clone())
            .collect::<Vec<_>>();
        assert!(
            uris.iter().any(|uri| {
                uri == "http://product-catalog-service:60000/get-product?product_id=productID"
            }),
            "resolved URIs: {uris:?}"
        );
    }

    #[test]
    fn does_not_treat_regular_delete_method_calls_as_endpoints() {
        let code = r#"
package api

import "net/http"

const basePath = "/api/v1/stationservice"

func NewRouter(stations *Service) http.Handler {
    mux := http.NewServeMux()
    mux.HandleFunc("DELETE "+basePath+"/stations/{stationsId}", func(writer http.ResponseWriter, request *http.Request) {
        _ = stations.Delete(request.Context(), request.PathValue("stationsId"))
    })
    return mux
}
"#;

        let record = extract_syntactic(code, "router.go").expect("Go extraction should succeed");
        assert_eq!(record.endpoints.len(), 1);
        assert_eq!(
            record.endpoints[0].uri,
            "/api/v1/stationservice/stations/{stationsId}"
        );
    }
}
