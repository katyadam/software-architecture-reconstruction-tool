use std::collections::HashMap;

use models::{
    Argument, Assignment, AssignmentKey, CallStatement, Callable, Namespace, Parameter,
    ParsedCallable, Scope, ir::ast::CallableAst,
};
use statix::strings::{hash_text, normalize_whitespace};
use tree_sitter::Node;

use super::shared::{
    GoNodeKind, evaluate_expression_node, go_node_kind, node_text, scope_bindings,
    simple_callable_name, walk_named,
};

pub(super) fn collect_global_assignments(
    root: Node,
    code: &str,
    assignments: &mut HashMap<AssignmentKey, Assignment>,
) {
    for child in root.named_children(&mut root.walk()) {
        match go_node_kind(child) {
            GoNodeKind::ConstDeclaration | GoNodeKind::VarDeclaration => {
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
        if !matches!(
            go_node_kind(child),
            GoNodeKind::FunctionDeclaration | GoNodeKind::MethodDeclaration
        ) {
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
    let parameters = node
        .child_by_field_name("parameters")
        .map(|params| parse_parameters(params, code))
        .unwrap_or_default();
    let body_hash = hash_text(node_text(node, code));
    let namespace = if go_node_kind(node) == GoNodeKind::MethodDeclaration {
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
            parameters,
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
        if matches!(
            go_node_kind(child),
            GoNodeKind::ConstSpec | GoNodeKind::VarSpec
        ) {
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
        match go_node_kind(child) {
            GoNodeKind::Identifier => identifiers.push(child),
            GoNodeKind::ExpressionList => {
                values.extend(child.named_children(&mut child.walk()));
            }
            GoNodeKind::InterpretedStringLiteral
            | GoNodeKind::RawStringLiteral
            | GoNodeKind::BinaryExpression
            | GoNodeKind::CallExpression
            | GoNodeKind::SelectorExpression
            | GoNodeKind::ParenthesizedExpression => values.push(child),
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
    walk_named(body, &mut |node| match go_node_kind(node) {
        GoNodeKind::ShortVarDeclaration | GoNodeKind::AssignmentStatement => {
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
        GoNodeKind::ConstDeclaration | GoNodeKind::VarDeclaration => {
            collect_declaration_assignments(node, code, scope.clone(), assignments);
            scope_values.extend(scope_bindings(assignments, &scope));
        }
        _ if node.kind() == "for_statement" => {
            if let Some((name, source)) = parse_range_binding(node_text(node, code)) {
                let value = scope_values.get(&source).cloned().unwrap_or(source);
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
        _ => {}
    });
}

fn assignment_pairs<'a>(node: Node<'a>, code: &'a str) -> Vec<(String, Node<'a>)> {
    let mut left_values = Vec::new();
    let mut right_values = Vec::new();
    let mut seen_right = false;

    for child in node.named_children(&mut node.walk()) {
        match go_node_kind(child) {
            GoNodeKind::ExpressionList => {
                let values: Vec<Node> = child.named_children(&mut child.walk()).collect();
                if !seen_right {
                    left_values.extend(values);
                    seen_right = true;
                } else {
                    right_values.extend(values);
                }
            }
            _ => {
                if !seen_right && go_node_kind(child) == GoNodeKind::Identifier {
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
            if go_node_kind(left) != GoNodeKind::Identifier {
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
        if go_node_kind(node) != GoNodeKind::CallExpression {
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

fn parse_parameters(node: Node, code: &str) -> Vec<Parameter> {
    let raw = node_text(node, code)
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')');
    if raw.is_empty() {
        return vec![];
    }

    split_top_level_commas(raw)
        .into_iter()
        .flat_map(parse_parameter_group)
        .collect()
}

fn parse_parameter_group(group: String) -> Vec<Parameter> {
    let group = group.trim();
    if group.is_empty() {
        return vec![];
    }

    let mut parts = group.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 2 {
        return vec![];
    }

    let datatype = parts.pop().unwrap_or_default().to_string();
    let names = parts.join(" ");
    names
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| Parameter::new(name.to_string(), Some(datatype.clone()), None))
        .collect()
}

fn split_top_level_commas(input: &str) -> Vec<String> {
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
                parts.push(input[start..index].trim().to_string());
                start = index + 1;
            }
            _ => {}
        }
    }

    let tail = input[start..].trim();
    if !tail.is_empty() {
        parts.push(tail.to_string());
    }
    parts
}

fn parse_range_binding(loop_text: &str) -> Option<(String, String)> {
    let header = loop_text.split('{').next()?.trim();
    let (left, right) = header
        .split_once(":= range")
        .or_else(|| header.split_once("= range"))?;
    let name = left
        .split(',')
        .next_back()?
        .trim()
        .trim_start_matches('*')
        .to_string();
    let source = right.trim().to_string();
    if name.is_empty() || source.is_empty() {
        return None;
    }
    Some((name, source))
}
