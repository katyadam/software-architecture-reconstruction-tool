use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use models::{
    Argument, Assignment, AssignmentKey, CallStatement, Callable, Endpoint, HttpMethod, Namespace,
    ParsedCallable, RestCall, Scope,
    api::ExtractionError,
    ir::{ast::CallableAst, language::Language, project::TypedFileRecord, syntax::FileRecord},
};
use statix::strings::{hash_text, normalize_whitespace, strip_quotes};
use tree_sitter::{Node, Parser, Tree};

pub fn extract_syntactic(text: &str, file_path: &str) -> Result<FileRecord, ExtractionError> {
    let tree = parse_go_tree(text)?;
    let root = tree.root_node();

    let mut callables = Vec::new();
    let mut callable_lookup = HashMap::new();
    let mut call_statements = Vec::new();
    let mut assignments = HashMap::new();

    collect_global_assignments(root, text, &mut assignments);
    collect_callable_ir(
        root,
        text,
        file_path,
        &mut callables,
        &mut callable_lookup,
        &mut call_statements,
        &mut assignments,
    );

    let mut synthetic_callables = Vec::new();
    let endpoints = collect_endpoints(
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
        raw_message_edges: vec![],
    })
}

pub fn identify(file: &mut TypedFileRecord) {
    file.raw_restcalls = file
        .call_statements
        .iter()
        .filter_map(|call| identify_restcall(file, call))
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

fn collect_global_assignments(
    root: Node,
    code: &str,
    assignments: &mut HashMap<AssignmentKey, Assignment>,
) {
    for child in root.named_children(&mut root.walk()) {
        match child.kind() {
            "const_declaration" | "var_declaration" => {
                collect_declaration_assignments(child, code, Scope::Global, assignments);
            }
            _ => {}
        }
    }
}

fn collect_callable_ir(
    root: Node,
    code: &str,
    file_path: &str,
    callables: &mut Vec<ParsedCallable>,
    callable_lookup: &mut HashMap<String, Callable>,
    call_statements: &mut Vec<CallStatement>,
    assignments: &mut HashMap<AssignmentKey, Assignment>,
) {
    for child in root.named_children(&mut root.walk()) {
        if !matches!(child.kind(), "function_declaration" | "method_declaration") {
            continue;
        }

        let callable = build_callable(child, code, file_path);
        let metadata = callable.metadata.clone();
        register_callable_aliases(&metadata, callable_lookup);

        if let Some(body) = child.child_by_field_name("body") {
            collect_local_assignments(body, code, &metadata.signature, assignments);
            collect_call_statements(body, code, &metadata, call_statements);
        }

        callables.push(callable);
    }
}

fn build_callable(node: Node, code: &str, file_path: &str) -> ParsedCallable {
    let signature = callable_signature(node, code);
    let function_name = node
        .child_by_field_name("name")
        .map(|name| node_text(name, code).to_string())
        .unwrap_or_else(|| signature.clone());
    let body_hash = hash_text(node_text(node, code));
    let namespace = if node.kind() == "method_declaration" {
        Namespace::Class(
            receiver_type(
                node.child_by_field_name("receiver")
                    .map(|receiver| node_text(receiver, code))
                    .unwrap_or_default(),
            )
            .unwrap_or_else(|| file_path.to_string()),
        )
    } else {
        Namespace::Module(file_path.to_string())
    };

    ParsedCallable {
        metadata: Callable {
            name: function_name,
            signature,
            namespace,
            parameters: vec![],
            return_type: None,
            is_async: false,
            is_constructor: false,
            hash: body_hash,
            file_path: file_path.to_string(),
        },
        ast: CallableAst {
            statements: vec![],
            nested: vec![],
        },
    }
}

fn callable_signature(node: Node, code: &str) -> String {
    if let Some(body) = node.child_by_field_name("body") {
        return normalize_whitespace(code[node.start_byte()..body.start_byte()].trim());
    }
    normalize_whitespace(node_text(node, code))
}

fn receiver_type(receiver_text: &str) -> Option<String> {
    receiver_text
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split_whitespace()
        .next_back()
        .map(|value| value.trim_start_matches('*').to_string())
        .filter(|value| !value.is_empty())
}

fn register_callable_aliases(callable: &Callable, lookup: &mut HashMap<String, Callable>) {
    lookup.insert(callable.signature.clone(), callable.clone());
    lookup.insert(callable.name.clone(), callable.clone());
    if let Some(simple) = simple_callable_name(&callable.signature) {
        lookup.insert(simple, callable.clone());
    }
}

fn collect_declaration_assignments(
    node: Node,
    code: &str,
    scope: Scope,
    assignments: &mut HashMap<AssignmentKey, Assignment>,
) {
    for child in node.named_children(&mut node.walk()) {
        if matches!(child.kind(), "const_spec" | "var_spec") {
            collect_spec_assignments(child, code, scope.clone(), assignments);
        }
    }
}

fn collect_spec_assignments(
    spec: Node,
    code: &str,
    scope: Scope,
    assignments: &mut HashMap<AssignmentKey, Assignment>,
) {
    let mut identifiers = Vec::new();
    let mut values = Vec::new();

    for child in spec.named_children(&mut spec.walk()) {
        match child.kind() {
            "identifier" => identifiers.push(child),
            "expression_list" => {
                values.extend(child.named_children(&mut child.walk()));
            }
            "interpreted_string_literal"
            | "raw_string_literal"
            | "binary_expression"
            | "call_expression"
            | "selector_expression"
            | "parenthesized_expression" => values.push(child),
            _ => {}
        }
    }

    let mut scope_values = scope_bindings(assignments, &scope);
    scope_values.extend(scope_bindings(assignments, &Scope::Global));

    for (index, ident) in identifiers.into_iter().enumerate() {
        let variable_name = node_text(ident, code).to_string();
        let value = values
            .get(index)
            .map(|value_node| evaluate_expression_node(*value_node, code, &scope_values))
            .unwrap_or_default();
        let assignment = Assignment {
            variable_name: variable_name.clone(),
            variable_type: "".to_string(),
            value: value.clone(),
        };
        assignments.insert(
            AssignmentKey {
                scope: scope.clone(),
                variable_name: variable_name.clone(),
            },
            assignment,
        );
        scope_values.insert(variable_name, value);
    }
}

fn collect_local_assignments(
    body: Node,
    code: &str,
    function_signature: &str,
    assignments: &mut HashMap<AssignmentKey, Assignment>,
) {
    let scope = Scope::Function(function_signature.to_string());
    let mut scope_values = scope_bindings(assignments, &Scope::Global);
    walk_named(body, &mut |node| match node.kind() {
        "short_var_declaration" | "assignment_statement" => {
            let pairs = assignment_pairs(node, code);
            for (name, value_node) in pairs {
                let value = evaluate_expression_node(value_node, code, &scope_values);
                assignments.insert(
                    AssignmentKey {
                        scope: scope.clone(),
                        variable_name: name.clone(),
                    },
                    Assignment {
                        variable_name: name.clone(),
                        variable_type: "".to_string(),
                        value: value.clone(),
                    },
                );
                scope_values.insert(name, value);
            }
        }
        "const_declaration" | "var_declaration" => {
            collect_declaration_assignments(node, code, scope.clone(), assignments);
            scope_values.extend(scope_bindings(assignments, &scope));
        }
        _ => {}
    });
}

fn assignment_pairs<'a>(node: Node<'a>, code: &'a str) -> Vec<(String, Node<'a>)> {
    let mut left_values = Vec::new();
    let mut right_values = Vec::new();
    let mut seen_right = false;

    for child in node.named_children(&mut node.walk()) {
        match child.kind() {
            "expression_list" => {
                let values: Vec<Node> = child.named_children(&mut child.walk()).collect();
                if !seen_right {
                    left_values.extend(values);
                    seen_right = true;
                } else {
                    right_values.extend(values);
                }
            }
            _ => {
                if !seen_right && child.kind() == "identifier" {
                    left_values.push(child);
                } else {
                    seen_right = true;
                    right_values.push(child);
                }
            }
        }
    }

    left_values
        .into_iter()
        .zip(right_values)
        .filter_map(|(left, right)| {
            if left.kind() != "identifier" {
                return None;
            }
            Some((node_text(left, code).to_string(), right))
        })
        .collect()
}

fn collect_call_statements(
    body: Node,
    code: &str,
    callable: &Callable,
    call_statements: &mut Vec<CallStatement>,
) {
    walk_named(body, &mut |node| {
        if node.kind() != "call_expression" {
            return;
        }

        let Some(function_node) = node.child_by_field_name("function") else {
            return;
        };
        let function_name = normalize_whitespace(node_text(function_node, code));
        let arguments = node
            .child_by_field_name("arguments")
            .map(|args| parse_arguments(args, code))
            .unwrap_or_default();

        call_statements.push(CallStatement {
            function_name,
            arguments,
            enclosing_function_name: Some(callable.signature.clone()),
            enclosing_class_name: match &callable.namespace {
                Namespace::Class(name) => Some(name.clone()),
                Namespace::Module(_) => None,
            },
            enclosing_function_hash: Some(callable.hash.clone()),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        });
    });
}

fn parse_arguments(node: Node, code: &str) -> Vec<Argument> {
    node.named_children(&mut node.walk())
        .map(|arg| Argument {
            assigned_variable: "".to_string(),
            value: normalize_whitespace(node_text(arg, code)),
            datatype: None,
        })
        .collect()
}

fn collect_endpoints(
    root: Node,
    code: &str,
    file_path: &str,
    assignments: &HashMap<AssignmentKey, Assignment>,
    callable_lookup: &HashMap<String, Callable>,
    synthetic_callables: &mut Vec<ParsedCallable>,
) -> Vec<Endpoint> {
    let globals = scope_bindings(assignments, &Scope::Global);
    let mut synthetic_hashes = HashSet::new();
    let mut endpoints = Vec::new();

    walk_named(root, &mut |node| {
        if node.kind() != "call_expression" {
            return;
        }

        if let Some(endpoint) = endpoint_from_call(
            node,
            code,
            file_path,
            &globals,
            callable_lookup,
            synthetic_callables,
            &mut synthetic_hashes,
        ) {
            endpoints.push(endpoint);
        }
    });

    endpoints
}

fn endpoint_from_call(
    node: Node,
    code: &str,
    file_path: &str,
    globals: &HashMap<String, String>,
    callable_lookup: &HashMap<String, Callable>,
    synthetic_callables: &mut Vec<ParsedCallable>,
    synthetic_hashes: &mut HashSet<String>,
) -> Option<Endpoint> {
    let function_node = node.child_by_field_name("function")?;

    if let Some((method, path, handler)) = gorilla_route_parts(node, code, globals) {
        let callable = resolve_handler_callable(
            file_path,
            &handler,
            &path,
            &method,
            callable_lookup,
            synthetic_callables,
            synthetic_hashes,
        );
        return Some(Endpoint {
            function_name: callable.signature.clone(),
            function_hash: callable.hash,
            http_method: method,
            parameters: vec![],
            uri: path,
            file_path: file_path.to_string(),
            router_variable: None,
        });
    }

    let selector = selector_name(function_node, code)?;
    if selector == "HandleFunc" {
        let arguments = node
            .child_by_field_name("arguments")
            .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())
            .unwrap_or_default();
        if arguments.len() < 2 {
            return None;
        }
        let resolved = evaluate_expression_node(arguments[0], code, globals);
        let (method, uri) = split_method_and_path(&resolved)?;
        let handler_expr = normalize_whitespace(node_text(arguments[1], code));
        let callable = resolve_handler_callable(
            file_path,
            &handler_expr,
            &uri,
            &method,
            callable_lookup,
            synthetic_callables,
            synthetic_hashes,
        );
        return Some(Endpoint {
            function_name: callable.signature.clone(),
            function_hash: callable.hash,
            http_method: method,
            parameters: vec![],
            uri,
            file_path: file_path.to_string(),
            router_variable: None,
        });
    }

    if let Some(method) = web_route_method(&selector) {
        let arguments = node
            .child_by_field_name("arguments")
            .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())
            .unwrap_or_default();
        if arguments.len() < 2 {
            return None;
        }
        let uri = evaluate_expression_node(arguments[0], code, globals);
        let handler_expr = normalize_whitespace(node_text(arguments[1], code));
        let callable = resolve_handler_callable(
            file_path,
            &handler_expr,
            &uri,
            &method,
            callable_lookup,
            synthetic_callables,
            synthetic_hashes,
        );
        return Some(Endpoint {
            function_name: callable.signature.clone(),
            function_hash: callable.hash,
            http_method: method,
            parameters: vec![],
            uri,
            file_path: file_path.to_string(),
            router_variable: None,
        });
    }

    None
}

fn gorilla_route_parts(
    node: Node,
    code: &str,
    globals: &HashMap<String, String>,
) -> Option<(HttpMethod, String, String)> {
    let function_node = node.child_by_field_name("function")?;
    if selector_name(function_node, code)? != "Methods" {
        return None;
    }

    let selector_operand = function_node.child_by_field_name("operand")?;
    if selector_operand.kind() != "call_expression" {
        return None;
    }

    let inner_function = selector_operand.child_by_field_name("function")?;
    if selector_name(inner_function, code)? != "HandleFunc" {
        return None;
    }

    let inner_args = selector_operand
        .child_by_field_name("arguments")
        .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())?;
    let method_args = node
        .child_by_field_name("arguments")
        .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())?;
    if inner_args.len() < 2 || method_args.is_empty() {
        return None;
    }

    let method = parse_http_method_value(&evaluate_expression_node(method_args[0], code, globals))?;
    let path = evaluate_expression_node(inner_args[0], code, globals);
    let handler = normalize_whitespace(node_text(inner_args[1], code));
    Some((method, path, handler))
}

fn resolve_handler_callable(
    file_path: &str,
    handler_expr: &str,
    uri: &str,
    method: &HttpMethod,
    callable_lookup: &HashMap<String, Callable>,
    synthetic_callables: &mut Vec<ParsedCallable>,
    synthetic_hashes: &mut HashSet<String>,
) -> Callable {
    if let Some(actual) = lookup_callable(handler_expr, callable_lookup) {
        return actual;
    }

    let signature = format!("handler {} {}", format_http_method(method), uri);
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

fn lookup_callable(
    handler_expr: &str,
    callable_lookup: &HashMap<String, Callable>,
) -> Option<Callable> {
    let trimmed = handler_expr.trim();
    let simple = trimmed.split('.').next_back().unwrap_or(trimmed);
    callable_lookup
        .get(trimmed)
        .cloned()
        .or_else(|| callable_lookup.get(simple).cloned())
}

fn identify_restcall(file: &TypedFileRecord, call: &CallStatement) -> Option<RestCall> {
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

fn merged_scope_bindings(
    assignments: &HashMap<AssignmentKey, Assignment>,
    scope: &Scope,
) -> HashMap<String, String> {
    let mut values = scope_bindings(assignments, &Scope::Global);
    values.extend(scope_bindings(assignments, scope));
    values
}

fn scope_bindings(
    assignments: &HashMap<AssignmentKey, Assignment>,
    scope: &Scope,
) -> HashMap<String, String> {
    assignments
        .iter()
        .filter(|(key, _)| key.scope == *scope)
        .map(|(_, assignment)| (assignment.variable_name.clone(), assignment.value.clone()))
        .collect()
}

fn evaluate_expression_node(node: Node, code: &str, scope: &HashMap<String, String>) -> String {
    match node.kind() {
        "interpreted_string_literal" | "raw_string_literal" => {
            strip_quotes(node_text(node, code)).to_string()
        }
        "identifier" => scope
            .get(node_text(node, code))
            .cloned()
            .unwrap_or_else(|| node_text(node, code).to_string()),
        "parenthesized_expression" => node
            .named_child(0)
            .map(|child| evaluate_expression_node(child, code, scope))
            .unwrap_or_default(),
        "binary_expression" => {
            let children: Vec<Node> = node.named_children(&mut node.walk()).collect();
            if children.len() == 2 && node_text(node, code).contains('+') {
                return evaluate_expression_node(children[0], code, scope)
                    + &evaluate_expression_node(children[1], code, scope);
            }
            normalize_whitespace(node_text(node, code))
        }
        "call_expression" => evaluate_special_call(node, code, scope)
            .unwrap_or_else(|| normalize_whitespace(node_text(node, code))),
        "selector_expression" => scope
            .get(node_text(node, code))
            .cloned()
            .unwrap_or_else(|| normalize_whitespace(node_text(node, code))),
        "unary_expression" => node
            .named_child(0)
            .map(|child| evaluate_expression_node(child, code, scope))
            .unwrap_or_else(|| normalize_whitespace(node_text(node, code))),
        _ => normalize_whitespace(node_text(node, code)),
    }
}

fn evaluate_special_call(
    node: Node,
    code: &str,
    scope: &HashMap<String, String>,
) -> Option<String> {
    let function_node = node.child_by_field_name("function")?;
    let selector = selector_name(function_node, code)?;
    let arguments = node
        .child_by_field_name("arguments")
        .map(|args| args.named_children(&mut args.walk()).collect::<Vec<_>>())
        .unwrap_or_default();

    match selector.as_str() {
        "PathEscape" => arguments.first().map(|arg| {
            let value = node_text(*arg, code).trim().to_string();
            format!("{{{value}}}")
        }),
        "PathValue" => arguments.first().map(|arg| {
            let value = strip_quotes(node_text(*arg, code));
            format!("{{{value}}}")
        }),
        "TrimRight" => {
            if arguments.len() < 2 {
                return None;
            }
            let base = evaluate_expression_node(arguments[0], code, scope);
            let suffix = strip_quotes(node_text(arguments[1], code));
            Some(base.trim_end_matches(&suffix).to_string())
        }
        _ => None,
    }
}

fn evaluate_expression_text(expr: &str, scope: &HashMap<String, String>) -> String {
    if let Some(value) = scope.get(expr.trim()) {
        return value.clone();
    }

    if let Some(parts) = split_top_level_plus(expr) {
        return parts
            .into_iter()
            .map(|part| evaluate_expression_text(&part, scope))
            .collect::<String>();
    }

    let trimmed = expr.trim().trim_end_matches(',');
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('`') && trimmed.ends_with('`'))
    {
        return strip_quotes(trimmed).to_string();
    }
    if let Some(inner) = trimmed
        .strip_prefix("url.PathEscape(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return format!("{{{}}}", inner.trim());
    }
    if let Some(inner) = trimmed
        .strip_prefix("request.PathValue(")
        .or_else(|| trimmed.strip_prefix("PathValue("))
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return format!("{{{}}}", strip_quotes(inner.trim()));
    }
    if let Some(inner) = trimmed
        .strip_prefix("strings.TrimRight(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let parts = split_argument_text(inner);
        if parts.len() == 2 {
            let base = evaluate_expression_text(parts[0], scope);
            let suffix = strip_quotes(parts[1].trim());
            return base.trim_end_matches(&suffix).to_string();
        }
    }

    trimmed.to_string()
}

fn split_argument_text(input: &str) -> Vec<&str> {
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
            '(' if !in_string => depth += 1,
            ')' if !in_string => depth -= 1,
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

fn split_top_level_plus(input: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut quote = '\0';
    let mut start = 0usize;
    let mut seen_plus = false;

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
            '(' if !in_string => depth += 1,
            ')' if !in_string => depth -= 1,
            '+' if !in_string && depth == 0 => {
                parts.push(input[start..index].trim().to_string());
                start = index + 1;
                seen_plus = true;
            }
            _ => {}
        }
    }

    if !seen_plus {
        return None;
    }
    parts.push(input[start..].trim().to_string());
    Some(parts)
}

fn selector_name(node: Node, code: &str) -> Option<String> {
    if node.kind() == "identifier" {
        return Some(node_text(node, code).to_string());
    }
    if node.kind() != "selector_expression" {
        return None;
    }
    node.child_by_field_name("field")
        .map(|field| node_text(field, code).to_string())
        .or_else(|| {
            node.named_children(&mut node.walk())
                .last()
                .map(|field| node_text(field, code).to_string())
        })
}

fn web_route_method(selector: &str) -> Option<HttpMethod> {
    let method = selector
        .strip_prefix("Get")
        .map(|_| "GET")
        .or_else(|| selector.strip_prefix("Post").map(|_| "POST"))
        .or_else(|| selector.strip_prefix("Put").map(|_| "PUT"))
        .or_else(|| selector.strip_prefix("Delete").map(|_| "DELETE"))?;
    HttpMethod::from_str(method).ok()
}

fn parse_http_method_value(value: &str) -> Option<HttpMethod> {
    let trimmed = value.trim();
    let normalized = trimmed
        .strip_prefix("http.Method")
        .unwrap_or(trimmed)
        .trim_matches('"')
        .trim_matches('`');
    HttpMethod::from_str(&normalized.to_uppercase()).ok()
}

fn split_method_and_path(value: &str) -> Option<(HttpMethod, String)> {
    let (method, path) = value.split_once(' ')?;
    Some((HttpMethod::from_str(method).ok()?, path.to_string()))
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

fn simple_callable_name(signature: &str) -> Option<String> {
    signature
        .split('(')
        .next()
        .map(str::trim)
        .and_then(|prefix| prefix.split_whitespace().next_back())
        .map(|name| name.to_string())
}

fn walk_named(node: Node, visit: &mut impl FnMut(Node)) {
    visit(node);
    for child in node.named_children(&mut node.walk()) {
        walk_named(child, visit);
    }
}

fn node_text<'a>(node: Node, code: &'a str) -> &'a str {
    &code[node.start_byte()..node.end_byte()]
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
}
