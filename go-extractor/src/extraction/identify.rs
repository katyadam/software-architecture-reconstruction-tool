use std::collections::HashMap;

use models::{
    Argument, CallStatement, HttpMethod, ParsedCallable, RestCall, Scope,
    ir::ast::{Expr, Stmt},
    ir::project::TypedFileRecord,
};

use super::evaluator::evaluate_call_text;
use super::shared::{
    evaluate_expression_text, merged_scope_bindings, merged_scope_bindings_with_globals,
    parse_http_method_value,
};

struct IdentifyContext<'a> {
    file: &'a TypedFileRecord,
    call: &'a CallStatement,
    resolved_scope: &'a HashMap<String, String>,
    package_callables: &'a [ParsedCallable],
}

trait RestCallIdentificationStrategy: Sync {
    fn identify(&self, ctx: &IdentifyContext<'_>) -> Option<RestCall>;
}

struct ExchangeStrategy;
struct NetHttpVerbStrategy;
struct ClientVerbStrategy;
struct NewRequestStrategy;
struct DoRequestStrategy;

static EXCHANGE: ExchangeStrategy = ExchangeStrategy;
static NET_HTTP_VERB: NetHttpVerbStrategy = NetHttpVerbStrategy;
static CLIENT_VERB: ClientVerbStrategy = ClientVerbStrategy;
static NEW_REQUEST: NewRequestStrategy = NewRequestStrategy;
static DO_REQUEST: DoRequestStrategy = DoRequestStrategy;
static IDENTIFICATION_STRATEGIES: &[&dyn RestCallIdentificationStrategy] = &[
    &EXCHANGE,
    &NET_HTTP_VERB,
    &CLIENT_VERB,
    &NEW_REQUEST,
    &DO_REQUEST,
];

pub(super) fn identify_restcall(
    file: &TypedFileRecord,
    call: &CallStatement,
    package_globals: Option<&HashMap<String, String>>,
    package_callables: &[ParsedCallable],
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
    add_constructor_receiver_field_aliases(call, package_callables, &mut resolved_scope);
    let ctx = IdentifyContext {
        file,
        call,
        resolved_scope: &resolved_scope,
        package_callables,
    };
    IDENTIFICATION_STRATEGIES
        .iter()
        .find_map(|strategy| strategy.identify(&ctx))
}

impl RestCallIdentificationStrategy for ExchangeStrategy {
    fn identify(&self, ctx: &IdentifyContext<'_>) -> Option<RestCall> {
        if !ctx.call.function_name.ends_with(".exchange") || ctx.call.arguments.len() < 4 {
            return None;
        }

        let service = resolve_argument_value(
            &ctx.call.arguments[1],
            ctx.resolved_scope,
            ctx.package_callables,
        );
        let method = parse_http_method_value(&resolve_argument_value(
            &ctx.call.arguments[2],
            ctx.resolved_scope,
            ctx.package_callables,
        ))?;
        let path = resolve_argument_value(
            &ctx.call.arguments[3],
            ctx.resolved_scope,
            ctx.package_callables,
        );
        let target_uri = if service.starts_with("http://") || service.starts_with("https://") {
            format!("{}{}", service.trim_end_matches('/'), path)
        } else {
            format!("http://{}{}", service, path)
        };
        Some(build_restcall(ctx.file, ctx.call, method, target_uri))
    }
}

impl RestCallIdentificationStrategy for NetHttpVerbStrategy {
    fn identify(&self, ctx: &IdentifyContext<'_>) -> Option<RestCall> {
        direct_http_verb("Get", HttpMethod::GET, ctx)
            .or_else(|| direct_http_verb("Post", HttpMethod::POST, ctx))
    }
}

impl RestCallIdentificationStrategy for ClientVerbStrategy {
    fn identify(&self, ctx: &IdentifyContext<'_>) -> Option<RestCall> {
        client_http_verb("Get", HttpMethod::GET, ctx)
            .or_else(|| client_http_verb("Post", HttpMethod::POST, ctx))
    }
}

impl RestCallIdentificationStrategy for NewRequestStrategy {
    fn identify(&self, ctx: &IdentifyContext<'_>) -> Option<RestCall> {
        if !is_net_http_call(&ctx.call.function_name, "NewRequest", ctx.file)
            && !is_net_http_call(&ctx.call.function_name, "NewRequestWithContext", ctx.file)
        {
            return None;
        }

        let method_index = usize::from(ctx.call.function_name.ends_with(".NewRequestWithContext"));
        let url_index = method_index + 1;
        if ctx.call.arguments.len() <= url_index {
            return None;
        }
        let method = parse_http_method_value(&resolve_argument_value(
            &ctx.call.arguments[method_index],
            ctx.resolved_scope,
            ctx.package_callables,
        ))?;
        let target_uri = resolve_argument_value(
            &ctx.call.arguments[url_index],
            ctx.resolved_scope,
            ctx.package_callables,
        );
        Some(build_restcall(ctx.file, ctx.call, method, target_uri))
    }
}

impl RestCallIdentificationStrategy for DoRequestStrategy {
    fn identify(&self, ctx: &IdentifyContext<'_>) -> Option<RestCall> {
        if !ctx.call.function_name.ends_with(".Do")
            || ctx.call.function_name.starts_with("http.")
            || ctx.call.arguments.is_empty()
            || !is_http_client_receiver(&ctx.call.function_name, ctx.resolved_scope)
        {
            return None;
        }

        let request_value = resolve_argument_value(
            &ctx.call.arguments[0],
            ctx.resolved_scope,
            ctx.package_callables,
        );
        if request_value.starts_with("http.NewRequest(")
            || request_value.starts_with("http.NewRequestWithContext(")
        {
            return None;
        }
        let (method, target_uri) =
            parse_request_call(&request_value, ctx.resolved_scope, ctx.file)?;
        Some(build_restcall(ctx.file, ctx.call, method, target_uri))
    }
}

fn direct_http_verb(
    method_name: &str,
    http_method: HttpMethod,
    ctx: &IdentifyContext<'_>,
) -> Option<RestCall> {
    if !is_net_http_call(&ctx.call.function_name, method_name, ctx.file)
        || ctx.call.arguments.is_empty()
    {
        return None;
    }
    let target_uri = resolve_argument_value(
        &ctx.call.arguments[0],
        ctx.resolved_scope,
        ctx.package_callables,
    );
    Some(build_restcall(ctx.file, ctx.call, http_method, target_uri))
}

fn client_http_verb(
    method_name: &str,
    http_method: HttpMethod,
    ctx: &IdentifyContext<'_>,
) -> Option<RestCall> {
    if !ctx.call.function_name.ends_with(&format!(".{method_name}"))
        || ctx.call.function_name.starts_with("http.")
        || ctx.call.arguments.is_empty()
        || is_route_registration(
            ctx.file,
            ctx.call,
            &http_method,
            ctx.resolved_scope,
            ctx.package_callables,
        )
        || !is_http_client_receiver(&ctx.call.function_name, ctx.resolved_scope)
    {
        return None;
    }
    let target_uri = resolve_argument_value(
        &ctx.call.arguments[0],
        ctx.resolved_scope,
        ctx.package_callables,
    );
    Some(build_restcall(ctx.file, ctx.call, http_method, target_uri))
}

fn is_net_http_call(function_name: &str, method: &str, file: &TypedFileRecord) -> bool {
    let Some((receiver, selector)) = function_name.rsplit_once('.') else {
        return false;
    };
    selector == method
        && (receiver == "http"
            || file
                .imports
                .iter()
                .any(|import| import.orig_module == "net/http" && import.module_alias == receiver))
}

fn is_http_client_receiver(function_name: &str, scope: &HashMap<String, String>) -> bool {
    let Some((receiver, _method)) = function_name.rsplit_once('.') else {
        return false;
    };
    let receiver_name = receiver
        .trim_end_matches(".R()")
        .rsplit('.')
        .next()
        .unwrap_or(receiver)
        .to_ascii_lowercase();
    if matches!(
        receiver_name.as_str(),
        "client" | "restclient" | "httpclient" | "apiclient" | "defaultclient"
    ) {
        return true;
    }

    scope.get(receiver).is_some_and(|origin| {
        origin.contains("http.Client")
            || origin.contains("resty.New(")
            || origin.contains("retryablehttp.NewClient(")
    })
}

fn is_route_registration(
    file: &TypedFileRecord,
    call: &CallStatement,
    method: &HttpMethod,
    scope: &HashMap<String, String>,
    package_callables: &[ParsedCallable],
) -> bool {
    if call.arguments.len() != 2 {
        return false;
    }

    let uri = resolve_argument_value(&call.arguments[0], scope, package_callables);
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

fn add_constructor_receiver_field_aliases(
    call: &CallStatement,
    package_callables: &[ParsedCallable],
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

    for (field, value) in constructor_field_aliases(class_name, package_callables, resolved_scope) {
        resolved_scope
            .entry(format!("{receiver_name}.{field}"))
            .or_insert(value);
    }
}

fn constructor_field_aliases(
    class_name: &str,
    package_callables: &[ParsedCallable],
    scope: &HashMap<String, String>,
) -> Vec<(String, String)> {
    package_callables
        .iter()
        .filter(|callable| callable_returns_class(callable, class_name))
        .find_map(|callable| evaluate_constructor_fields(callable, package_callables, scope))
        .unwrap_or_default()
}

fn callable_returns_class(callable: &ParsedCallable, class_name: &str) -> bool {
    callable
        .metadata
        .return_type
        .as_deref()
        .is_some_and(|return_type| return_type.contains(class_name))
        || callable
            .ast
            .statements
            .iter()
            .any(|statement| returned_struct_matches_class(statement, class_name))
}

fn returned_struct_matches_class(statement: &Stmt, class_name: &str) -> bool {
    match statement {
        Stmt::Return(Expr::StructLiteral { type_name, .. }) => type_name
            .as_deref()
            .is_some_and(|type_name| type_name.trim_start_matches('&') == class_name),
        _ => false,
    }
}

fn evaluate_constructor_fields(
    callable: &ParsedCallable,
    package_callables: &[ParsedCallable],
    scope: &HashMap<String, String>,
) -> Option<Vec<(String, String)>> {
    let mut env = scope.clone();
    let mut struct_bindings = HashMap::<String, Vec<(String, Expr)>>::new();

    for statement in &callable.ast.statements {
        match statement {
            Stmt::Declaration { name, value, .. } | Stmt::Assignment { name, value } => {
                if let Expr::StructLiteral { fields, .. } = value {
                    struct_bindings.insert(name.clone(), fields.clone());
                } else if let Some(value) = resolve_expr_value(value, package_callables, &env) {
                    env.insert(name.clone(), value);
                }
            }
            Stmt::Return(value) => {
                return struct_literal_fields(value, package_callables, &env, &struct_bindings);
            }
            _ => {}
        }
    }

    None
}

fn struct_literal_fields(
    value: &Expr,
    package_callables: &[ParsedCallable],
    env: &HashMap<String, String>,
    struct_bindings: &HashMap<String, Vec<(String, Expr)>>,
) -> Option<Vec<(String, String)>> {
    match value {
        Expr::StructLiteral { fields, .. } => Some(
            fields
                .iter()
                .filter_map(|(field, expr)| {
                    resolve_expr_value(expr, package_callables, env)
                        .map(|value| (field.clone(), value))
                })
                .collect(),
        ),
        Expr::Var(name) => struct_bindings.get(name).map(|fields| {
            fields
                .iter()
                .filter_map(|(field, expr)| {
                    resolve_expr_value(expr, package_callables, env)
                        .map(|value| (field.clone(), value))
                })
                .collect()
        }),
        _ => None,
    }
}

fn resolve_expr_value(
    expr: &Expr,
    package_callables: &[ParsedCallable],
    env: &HashMap<String, String>,
) -> Option<String> {
    let rendered = render_expr(expr)?;
    Some(
        evaluate_call_text(&rendered, package_callables, env)
            .unwrap_or_else(|| evaluate_expression_text(&rendered, env)),
    )
}

fn render_expr(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Literal(value) => Some(value.clone()),
        Expr::Var(name) => Some(name.clone()),
        Expr::Concat(left, right) => Some(format!("{}{}", render_expr(left)?, render_expr(right)?)),
        Expr::StructLiteral { .. } => None,
        Expr::Call {
            name,
            receiver,
            args,
        } => {
            let mut rendered = String::new();
            if let Some(receiver) = receiver {
                rendered.push_str(&render_expr(receiver)?);
                rendered.push('.');
            }
            rendered.push_str(name);
            rendered.push('(');
            rendered.push_str(
                &args
                    .iter()
                    .map(render_expr)
                    .collect::<Option<Vec<_>>>()?
                    .join(", "),
            );
            rendered.push(')');
            Some(rendered)
        }
        Expr::Empty => None,
        Expr::Joined { vals } => vals.first().and_then(render_expr),
        Expr::Attr { object, field } => Some(format!("{}.{}", render_expr(object)?, field)),
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

fn resolve_argument_value(
    argument: &Argument,
    scope: &HashMap<String, String>,
    package_callables: &[ParsedCallable],
) -> String {
    let resolved = evaluate_expression_text(&argument.value, scope);
    evaluate_call_text(&resolved, package_callables, scope).unwrap_or(resolved)
}

fn parse_request_call(
    raw: &str,
    scope: &HashMap<String, String>,
    file: &TypedFileRecord,
) -> Option<(HttpMethod, String)> {
    let (name, args) = raw.split_once('(')?;
    let name = name.trim();
    if !is_net_http_call(name, "NewRequest", file)
        && !is_net_http_call(name, "NewRequestWithContext", file)
    {
        return None;
    }

    let body = args.strip_suffix(')')?;
    let args = split_args(body);
    let method_index = usize::from(name.ends_with("WithContext"));
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
