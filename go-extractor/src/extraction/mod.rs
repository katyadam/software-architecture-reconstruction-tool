mod endpoints;
mod identify;
mod ir;
mod shared;

use std::collections::HashMap;

use models::{
    api::ExtractionError,
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
    file.raw_restcalls = file
        .call_statements
        .iter()
        .filter_map(|call| identify::identify_restcall(file, call))
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
