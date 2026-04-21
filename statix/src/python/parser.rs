use std::collections::HashSet;

use models::{
    Parameter,
    ir::ast::{CallableAst, Expr, NestedRef, Stmt},
};
use tree_sitter::Node;

use crate::{
    error::ParseError,
    python::matcher::python_convert_full_header_to_mangled_name,
    strings,
    util::{node_field_text, node_text},
};

pub(crate) fn find_function_nodes(root: Node) -> Vec<Node> {
    let mut functions = Vec::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() == "function_definition" {
            functions.push(child);
        }
        functions.extend(find_function_nodes(child));
    }
    functions
}

pub(crate) fn parse_python_function(node: Node, code: &str) -> Result<CallableAst, ParseError> {
    let params_node = node
        .child_by_field_name("parameters")
        .ok_or(ParseError::FieldNotFound("parameters".to_string()))?;

    let params = parse_parameters(params_node, code)?;

    let body_node = node
        .child_by_field_name("body")
        .ok_or(ParseError::FieldNotFound("body".to_string()))?;

    let mut declared_vars = HashSet::new();
    for param in &params {
        declared_vars.insert(param.name.clone());
    }

    let body = parse_block(body_node, code, &mut declared_vars)?;
    let nested = collect_nested_refs(body_node, code, &declared_vars);

    Ok(CallableAst {
        statements: body,
        nested,
    })
}

pub(crate) fn parse_callable_parameters(node: Node, source: &str) -> Vec<Parameter> {
    let mut params = Vec::new();
    let mut cursor = node.walk();

    for param in node.named_children(&mut cursor) {
        let name: String;
        let mut datatype = None;
        let mut initial_value = None;

        match param.kind() {
            "identifier" => {
                name = match node_text(param, source) {
                    Ok(n) => n,
                    Err(_) => continue,
                };
            }
            "typed_parameter" => {
                let name_node = match param.child(0) {
                    Some(n) => n,
                    None => continue,
                };
                name = match node_text(name_node, source) {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                datatype = node_field_text(param, "type", source).ok();
            }
            "default_parameter" => {
                name = match node_field_text(param, "name", source) {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                initial_value = node_field_text(param, "value", source).ok();
            }
            _ => continue,
        }
        params.push(Parameter {
            name,
            datatype,
            initial_value,
        });
    }
    params
}

fn parse_parameters(node: Node, source: &str) -> Result<Vec<Parameter>, ParseError> {
    Ok(parse_callable_parameters(node, source))
}

fn parse_block(
    node: Node,
    source: &str,
    scope_vars: &mut HashSet<String>,
) -> Result<Vec<Stmt>, ParseError> {
    let mut stmts = Vec::new();
    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        match child.kind() {
            // Difference between Python and Java is the declaration style. For Python we are using Declare-on-Write
            "expression_statement" => {
                let inner = child.named_child(0).ok_or(ParseError::FieldNotFound(
                    "expression_statement inner".to_string(),
                ))?;
                if inner.kind() == "assignment" {
                    let name = node_field_text(inner, "left", source)?;

                    // This is really weird Python thing:
                    // a = 5
                    // a: int
                    // This means |a| gets 5 and in the next line gets datatype int, but still remains 5
                    // Tree-sitter takes declaration as assignment without RHS
                    if let Some(right_node) = inner.child_by_field_name("right") {
                        let value = parse_expr(right_node, source)?;
                        if scope_vars.contains(&name) {
                            stmts.push(Stmt::Assignment { name, value });
                        } else {
                            scope_vars.insert(name.clone());
                            stmts.push(Stmt::Declaration {
                                name,
                                dtype: None,
                                value,
                            });
                        }
                    } else {
                        stmts.push(Stmt::Declaration {
                            name,
                            dtype: None,
                            value: Expr::Empty,
                        });
                    }
                }
            }
            "if_statement" => {
                stmts.push(parse_if(child, source, scope_vars)?);
            }
            "try_statement" => {
                stmts.extend(parse_try(child, source, scope_vars)?);
            }
            "return_statement" => {
                let expr = child
                    .named_child(0)
                    .map_or(Ok(Expr::Empty), |n| parse_expr(n, source))?;
                stmts.push(Stmt::Return(expr));
            }
            _ => {}
        }
    }
    Ok(stmts)
}

fn parse_expr(node: Node, source: &str) -> Result<Expr, ParseError> {
    match node.kind() {
        "string" => Ok(Expr::Literal(crate::strings::clean_python_string(
            &node_text(node, source)?,
        ))),
        "integer" => Ok(Expr::Literal(node_text(node, source)?)),
        "true" => Ok(Expr::Literal("True".to_string())),
        "false" => Ok(Expr::Literal("False".to_string())),
        "identifier" => Ok(Expr::Var(node_text(node, source)?)),

        "attribute" => {
            let object_node = node
                .child_by_field_name("object")
                .ok_or(ParseError::FieldNotFound("attribute object".to_string()))?;
            let field = node_field_text(node, "attribute", source)?;
            let object = parse_expr(object_node, source)?;
            Ok(Expr::Attr {
                object: Box::new(object),
                field,
            })
        }

        "call" => {
            let function_node = node
                .child_by_field_name("function")
                .ok_or(ParseError::FieldNotFound("call function".to_string()))?;
            let args_node = node
                .child_by_field_name("arguments")
                .ok_or(ParseError::FieldNotFound("call arguments".to_string()))?;

            // Method call on an attribute: `receiver.method(args)`.
            // Encode as Call { name: method, args: [receiver, ...positional_args] }
            // so the evaluator can apply identity semantics for string methods (rstrip, etc.).
            if function_node.kind() == "attribute" {
                let receiver_node = function_node
                    .child_by_field_name("object")
                    .ok_or(ParseError::FieldNotFound("method receiver".to_string()))?;
                let method_name = node_field_text(function_node, "attribute", source)?;
                let receiver = parse_expr(receiver_node, source)?;
                let mut args = Vec::new();
                for arg in args_node.named_children(&mut args_node.walk()) {
                    args.push(parse_expr(arg, source)?);
                }
                return Ok(Expr::Call {
                    name: method_name,
                    receiver: Some(Box::new(receiver)),
                    args,
                });
            }

            let name = node_text(function_node, source)?;
            let mut args = Vec::new();
            for arg in args_node.named_children(&mut args_node.walk()) {
                args.push(parse_expr(arg, source)?);
            }
            Ok(Expr::Call {
                name,
                receiver: None,
                args,
            })
        }

        "binary_operator" => {
            let left_node = node
                .child_by_field_name("left")
                .ok_or(ParseError::FieldNotFound("left operand".to_string()))?;
            let right_node = node
                .child_by_field_name("right")
                .ok_or(ParseError::FieldNotFound("right operand".to_string()))?;
            let left = parse_expr(left_node, source)?;
            let right = parse_expr(right_node, source)?;
            let operator = node_field_text(node, "operator", source)?;

            if operator == "+" {
                Ok(Expr::Concat(Box::new(left), Box::new(right)))
            } else {
                Ok(Expr::Empty)
            }
        }
        _ => Ok(Expr::Empty),
    }
}

fn parse_if(
    node: Node,
    source: &str,
    scope_vars: &mut HashSet<String>,
) -> Result<Stmt, ParseError> {
    let condition_node = node
        .child_by_field_name("condition")
        .ok_or(ParseError::FieldNotFound("if condition".to_string()))?;
    let condition = parse_expr(condition_node, source)?;

    let consequence_node = node
        .child_by_field_name("consequence")
        .ok_or(ParseError::FieldNotFound("if consequence".to_string()))?;
    let then_branch = parse_block(consequence_node, source, scope_vars)?;

    let mut else_branch = None;
    if let Some(alt_node) = node.child_by_field_name("alternative") {
        if alt_node.kind() == "if_statement" || alt_node.kind() == "elif_clause" {
            // elif_clause has the same condition/consequence fields as if_statement
            else_branch = Some(vec![parse_if(alt_node, source, scope_vars)?]);
        } else {
            // else_clause: actual statements are in its `body` field, not on the node itself
            let body_node = alt_node
                .child_by_field_name("body")
                .ok_or(ParseError::FieldNotFound("else body".to_string()))?;
            else_branch = Some(parse_block(body_node, source, scope_vars)?);
        }
    }

    Ok(Stmt::If {
        condition,
        then_branch,
        else_branch,
    })
}

fn parse_try(
    node: Node,
    source: &str,
    scope_vars: &mut HashSet<String>,
) -> Result<Vec<Stmt>, ParseError> {
    let try_body_node = node
        .child_by_field_name("body")
        .ok_or(ParseError::FieldNotFound("try block".to_string()))?;

    // Snapshot scope before the try body so the except branch uses the same
    // baseline — variables declared in the try body are not in scope when an
    // exception is raised, so both branches should declare them independently.
    let scope_before = scope_vars.clone();
    let try_branch = parse_block(try_body_node, source, scope_vars)?;

    // Parse all except_clause children using the pre-try scope so variables
    // first assigned in the try body produce Declaration (not Assignment) in
    // the except branch, enabling the evaluator to join them correctly.
    let mut except_scope = scope_before;
    let mut except_stmts: Vec<Stmt> = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "except_clause" {
            // except_clause has no named "body" field in the tree-sitter Python grammar;
            // the suite is always the last named child (block or simple_statement).
            let count: u32 = child.named_child_count() as u32;
            if count > 0
                && let Some(body) = child.named_child(count - 1)
            {
                except_stmts.extend(parse_block(body, source, &mut except_scope)?);
            }
        }
    }

    // Merge all vars from both branches into the outer scope.
    scope_vars.extend(except_scope);

    if except_stmts.is_empty() {
        Ok(try_branch)
    } else {
        Ok(vec![Stmt::TryCatch {
            try_branch,
            catch_branch: except_stmts,
        }])
    }
}

/// Walk `node` (a function body block) and collect `NestedRef`s for every
/// `function_definition` or `decorated_definition` found as a direct or
/// indirect child, but NOT descending into those inner functions themselves.
/// `outer_vars` is the set of names declared in the enclosing function's scope;
/// it becomes the `captured` list on each `NestedRef`.
pub(crate) fn collect_nested_refs(
    node: Node,
    source: &str,
    outer_vars: &HashSet<String>,
) -> Vec<NestedRef> {
    let mut nested = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "function_definition" => {
                if let Some(nr) = make_nested_ref(child, source, outer_vars) {
                    nested.push(nr);
                }
            }
            "decorated_definition" => {
                if let Some(func_node) = child.child_by_field_name("definition")
                    && func_node.kind() == "function_definition"
                    && let Some(nr) = make_nested_ref(func_node, source, outer_vars)
                {
                    nested.push(nr);
                }
            }
            _ => {
                nested.extend(collect_nested_refs(child, source, outer_vars));
            }
        }
    }
    nested
}

/// Build a `NestedRef` for a single `function_definition` node.
fn make_nested_ref(
    func_node: Node,
    source: &str,
    outer_vars: &HashSet<String>,
) -> Option<NestedRef> {
    let name = func_node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())?;
    let params = func_node
        .child_by_field_name("parameters")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .unwrap_or("()");
    let return_type = func_node
        .child_by_field_name("return_type")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .unwrap_or("Any");

    let full_header = format!("{}{} -> {}", name, params, return_type);
    let key = python_convert_full_header_to_mangled_name(&full_header);

    let func_text = func_node.utf8_text(source.as_bytes()).unwrap_or("");
    let hash = strings::hash_text(func_text);

    Some(NestedRef {
        key,
        hash,
        captured: outer_vars.iter().cloned().collect(),
    })
}
