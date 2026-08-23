use std::collections::HashMap;

use models::{Argument, CallStatement, HttpMethod, RestCall, Scope, ir::project::TypedFileRecord};

use super::shared::{evaluate_expression_text, merged_scope_bindings, parse_http_method_value};

pub(super) fn identify_restcall(file: &TypedFileRecord, call: &CallStatement) -> Option<RestCall> {
    let scope = call
        .enclosing_function_name
        .as_ref()
        .map(|name| Scope::Function(name.clone()))
        .unwrap_or(Scope::Global);
    let resolved_scope = merged_scope_bindings(&file.assignments, &scope);

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

    if call.function_name == "http.Post" && !call.arguments.is_empty() {
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

    None
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
