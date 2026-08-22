use std::{collections::{HashMap, HashSet}, str::FromStr};

use models::{
    Assignment, AssignmentKey, Callable, Endpoint, HttpMethod, Namespace, ParsedCallable,
    RestCall, Scope,
    api::ExtractionError,
    ir::{
        ast::CallableAst,
        language::Language,
        project::TypedFileRecord,
        syntax::FileRecord,
    },
};
use once_cell::sync::Lazy;
use regex::Regex;
use statix::strings::hash_text;

static SIMPLE_CONST_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*const\s+([A-Za-z_]\w*)\s*(?:[A-Za-z_][\w\[\]\*]*)?\s*=\s*("([^"\\]|\\.)*")\s*$"#)
        .expect("valid const regex")
});
static CONST_BLOCK_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?s)const\s*\((.*?)\)"#).expect("valid const block regex"));
static BLOCK_CONST_ENTRY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?m)^\s*([A-Za-z_]\w*)\s*(?:[A-Za-z_][\w\[\]\*]*)?\s*=\s*("([^"\\]|\\.)*")\s*$"#)
        .expect("valid const block entry regex")
});
static FUNC_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"func\s*(?:\(([^)]*)\)\s*)?([A-Za-z_]\w*)\s*\(([^)]*)\)\s*(?:\([^)]*\)|[\w\.\*\[\]]+)?\s*\{"#,
    )
    .expect("valid function regex")
});
static FUNC_NAME_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"func\s*(?:\([^)]*\)\s*)?([A-Za-z_]\w*)"#).expect("valid function name regex")
});
static LOCAL_ASSIGN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^\s*(?:var\s+)?([A-Za-z_]\w*)\s*(?::=|=)\s*(.+?)\s*$"#)
        .expect("valid local assignment regex")
});
static WEB_METHOD_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"web\.(Get|Post|Put|Delete)\("#).expect("valid web route regex")
});
static EXCHANGE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?s)\.exchange\(\s*[^,]+,\s*([^,]+?)\s*,\s*([^,]+?)\s*,\s*([^,]+?)\s*,"#)
        .expect("valid exchange regex")
});
static HTTP_GET_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"http\.Get\(\s*(.+?)\s*\)"#).expect("valid http get regex"));
static HTTP_POST_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"http\.Post\(\s*(.+?)\s*,\s*[^,]+,\s*.+?\)"#).expect("valid http post regex")
});
static HTTP_NEW_REQUEST_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"http\.NewRequest(?:WithContext)?\(\s*(?:[^,]+,\s*)?([^,]+?)\s*,\s*(.+?)\s*,\s*.+?\)"#,
    )
    .expect("valid new request regex")
});
static HTTP_METHOD_CONST_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"http\.Method(Get|Post|Put|Delete|Patch)"#).expect("valid method const regex")
});
static STRING_LITERAL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^"((?:[^"\\]|\\.)*)"$"#).expect("valid string regex"));
static URL_PATH_ESCAPE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"url\.PathEscape\(\s*([A-Za-z_]\w*)\s*\)"#).expect("valid path escape regex")
});
static PATH_VALUE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:request\.)?PathValue\(\s*"([^"]+)"\s*\)"#).expect("valid path value regex")
});
static TRIM_RIGHT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"strings\.TrimRight\(\s*([^,]+?)\s*,\s*"/"\s*\)"#).expect("valid trim right regex")
});

const GO_SOURCE_SENTINEL: &str = "__go_source__";

pub fn extract_syntactic(text: &str, file_path: &str) -> Result<FileRecord, ExtractionError> {
    let globals = collect_string_bindings(text);
    let mut callables = extract_callables(text, file_path);
    let callable_index = build_callable_index(&callables);
    let mut synthetic_callables = Vec::new();
    let endpoints = extract_endpoints(
        text,
        file_path,
        &globals,
        &callable_index,
        &mut synthetic_callables,
    );
    callables.extend(synthetic_callables);

    let mut assignments = HashMap::new();
    assignments.insert(
        AssignmentKey {
            scope: Scope::Global,
            variable_name: GO_SOURCE_SENTINEL.to_string(),
        },
        Assignment {
            variable_name: GO_SOURCE_SENTINEL.to_string(),
            variable_type: "string".to_string(),
            value: text.to_string(),
        },
    );

    Ok(FileRecord {
        file_path: file_path.to_string(),
        language: Language::Go,
        imports: vec![],
        entities: vec![],
        endpoints,
        callables,
        call_statements: vec![],
        assignments,
        enums: vec![],
        raw_message_edges: vec![],
    })
}

pub fn identify(file: &mut TypedFileRecord, code: &str) {
    let globals = collect_string_bindings(code);
    let callables = build_callable_index(&file.callables);
    file.raw_restcalls = extract_restcalls(code, &file.file_path, &globals, &callables);
}

pub fn source_from_assignments(file: &TypedFileRecord) -> Option<&str> {
    file.assignments
        .get(&AssignmentKey {
            scope: Scope::Global,
            variable_name: GO_SOURCE_SENTINEL.to_string(),
        })
        .map(|assignment| assignment.value.as_str())
}

fn extract_callables(code: &str, file_path: &str) -> Vec<ParsedCallable> {
    find_functions(code)
        .into_iter()
        .map(|function| ParsedCallable {
            metadata: Callable {
                name: function.signature.clone(),
                signature: function.signature,
                namespace: function.namespace,
                parameters: vec![],
                return_type: None,
                is_async: false,
                is_constructor: false,
                hash: function.hash,
                file_path: file_path.to_string(),
            },
            ast: CallableAst {
                statements: vec![],
                nested: vec![],
            },
        })
        .collect()
}

fn build_callable_index(callables: &[ParsedCallable]) -> HashMap<String, Callable> {
    let mut index = HashMap::new();
    for callable in callables {
        index.insert(simple_callable_name(&callable.metadata.name), callable.metadata.clone());
    }
    index
}

fn extract_endpoints(
    code: &str,
    file_path: &str,
    globals: &HashMap<String, String>,
    callables: &HashMap<String, Callable>,
    synthetic_callables: &mut Vec<ParsedCallable>,
) -> Vec<Endpoint> {
    let mut endpoints = Vec::new();
    let mut synthetic_hashes = HashSet::new();

    for route in find_handlefunc_routes(code, globals) {
        let handler = resolve_handler_callable(
            file_path,
            &route.handler_expr,
            &route.uri,
            route.http_method.clone(),
            callables,
            synthetic_callables,
            &mut synthetic_hashes,
        );
        endpoints.push(Endpoint {
            function_name: handler.name,
            function_hash: handler.hash,
            http_method: route.http_method,
            parameters: vec![],
            uri: route.uri,
            file_path: file_path.to_string(),
            router_variable: None,
        });
    }

    for route in find_web_routes(code, globals) {
        let handler = resolve_handler_callable(
            file_path,
            &route.handler_expr,
            &route.uri,
            route.http_method.clone(),
            callables,
            synthetic_callables,
            &mut synthetic_hashes,
        );
        endpoints.push(Endpoint {
            function_name: handler.name,
            function_hash: handler.hash,
            http_method: route.http_method,
            parameters: vec![],
            uri: route.uri,
            file_path: file_path.to_string(),
            router_variable: None,
        });
    }

    endpoints
}

fn extract_restcalls(
    code: &str,
    file_path: &str,
    globals: &HashMap<String, String>,
    callables: &HashMap<String, Callable>,
) -> Vec<RestCall> {
    let mut restcalls = Vec::new();

    for function in find_functions(code) {
        let scope = merged_scope(globals, &function.body);
        let callable = callables
            .get(&simple_callable_name(&function.signature))
            .cloned()
            .unwrap_or_else(|| Callable {
                name: function.signature.clone(),
                signature: function.signature.clone(),
                namespace: function.namespace.clone(),
                parameters: vec![],
                return_type: None,
                is_async: false,
                is_constructor: false,
                hash: function.hash.clone(),
                file_path: file_path.to_string(),
            });

        for caps in EXCHANGE_RE.captures_iter(&function.body) {
            let Some(service_expr) = caps.get(1).map(|m| m.as_str()) else {
                continue;
            };
            let Some(method_expr) = caps.get(2).map(|m| m.as_str()) else {
                continue;
            };
            let Some(path_expr) = caps.get(3).map(|m| m.as_str()) else {
                continue;
            };
            let Some(method) = parse_http_method(method_expr) else {
                continue;
            };
            let service_name = evaluate_expr(service_expr, &scope);
            let path = evaluate_expr(path_expr, &scope);
            let target_uri =
                if service_name.starts_with("http://") || service_name.starts_with("https://") {
                    format!("{}{}", service_name.trim_end_matches('/'), path)
                } else {
                    format!("http://{}{}", service_name, path)
                };
            restcalls.push(RestCall {
                function_name: callable.signature.clone(),
                function_hash: callable.hash.clone(),
                call_arguments: vec![],
                http_method: method,
                target_uri,
                file_path: file_path.to_string(),
            });
        }

        for caps in HTTP_GET_RE.captures_iter(&function.body) {
            if let Some(url_expr) = caps.get(1).map(|m| m.as_str()) {
                restcalls.push(build_restcall(
                    file_path,
                    HttpMethod::GET,
                    url_expr,
                    &scope,
                    &callable,
                ));
            }
        }

        for caps in HTTP_POST_RE.captures_iter(&function.body) {
            if let Some(url_expr) = caps.get(1).map(|m| m.as_str()) {
                restcalls.push(build_restcall(
                    file_path,
                    HttpMethod::POST,
                    url_expr,
                    &scope,
                    &callable,
                ));
            }
        }

        for caps in HTTP_NEW_REQUEST_RE.captures_iter(&function.body) {
            let Some(method_expr) = caps.get(1).map(|m| m.as_str()) else {
                continue;
            };
            let Some(url_expr) = caps.get(2).map(|m| m.as_str()) else {
                continue;
            };
            let Some(method) = parse_http_method(method_expr) else {
                continue;
            };
            restcalls.push(build_restcall(file_path, method, url_expr, &scope, &callable));
        }
    }

    restcalls
}

fn build_restcall(
    file_path: &str,
    http_method: HttpMethod,
    url_expr: &str,
    scope: &HashMap<String, String>,
    callable: &Callable,
) -> RestCall {
    RestCall {
        function_name: callable.signature.clone(),
        function_hash: callable.hash.clone(),
        call_arguments: vec![],
        http_method,
        target_uri: evaluate_expr(url_expr, scope),
        file_path: file_path.to_string(),
    }
}

fn merged_scope(globals: &HashMap<String, String>, body: &str) -> HashMap<String, String> {
    let mut scope = globals.clone();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("return ") || trimmed.starts_with("if ") {
            continue;
        }
        let Some(caps) = LOCAL_ASSIGN_RE.captures(trimmed) else {
            continue;
        };
        let Some(name) = caps.get(1).map(|m| m.as_str()) else {
            continue;
        };
        let Some(expr) = caps.get(2).map(|m| m.as_str()) else {
            continue;
        };
        let value = evaluate_expr(expr, &scope);
        scope.insert(name.to_string(), value);
    }
    scope
}

fn find_functions(code: &str) -> Vec<FunctionBlock> {
    let mut functions = Vec::new();
    for caps in FUNC_RE.captures_iter(code) {
        let Some(full) = caps.get(0) else {
            continue;
        };
        let receiver = caps.get(1).map(|m| m.as_str()).unwrap_or_default().trim();
        let open_brace = full.end() - 1;
        let Some(close_brace) = find_matching_brace(code, open_brace) else {
            continue;
        };
        let signature = full.as_str().trim_end_matches('{').trim().to_string();
        let namespace = parse_namespace(receiver, code, full.start());
        let body = code[open_brace + 1..close_brace].to_string();
        let hash = hash_text(full.as_str());
        functions.push(FunctionBlock {
            signature,
            hash,
            namespace,
            body,
        });
    }
    functions
}

fn parse_namespace(receiver: &str, file_path: &str, offset: usize) -> Namespace {
    if receiver.is_empty() {
        return Namespace::Module(file_path.to_string());
    }
    let receiver_type = receiver
        .split_whitespace()
        .last()
        .unwrap_or(receiver)
        .trim_start_matches('*')
        .to_string();
    if receiver_type.is_empty() {
        Namespace::Module(format!("{file_path}#{offset}"))
    } else {
        Namespace::Class(receiver_type)
    }
}

fn find_matching_brace(code: &str, open_brace: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in code[open_brace..].char_indices() {
        match ch {
            '"' if !escaped => in_string = !in_string,
            '\\' if in_string => {
                escaped = !escaped;
                continue;
            }
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open_brace + idx);
                }
            }
            _ => {}
        }
        escaped = false;
    }

    None
}

fn collect_string_bindings(code: &str) -> HashMap<String, String> {
    let mut bindings = HashMap::new();

    for caps in SIMPLE_CONST_RE.captures_iter(code) {
        let Some(name) = caps.get(1).map(|m| m.as_str()) else {
            continue;
        };
        let Some(value) = caps.get(2).map(|m| m.as_str()) else {
            continue;
        };
        bindings.insert(name.to_string(), unquote(value));
    }

    for block in CONST_BLOCK_RE.captures_iter(code) {
        let Some(body) = block.get(1).map(|m| m.as_str()) else {
            continue;
        };
        for caps in BLOCK_CONST_ENTRY_RE.captures_iter(body) {
            let Some(name) = caps.get(1).map(|m| m.as_str()) else {
                continue;
            };
            let Some(value) = caps.get(2).map(|m| m.as_str()) else {
                continue;
            };
            bindings.insert(name.to_string(), unquote(value));
        }
    }

    bindings
}

fn find_handlefunc_routes(code: &str, globals: &HashMap<String, String>) -> Vec<RouteRegistration> {
    let mut routes = Vec::new();
    for line in code.lines() {
        let Some(idx) = line.find(".HandleFunc(") else {
            continue;
        };
        let args = split_args(&line[idx + ".HandleFunc(".len()..]);
        if args.len() < 2 {
            continue;
        }
        let path_expr = args[0];
        let handler_expr = args[1].trim().to_string();

        if let Some(methods_idx) = line.find(".Methods(") {
            let methods_args = split_args(&line[methods_idx + ".Methods(".len()..]);
            let path = evaluate_expr(path_expr, globals);
            for method in methods_args
                .into_iter()
                .filter_map(|value| parse_http_method(value.trim()))
            {
                routes.push(RouteRegistration {
                    http_method: method,
                    uri: path.clone(),
                    handler_expr: handler_expr.clone(),
                });
            }
        } else {
            let resolved = evaluate_expr(path_expr, globals);
            let Some((method, uri)) = split_method_and_path(&resolved) else {
                continue;
            };
            routes.push(RouteRegistration {
                http_method: method,
                uri,
                handler_expr,
            });
        }
    }
    routes
}

fn find_web_routes(code: &str, globals: &HashMap<String, String>) -> Vec<RouteRegistration> {
    let mut routes = Vec::new();
    for line in code.lines() {
        let Some(caps) = WEB_METHOD_RE.captures(line) else {
            continue;
        };
        let Some(method_text) = caps.get(1).map(|m| m.as_str()) else {
            continue;
        };
        let Some(start) = line.find(&format!("web.{method_text}(")) else {
            continue;
        };
        let args = split_args(&line[start + format!("web.{method_text}(").len()..]);
        if args.len() < 2 {
            continue;
        }
        let Ok(http_method) = HttpMethod::from_str(&method_text.to_uppercase()) else {
            continue;
        };
        routes.push(RouteRegistration {
            http_method,
            uri: evaluate_expr(args[0], globals),
            handler_expr: args[1].trim().to_string(),
        });
    }
    routes
}

fn resolve_handler_callable(
    file_path: &str,
    handler_expr: &str,
    uri: &str,
    method: HttpMethod,
    callables: &HashMap<String, Callable>,
    synthetic_callables: &mut Vec<ParsedCallable>,
    synthetic_hashes: &mut HashSet<String>,
) -> Callable {
    if let Some(actual) = lookup_callable(handler_expr, callables) {
        return actual;
    }

    let signature = format!("handler {} {}", format_http_method(&method), uri);
    let hash = hash_text(&format!("{file_path}::{signature}::{handler_expr}"));
    if synthetic_hashes.insert(hash.clone()) {
        synthetic_callables.push(ParsedCallable {
            metadata: Callable {
                name: signature.clone(),
                signature: signature.clone(),
                namespace: Namespace::Module(file_path.to_string()),
                parameters: vec![],
                return_type: None,
                is_async: false,
                is_constructor: false,
                hash: hash.clone(),
                file_path: file_path.to_string(),
            },
            ast: CallableAst {
                statements: vec![],
                nested: vec![],
            },
        });
    }

    Callable {
        name: signature.clone(),
        signature,
        namespace: Namespace::Module(file_path.to_string()),
        parameters: vec![],
        return_type: None,
        is_async: false,
        is_constructor: false,
        hash,
        file_path: file_path.to_string(),
    }
}

fn lookup_callable(handler_expr: &str, callables: &HashMap<String, Callable>) -> Option<Callable> {
    let trimmed = handler_expr.trim();
    let key = trimmed
        .split('.')
        .next_back()
        .unwrap_or(trimmed)
        .trim();
    callables.get(key).cloned()
}

fn simple_callable_name(signature: &str) -> String {
    FUNC_NAME_RE
        .captures(signature)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
        .unwrap_or_else(|| {
            signature
                .split('(')
                .next()
                .unwrap_or(signature)
                .split_whitespace()
                .next_back()
                .unwrap_or(signature)
                .to_string()
        })
}

fn split_args(input: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut start = 0usize;

    for (idx, ch) in input.char_indices() {
        match ch {
            '"' if !escaped => in_string = !in_string,
            '\\' if in_string => {
                escaped = !escaped;
                continue;
            }
            '(' if !in_string => depth += 1,
            ')' if !in_string => {
                if depth == 0 {
                    args.push(input[start..idx].trim());
                    return args;
                }
                depth -= 1;
            }
            ',' if !in_string && depth == 0 => {
                args.push(input[start..idx].trim());
                start = idx + 1;
            }
            _ => {}
        }
        escaped = false;
    }

    args
}

fn split_method_and_path(value: &str) -> Option<(HttpMethod, String)> {
    let (method, path) = value.split_once(' ')?;
    Some((HttpMethod::from_str(method).ok()?, path.to_string()))
}

fn parse_http_method(expr: &str) -> Option<HttpMethod> {
    let trimmed = expr.trim();
    if let Some(caps) = HTTP_METHOD_CONST_RE.captures(trimmed) {
        return HttpMethod::from_str(caps.get(1)?.as_str()).ok();
    }
    if let Some(literal) = STRING_LITERAL_RE.captures(trimmed) {
        return HttpMethod::from_str(literal.get(1)?.as_str()).ok();
    }
    HttpMethod::from_str(trimmed).ok()
}

fn evaluate_expr(expr: &str, scope: &HashMap<String, String>) -> String {
    let trimmed = expr.trim().trim_end_matches(',');
    if let Some(parts) = split_top_level(trimmed, '+') {
        return parts
            .into_iter()
            .map(|part| evaluate_expr(&part, scope))
            .collect::<String>();
    }
    if let Some(caps) = STRING_LITERAL_RE.captures(trimmed) {
        return unescape_basic(caps.get(1).map(|m| m.as_str()).unwrap_or_default());
    }
    if let Some(caps) = URL_PATH_ESCAPE_RE.captures(trimmed) {
        return format!("{{{}}}", caps.get(1).map(|m| m.as_str()).unwrap_or("value"));
    }
    if let Some(caps) = PATH_VALUE_RE.captures(trimmed) {
        return format!("{{{}}}", caps.get(1).map(|m| m.as_str()).unwrap_or("value"));
    }
    if let Some(caps) = TRIM_RIGHT_RE.captures(trimmed) {
        return evaluate_expr(caps.get(1).map(|m| m.as_str()).unwrap_or_default(), scope)
            .trim_end_matches('/')
            .to_string();
    }
    if let Some(value) = scope.get(trimmed) {
        return value.clone();
    }
    trimmed.to_string()
}

fn split_top_level(input: &str, delimiter: char) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut start = 0usize;
    let mut saw_delimiter = false;

    for (idx, ch) in input.char_indices() {
        match ch {
            '"' if !escaped => in_string = !in_string,
            '\\' if in_string => {
                escaped = !escaped;
                continue;
            }
            '(' if !in_string => depth += 1,
            ')' if !in_string => depth -= 1,
            _ if ch == delimiter && !in_string && depth == 0 => {
                parts.push(input[start..idx].trim().to_string());
                start = idx + ch.len_utf8();
                saw_delimiter = true;
            }
            _ => {}
        }
        escaped = false;
    }

    if !saw_delimiter {
        return None;
    }
    parts.push(input[start..].trim().to_string());
    Some(parts)
}

fn unquote(input: &str) -> String {
    STRING_LITERAL_RE
        .captures(input)
        .and_then(|caps| caps.get(1).map(|m| unescape_basic(m.as_str())))
        .unwrap_or_else(|| input.to_string())
}

fn unescape_basic(input: &str) -> String {
    input.replace("\\\"", "\"").replace("\\n", "\n")
}

fn format_http_method(method: &HttpMethod) -> &'static str {
    match method {
        HttpMethod::GET => "GET",
        HttpMethod::POST => "POST",
        HttpMethod::PUT => "PUT",
        HttpMethod::DELETE => "DELETE",
        HttpMethod::PATCH => "PATCH",
    }
}

struct FunctionBlock {
    signature: String,
    hash: String,
    namespace: Namespace,
    body: String,
}

struct RouteRegistration {
    http_method: HttpMethod,
    uri: String,
    handler_expr: String,
}

#[cfg(test)]
mod tests {
    use super::{extract_callables, extract_endpoints, extract_restcalls};

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

        let globals = super::collect_string_bindings(code);
        let mut callables = extract_callables(code, "router.go");
        let callable_index = super::build_callable_index(&callables);
        let mut synthetic = Vec::new();
        let endpoints =
            extract_endpoints(code, "router.go", &globals, &callable_index, &mut synthetic);
        callables.extend(synthetic);
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].uri, "/api/v1/stationservice/stations");
        assert!(!endpoints[0].function_hash.is_empty());

        let restcalls = extract_restcalls(
            code,
            "client.go",
            &globals,
            &super::build_callable_index(&callables),
        );
        assert_eq!(restcalls.len(), 1);
        assert_eq!(
            restcalls[0].target_uri,
            "http://ts-route-service/api/v1/routeservice/routes/{start}/{end}"
        );
        assert!(!restcalls[0].function_hash.is_empty());
    }

    #[test]
    fn extracts_gorilla_and_direct_http_calls() {
        let code = r#"
func Router() {
    r.HandleFunc("/payment/{order_id}", paymentController.UpdatePaymentStatus).Methods("POST")
}

func invoke(url string) {
    req, err := http.NewRequest(http.MethodPost, url+"/ship-order", nil)
    _ = err
    _ = req
}
"#;
        let globals = super::collect_string_bindings(code);
        let callables = extract_callables(code, "router.go");
        let callable_index = super::build_callable_index(&callables);
        let mut synthetic = Vec::new();
        let endpoints =
            extract_endpoints(code, "router.go", &globals, &callable_index, &mut synthetic);
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].uri, "/payment/{order_id}");
        assert!(!endpoints[0].function_hash.is_empty());

        let mut all_callables = callables;
        all_callables.extend(synthetic);
        let restcalls = extract_restcalls(
            code,
            "client.go",
            &globals,
            &super::build_callable_index(&all_callables),
        );
        assert_eq!(restcalls.len(), 1);
        assert_eq!(restcalls[0].target_uri, "url/ship-order");
        assert!(!restcalls[0].function_hash.is_empty());
    }
}
