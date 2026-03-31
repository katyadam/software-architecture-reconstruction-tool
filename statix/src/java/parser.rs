use tree_sitter::Node;

use crate::{
    ast::{CallableAst, Expr, Parameter, Stmt},
    error::ParseError,
    util::{node_field_text, node_text},
};

pub fn find_method_nodes(root: Node) -> Vec<Node> {
    let mut methods = Vec::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() == "method_declaration" {
            methods.push(child);
        }
        methods.extend(find_method_nodes(child));
    }
    methods
}

pub fn parse_method(node: Node, code: &str) -> Result<CallableAst, ParseError> {
    let return_type = node_field_text(node, "type", code)?;
    let name = node_field_text(node, "name", code)?;

    let params_node = node
        .child_by_field_name("parameters")
        .ok_or(ParseError::FieldNotFound("parameters".to_string()))?;

    let params = parse_parameters(params_node, code)?;
    let params_types: Vec<String> = params.iter().map(|p| p.datatype.clone()).collect();

    let body = if let Some(body_node) = node.child_by_field_name("body") {
        parse_block(body_node, code)?
    } else {
        Vec::new()
    };
    Ok(CallableAst {
        return_type: return_type.clone(),
        header: return_type + " " + &name + &format!("({})", params_types.join(",")),
        params,
        body,
    })
}

fn parse_parameters(node: Node, source: &str) -> Result<Vec<Parameter>, ParseError> {
    node.named_children(&mut node.walk())
        .filter(|n| n.kind() == "formal_parameter")
        .map(|param| {
            let name = node_field_text(param, "name", source)?;
            let datatype = node_field_text(param, "type", source)?;
            Ok(Parameter::without_default_value(name, datatype))
        })
        .collect()
}

fn parse_block(node: Node, source: &str) -> Result<Vec<Stmt>, ParseError> {
    node.named_children(&mut node.walk())
        .map(|stmt| parse_stmt(stmt, source))
        .collect()
}

fn parse_stmt(node: Node, source: &str) -> Result<Stmt, ParseError> {
    match node.kind() {
        "return_statement" => {
            let expr = if let Some(expr_node) = node.named_child(0) {
                parse_expr(expr_node, source)?
            } else {
                Expr::Empty
            };
            Ok(Stmt::Return(expr))
        }

        "expression_statement" => {
            let child_node = node.child(0).ok_or(ParseError::UnsupportedNode(
                "expression_statement should have some child".to_string(),
            ))?;

            match child_node.kind() {
                "assignment_expression" => {
                    let name_node =
                        child_node
                            .child_by_field_name("left")
                            .ok_or(ParseError::FieldNotFound(
                                "assignment_expression -> left".to_string(),
                            ))?;
                    let name = source[name_node.start_byte()..name_node.end_byte()].to_string();

                    let value_node = child_node.child_by_field_name("right").ok_or(
                        ParseError::FieldNotFound("assignment_expression -> right".to_string()),
                    )?;

                    let value = parse_expr(value_node, source)?;

                    Ok(Stmt::Assignment { name, value })
                }
                _ => Ok(Stmt::Empty),
            }
        }
        "local_variable_declaration" => {
            let declarator = node
                .child_by_field_name("declarator")
                .or_else(|| node.named_child(0))
                .ok_or(ParseError::FieldNotFound("variable declarator".to_string()))?;

            let datatype = node_field_text(node, "type", source)?;
            let name = node_field_text(declarator, "name", source)?;

            let value = if let Some(value_node) = declarator.child_by_field_name("value") {
                parse_expr(value_node, source)?
            } else {
                Expr::Empty
            };

            Ok(Stmt::Declaration {
                name,
                dtype: datatype,
                value,
            })
        }

        "if_statement" => {
            let condition_node =
                node.child_by_field_name("condition")
                    .ok_or(ParseError::FieldNotFound(
                        "if statement should have condition".to_string(),
                    ))?;
            let condition = parse_expr(condition_node, source)?;

            let then_branch_node =
                node.child_by_field_name("consequence")
                    .ok_or(ParseError::FieldNotFound(
                        "if statement should have then block".to_string(),
                    ))?;
            let then_branch = parse_stmt_or_block(then_branch_node, source)?;

            let else_branch: Option<Vec<Stmt>> =
                if let Some(n) = node.child_by_field_name("alternative") {
                    let else_statements = parse_stmt_or_block(n, source)?;
                    Some(else_statements)
                } else {
                    None
                };

            Ok(Stmt::If {
                condition,
                then_branch,
                else_branch,
            })
        }

        "try_statement" => {
            let try_body_node = node
                .child_by_field_name("body")
                .ok_or(ParseError::FieldNotFound("try body".to_string()))?;
            let try_branch = parse_block(try_body_node, source)?;

            // Collect all catch_clause children (not a named field on try_statement).
            let mut catch_branch: Vec<Stmt> = Vec::new();
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if child.kind() == "catch_clause"
                    && let Some(body) = child.child_by_field_name("body")
                {
                    catch_branch.extend(parse_block(body, source)?);
                }
            }

            if catch_branch.is_empty() {
                // try with no catch: treat as a plain block
                Ok(Stmt::Empty)
            } else {
                Ok(Stmt::TryCatch {
                    try_branch,
                    catch_branch,
                })
            }
        }

        _ => Ok(Stmt::Empty),
    }
}

fn parse_stmt_or_block(node: Node, source: &str) -> Result<Vec<Stmt>, ParseError> {
    if node.kind() == "block" {
        parse_block(node, source)
    } else {
        Ok(vec![parse_stmt(node, source)?])
    }
}

fn parse_expr(node: Node, source: &str) -> Result<Expr, ParseError> {
    match node.kind() {
        "true" => Ok(Expr::Literal("true".to_string())),
        "false" => Ok(Expr::Literal("false".to_string())),
        "parenthesized_expression" => {
            let inner_node = node.named_child(0).ok_or(ParseError::FieldNotFound(
                "parenthesized expression content".to_string(),
            ))?;
            parse_expr(inner_node, source)
        }

        "string_literal" => Ok(Expr::Literal(crate::strings::strip_quotes(&node_text(
            node, source,
        )?))),

        "decimal_integer_literal" => Ok(Expr::Literal(crate::strings::strip_quotes(&node_text(
            node, source,
        )?))),

        "identifier" => Ok(Expr::Var(node_text(node, source)?)),

        "binary_expression" => {
            let op = node_field_text(node, "operator", source)?;

            if op == "+" {
                let left_node = node
                    .child_by_field_name("left")
                    .ok_or(ParseError::FieldNotFound("left operand".to_string()))?;
                let right_node = node
                    .child_by_field_name("right")
                    .ok_or(ParseError::FieldNotFound("right operand".to_string()))?;
                let left = parse_expr(left_node, source)?;
                let right = parse_expr(right_node, source)?;
                Ok(Expr::Concat(Box::new(left), Box::new(right)))
            } else {
                Ok(Expr::Empty)
            }
        }

        "method_invocation" => {
            let name = node_field_text(node, "name", source)?;

            let args_node = node
                .child_by_field_name("arguments")
                .ok_or(ParseError::FieldNotFound("method arguments".to_string()))?;

            let mut args = Vec::new();
            for arg in args_node.named_children(&mut args_node.walk()) {
                args.push(parse_expr(arg, source)?);
            }

            Ok(Expr::Call { name, args })
        }

        _ => Ok(Expr::Empty),
    }
}
