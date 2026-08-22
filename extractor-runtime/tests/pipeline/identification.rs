use extractor_runtime::pipeline::build_project_ir;
use go_extractor::extraction::extract_syntactic as go_extract;
use java_extractor::extraction::extract_syntactic as java_extract;
use python_extractor::extraction::parse::extract_syntactic as python_extract;

/// Java: Spring identification needs `invoked_on`, which only Pass 2 resolves.
/// Pass 1 must produce nothing; Pass 2 must produce the call.
#[test]
fn java_spring_restcall_is_identified_in_pass2() {
    let code = r#"
package com.example;

import org.springframework.web.client.RestTemplate;

public class OrderClient {
    private RestTemplate restTemplate = new RestTemplate();

    public String fetch() {
        return restTemplate.exchange("http://inventory/items", HttpMethod.GET, null, String.class);
    }
}
"#;

    let record = java_extract(code, "OrderClient.java").expect("Java extraction should succeed");
    let ir = build_project_ir(vec![record]);

    let restcalls = &ir.files[0].raw_restcalls;
    assert_eq!(
        restcalls.len(),
        1,
        "Pass 2 must identify the restTemplate.exchange call, got: {restcalls:?}"
    );
    // Java's identify_target_uri returns the raw argument value, so the source
    // quotes are still attached — match on the substring, not the exact string.
    assert!(
        restcalls[0].target_uri.contains("http://inventory/items"),
        "unexpected target_uri: {}",
        restcalls[0].target_uri
    );
}

/// Python: identification moved out of Pass 1, so Pass 2 must now produce it.
#[test]
fn python_restcall_is_identified_in_pass2() {
    let code = r#"
import requests

def fetch():
    return requests.get("http://inventory/items")
"#;

    let record = python_extract(code, "client.py").expect("Python extraction should succeed");
    let ir = build_project_ir(vec![record]);

    let restcalls = &ir.files[0].raw_restcalls;
    assert_eq!(
        restcalls.len(),
        1,
        "Pass 2 must identify the requests.get call, got: {restcalls:?}"
    );
    assert_eq!(restcalls[0].target_uri, "http://inventory/items");
}

/// The decorator exclusion must survive the move to Pass 2. This is the one
/// place where behaviour could silently drift.
///
/// The decorated function must stay nested inside `add_routes`. At module level
/// the decorator call has no enclosing function or class, and
/// `MethodCallIdentificationStrategy` rejects it on that ground alone -- the
/// test would then pass even with the `is_decorator` guard deleted, proving
/// nothing. Nesting gives the call an `enclosing_function_name`, so
/// `is_decorator` is the only thing standing between it and identification.
#[test]
fn python_fastapi_route_decorator_is_not_a_restcall() {
    let code = r#"
from fastapi import FastAPI

app = FastAPI()

def add_routes(app):
    @app.get("/items")
    def read_items():
        return []
"#;

    let record = python_extract(code, "api.py").expect("Python extraction should succeed");
    let ir = build_project_ir(vec![record]);

    assert!(
        ir.files[0].raw_restcalls.is_empty(),
        "@app.get(\"/items\") declares a route, it is not an outbound call: {:?}",
        ir.files[0].raw_restcalls
    );
}

/// Python message edges move to Pass 2 alongside REST calls.
#[test]
fn python_message_edges_are_identified_in_pass2() {
    let code = r#"
class Publisher:
    def publish(self, message):
        self.channel.basic_publish(exchange="orders", routing_key="created", body=message)
"#;

    let record = python_extract(code, "publisher.py").expect("Python extraction should succeed");
    let ir = build_project_ir(vec![record]);

    assert_eq!(
        ir.files[0].raw_message_edges.len(),
        1,
        "Pass 2 must identify the basic_publish call, got: {:?}",
        ir.files[0].raw_message_edges
    );
}

#[test]
fn go_train_ticket_restcall_is_identified_in_pass2() {
    let code = r#"
package clients

import (
    "net/http"
    "net/url"
)

const routeServiceName = "ts-route-service"

func (c *RouteClient) RoutesBetween(start, end string) {
    path := "/api/v1/routeservice/routes/" + url.PathEscape(start) + "/" + url.PathEscape(end)
    _ = c.transport.exchange(ctx, routeServiceName, http.MethodGet, path, nil, &response)
}
"#;

    let record = go_extract(code, "route_client.go").expect("Go extraction should succeed");
    let ir = build_project_ir(vec![record]);

    let restcalls = &ir.files[0].raw_restcalls;
    assert_eq!(
        restcalls.len(),
        1,
        "Pass 2 must identify the transport.exchange call, got: {restcalls:?}"
    );
    assert_eq!(
        restcalls[0].target_uri,
        "http://ts-route-service/api/v1/routeservice/routes/{start}/{end}"
    );
}

#[test]
fn go_endpoint_is_extracted_from_net_http_handlefunc() {
    let code = r#"
package api

import "net/http"

const basePath = "/api/v1/stationservice"

func NewRouter() http.Handler {
    mux := http.NewServeMux()
    mux.HandleFunc("GET "+basePath+"/stations", handler)
    return mux
}
"#;

    let record = go_extract(code, "router.go").expect("Go extraction should succeed");
    assert_eq!(record.endpoints.len(), 1);
    assert_eq!(record.endpoints[0].uri, "/api/v1/stationservice/stations");
}
