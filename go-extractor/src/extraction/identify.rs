use std::collections::HashMap;

use models::{Argument, CallStatement, HttpMethod, RestCall, Scope, ir::project::TypedFileRecord};

use super::shared::{
    evaluate_expression_text, merged_scope_bindings, merged_scope_bindings_with_globals,
    parse_http_method_value,
};

pub(super) fn identify_restcall(
    file: &TypedFileRecord,
    call: &CallStatement,
    package_globals: Option<&HashMap<String, String>>,
) -> Option<RestCall> {
    let scope = call
        .enclosing_function_name
        .as_ref()
        .map(|name| Scope::Function(name.clone()))
        .unwrap_or(Scope::Global);
    let mut resolved_scope = package_globals
        .map(|globals| merged_scope_bindings_with_globals(&file.assignments, &scope, globals))
        .unwrap_or_else(|| merged_scope_bindings(&file.assignments, &scope));
    if let Some(globals) = package_globals {
        add_receiver_field_aliases(call, globals, &mut resolved_scope);
    }

    if call.function_name.ends_with(".exchange") && call.arguments.len() >= 4 {
        let service = resolve_argument_value(&call.arguments[1], &resolved_scope);
        let method =
            parse_http_method_value(&resolve_argument_value(&call.arguments[2], &resolved_scope))?;
        let path = resolve_argument_value(&call.arguments[3], &resolved_scope);
        let target_uri = if service.starts_with("http://") || service.starts_with("https://") {
            format!("{}{}", service.trim_end_matches('/'), path)
        } else {
            format!("http://{}{}", service, path)
        };
        return Some(build_restcall(file, call, method, target_uri));
    }

    if call.function_name == "http.Get" && !call.arguments.is_empty() {
        let target_uri = resolve_argument_value(&call.arguments[0], &resolved_scope);
        return Some(build_restcall(file, call, HttpMethod::GET, target_uri));
    }

    if call.function_name.ends_with(".Get")
        && !call.function_name.starts_with("http.")
        && !call.arguments.is_empty()
        && !is_route_registration(file, call, &HttpMethod::GET, &resolved_scope)
    {
        let target_uri = resolve_argument_value(&call.arguments[0], &resolved_scope);
        return Some(build_restcall(file, call, HttpMethod::GET, target_uri));
    }

    if call.function_name == "http.Post" && !call.arguments.is_empty() {
        let target_uri = resolve_argument_value(&call.arguments[0], &resolved_scope);
        return Some(build_restcall(file, call, HttpMethod::POST, target_uri));
    }

    if call.function_name.ends_with(".Post")
        && !call.function_name.starts_with("http.")
        && !call.arguments.is_empty()
        && !is_route_registration(file, call, &HttpMethod::POST, &resolved_scope)
    {
        let target_uri = resolve_argument_value(&call.arguments[0], &resolved_scope);
        return Some(build_restcall(file, call, HttpMethod::POST, target_uri));
    }

    if matches!(
        call.function_name.as_str(),
        "http.NewRequest" | "http.NewRequestWithContext"
    ) {
        let method_index = usize::from(call.function_name == "http.NewRequestWithContext");
        let url_index = method_index + 1;
        if call.arguments.len() <= url_index {
            return None;
        }
        let method = parse_http_method_value(&resolve_argument_value(
            &call.arguments[method_index],
            &resolved_scope,
        ))?;
        let target_uri = resolve_argument_value(&call.arguments[url_index], &resolved_scope);
        return Some(build_restcall(file, call, method, target_uri));
    }

    if call.function_name.ends_with(".Do")
        && !call.function_name.starts_with("http.")
        && !call.arguments.is_empty()
    {
        let request_value = resolve_argument_value(&call.arguments[0], &resolved_scope);
        if request_value.starts_with("http.NewRequest(")
            || request_value.starts_with("http.NewRequestWithContext(")
        {
            return None;
        }
        if let Some((method, target_uri)) = parse_request_call(&request_value, &resolved_scope) {
            return Some(build_restcall(file, call, method, target_uri));
        }
    }

    None
}

fn is_route_registration(
    file: &TypedFileRecord,
    call: &CallStatement,
    method: &HttpMethod,
    scope: &HashMap<String, String>,
) -> bool {
    if call.arguments.len() != 2 {
        return false;
    }

    let uri = resolve_argument_value(&call.arguments[0], scope);
    file.endpoints
        .iter()
        .any(|endpoint| endpoint.http_method == *method && endpoint.uri == uri)
}

fn add_receiver_field_aliases(
    call: &CallStatement,
    package_globals: &HashMap<String, String>,
    resolved_scope: &mut HashMap<String, String>,
) {
    let Some(signature) = call.enclosing_function_name.as_deref() else {
        return;
    };
    let Some(class_name) = call.enclosing_class_name.as_deref() else {
        return;
    };
    let Some(receiver_name) = parse_receiver_name(signature) else {
        return;
    };

    for (key, value) in package_globals {
        let Some((root, field)) = key.split_once('.') else {
            continue;
        };
        let Some(root_value) = package_globals.get(root) else {
            continue;
        };
        if !matches_receiver_instance(root_value, class_name) {
            continue;
        }
        resolved_scope
            .entry(format!("{receiver_name}.{field}"))
            .or_insert_with(|| value.clone());
    }
}

fn parse_receiver_name(signature: &str) -> Option<&str> {
    let receiver = signature
        .strip_prefix("func")?
        .trim_start()
        .strip_prefix('(')?
        .split_once(')')?
        .0
        .trim();
    receiver.split_whitespace().next()
}

fn matches_receiver_instance(instance: &str, class_name: &str) -> bool {
    instance.contains(&format!("*{class_name}"))
        || instance.contains(&format!("&{class_name}"))
        || instance.contains(&format!("{class_name}{{"))
        || instance.contains(&format!("New{class_name}("))
}

fn build_restcall(
    file: &TypedFileRecord,
    call: &CallStatement,
    http_method: HttpMethod,
    target_uri: String,
) -> RestCall {
    RestCall {
        function_name: call
            .enclosing_function_name
            .clone()
            .unwrap_or_else(|| call.function_name.clone()),
        function_hash: call.enclosing_function_hash.clone().unwrap_or_default(),
        call_arguments: call.arguments.clone(),
        http_method,
        target_uri,
        file_path: file.file_path.clone(),
    }
}

fn resolve_argument_value(argument: &Argument, scope: &HashMap<String, String>) -> String {
    evaluate_expression_text(&argument.value, scope)
}

fn parse_request_call(raw: &str, scope: &HashMap<String, String>) -> Option<(HttpMethod, String)> {
    let (name, args) = raw.split_once('(')?;
    if !matches!(
        name.trim(),
        "http.NewRequest" | "http.NewRequestWithContext"
    ) {
        return None;
    }

    let body = args.strip_suffix(')')?;
    let args = split_args(body);
    let method_index = usize::from(name.trim().ends_with("WithContext"));
    let url_index = method_index + 1;
    if args.len() <= url_index {
        return None;
    }

    let method = parse_http_method_value(&evaluate_expression_text(args[method_index], scope))?;
    let target_uri = evaluate_expression_text(args[url_index], scope);
    Some((method, target_uri))
}

fn split_args(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut quote = '\0';
    let mut start = 0usize;

    for (index, ch) in input.char_indices() {
        match ch {
            '"' | '`' => {
                if !in_string {
                    in_string = true;
                    quote = ch;
                } else if ch == quote {
                    in_string = false;
                }
            }
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth -= 1,
            ',' if !in_string && depth == 0 => {
                parts.push(input[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }

    let tail = input[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}
