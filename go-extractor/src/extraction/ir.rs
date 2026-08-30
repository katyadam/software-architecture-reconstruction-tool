use std::collections::HashMap;

use models::{
    Argument, Assignment, AssignmentKey, CallStatement, Callable, Namespace, Parameter,
    ParsedCallable, Scope,
    ir::ast::{CallableAst, Expr, Stmt},
};
use statix::strings::{hash_text, normalize_whitespace, strip_quotes};
use tree_sitter::Node;

use super::shared::{
    evaluate_expression_node, node_text, scope_bindings, simple_callable_name, walk_named,
};

pub(super) fn collect_global_assignments(
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

pub(super) fn collect_callable_ir(
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
    let parameters = node
        .child_by_field_name("parameters")
        .map(|parameters| parse_callable_parameters(parameters, code))
        .unwrap_or_default();
    let return_type = node
        .child_by_field_name("result")
        .map(|result| normalize_whitespace(node_text(result, code)));
    let ast = node
        .child_by_field_name("body")
        .map(|body| parse_callable_body(body, code))
        .unwrap_or_else(empty_callable_ast);

    ParsedCallable {
        metadata: Callable {
            name: function_name,
            signature,
            namespace,
            parameters,
            return_type,
            is_async: false,
            is_constructor: false,
            hash: body_hash,
            file_path: file_path.to_string(),
        },
        ast,
    }
}

fn empty_callable_ast() -> CallableAst {
    CallableAst {
        statements: vec![],
        nested: vec![],
    }
}

fn parse_callable_parameters(node: Node, code: &str) -> Vec<Parameter> {
    let mut parameters = Vec::new();
    for declaration in node.named_children(&mut node.walk()) {
        if declaration.kind() != "parameter_declaration" {
            continue;
        }
        let datatype = declaration
            .child_by_field_name("type")
            .map(|kind| normalize_whitespace(node_text(kind, code)));
        let type_start = declaration
            .child_by_field_name("type")
            .map(|kind| kind.start_byte())
            .unwrap_or(declaration.end_byte());
        for name in declaration
            .named_children(&mut declaration.walk())
            .filter(|child| child.kind() == "identifier" && child.start_byte() < type_start)
        {
            parameters.push(Parameter {
                name: node_text(name, code).to_string(),
                datatype: datatype.clone(),
                initial_value: None,
            });
        }
    }
    parameters
}

fn parse_callable_body(node: Node, code: &str) -> CallableAst {
    CallableAst {
        statements: parse_statements(node, code),
        nested: vec![],
    }
}

fn parse_statements(node: Node, code: &str) -> Vec<Stmt> {
    let mut statements = Vec::new();
    for child in node.named_children(&mut node.walk()) {
        match child.kind() {
            "statement_list" => statements.extend(parse_statements(child, code)),
            "short_var_declaration" | "assignment_statement" => {
                for (name, value) in assignment_pairs(child, code) {
                    let value = parse_expression(value, code);
                    if child.kind() == "short_var_declaration" {
                        statements.push(Stmt::Declaration {
                            name,
                            dtype: None,
                            value,
                        });
                    } else {
                        statements.push(Stmt::Assignment { name, value });
                    }
                }
            }
            "return_statement" => {
                let value = child
                    .named_child(0)
                    .map(|value| parse_expression(value, code))
                    .unwrap_or(Expr::Empty);
                statements.push(Stmt::Return(value));
            }
            _ => {}
        }
    }
    statements
}

fn parse_expression(node: Node, code: &str) -> Expr {
    match node.kind() {
        "interpreted_string_literal" | "raw_string_literal" => {
            Expr::Literal(strip_quotes(node_text(node, code)).to_string())
        }
        "int_literal" | "float_literal" | "true" | "false" => {
            Expr::Literal(node_text(node, code).to_string())
        }
        "identifier" => Expr::Var(node_text(node, code).to_string()),
        "composite_literal" => parse_composite_literal(node, code),
        "selector_expression" => {
            let Some(object) = node.child_by_field_name("operand") else {
                return Expr::Empty;
            };
            let Some(field) = node.child_by_field_name("field") else {
                return Expr::Empty;
            };
            Expr::Attr {
                object: Box::new(parse_expression(object, code)),
                field: node_text(field, code).to_string(),
            }
        }
        "binary_expression" => {
            let children = node.named_children(&mut node.walk()).collect::<Vec<_>>();
            if children.len() == 2 && node_text(node, code).contains('+') {
                Expr::Concat(
                    Box::new(parse_expression(children[0], code)),
                    Box::new(parse_expression(children[1], code)),
                )
            } else {
                Expr::Empty
            }
        }
        "parenthesized_expression" | "unary_expression" => node
            .named_child(0)
            .map(|child| parse_expression(child, code))
            .unwrap_or(Expr::Empty),
        "literal_element" => node
            .named_child(0)
            .map(|child| parse_expression(child, code))
            .unwrap_or(Expr::Empty),
        "expression_list" => node
            .named_child(0)
            .map(|child| parse_expression(child, code))
            .unwrap_or(Expr::Empty),
        "index_expression" => {
            let collection = node
                .child_by_field_name("operand")
                .or_else(|| node.named_child(0))
                .map(|child| parse_expression(child, code))
                .unwrap_or(Expr::Empty);
            let index = node
                .child_by_field_name("index")
                .or_else(|| node.named_child(1))
                .map(|child| parse_expression(child, code))
                .unwrap_or(Expr::Empty);
            Expr::Call {
                name: "index".to_string(),
                receiver: Some(Box::new(collection)),
                args: vec![index],
            }
        }
        "call_expression" => parse_call_expression(node, code),
        _ => Expr::Empty,
    }
}

fn parse_call_expression(node: Node, code: &str) -> Expr {
    let Some(function) = node.child_by_field_name("function") else {
        return Expr::Empty;
    };
    let args = node
        .child_by_field_name("arguments")
        .map(|arguments| {
            arguments
                .named_children(&mut arguments.walk())
                .map(|argument| parse_expression(argument, code))
                .collect()
        })
        .unwrap_or_default();

    if function.kind() == "selector_expression" {
        let receiver = function
            .child_by_field_name("operand")
            .map(|operand| parse_expression(operand, code));
        let name = function
            .child_by_field_name("field")
            .map(|field| node_text(field, code).to_string())
            .unwrap_or_default();
        return Expr::Call {
            name,
            receiver: receiver.map(Box::new),
            args,
        };
    }

    Expr::Call {
        name: node_text(function, code).to_string(),
        receiver: None,
        args,
    }
}

fn parse_composite_literal(node: Node, code: &str) -> Expr {
    let mut type_name = None;
    let mut fields = Vec::new();

    for child in node.named_children(&mut node.walk()) {
        match child.kind() {
            "literal_value" => collect_composite_literal_fields(child, code, &mut fields),
            _ if type_name.is_none() => {
                type_name = Some(normalize_whitespace(node_text(child, code)));
            }
            _ => {}
        }
    }

    Expr::StructLiteral { type_name, fields }
}

fn collect_composite_literal_fields(node: Node, code: &str, fields: &mut Vec<(String, Expr)>) {
    for child in node.named_children(&mut node.walk()) {
        if child.kind() == "keyed_element" {
            let mut cursor = child.walk();
            let mut children = child.named_children(&mut cursor);
            let Some(key) = children.next() else {
                continue;
            };
            let Some(value) = children.next() else {
                continue;
            };
            fields.push((
                normalize_whitespace(node_text(key, code)),
                parse_expression(value, code),
            ));
            continue;
        }

        collect_composite_literal_fields(child, code, fields);
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
            | "composite_literal"
            | "selector_expression"
            | "unary_expression"
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
        scope_values.insert(variable_name.clone(), value);
        if let Some(value_node) = values.get(index)
            && node_text(*value_node, code)
                .trim_start()
                .starts_with("map[")
        {
            collect_map_literal_entries(
                variable_name,
                *value_node,
                code,
                scope.clone(),
                &scope_values,
                assignments,
            );
        }
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
                scope_values.insert(name.clone(), value);
                if let Some(root) = selector_root(&name)
                    && scope_bindings(assignments, &Scope::Global).contains_key(root)
                {
                    assignments.insert(
                        AssignmentKey {
                            scope: Scope::Global,
                            variable_name: name.clone(),
                        },
                        Assignment {
                            variable_name: name.clone(),
                            variable_type: "".to_string(),
                            value: scope_values[&name].clone(),
                        },
                    );
                }
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
                if !seen_right && matches!(child.kind(), "identifier" | "selector_expression") {
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
            if !matches!(left.kind(), "identifier" | "selector_expression") {
                return None;
            }
            Some((node_text(left, code).to_string(), right))
        })
        .collect()
}

fn collect_map_literal_entries(
    variable_name: String,
    value_node: Node,
    code: &str,
    scope: Scope,
    scope_values: &HashMap<String, String>,
    assignments: &mut HashMap<AssignmentKey, Assignment>,
) {
    let mut value_cursor = value_node.walk();
    for child in value_node.named_children(&mut value_cursor) {
        let mut entry_cursor = child.walk();
        for entry in child.named_children(&mut entry_cursor) {
            if entry.kind() != "keyed_element" {
                continue;
            }

            let mut keyed_cursor = entry.walk();
            let mut children = entry.named_children(&mut keyed_cursor);
            let Some(key_node) = children.next() else {
                continue;
            };
            let Some(value_node) = children.next() else {
                continue;
            };
            let key = evaluate_expression_node(key_node, code, scope_values);
            let value = evaluate_expression_node(value_node, code, scope_values);
            let entry_name = format!("{variable_name}[{key}]");

            assignments.insert(
                AssignmentKey {
                    scope: scope.clone(),
                    variable_name: entry_name.clone(),
                },
                Assignment {
                    variable_name: entry_name,
                    variable_type: "".to_string(),
                    value,
                },
            );
        }
    }
}

fn selector_root(selector: &str) -> Option<&str> {
    selector.split_once('.').map(|(root, _)| root)
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
