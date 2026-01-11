use tree_sitter::Node;

use crate::{
    ast::{Expr, MethodAst, Parameter, Stmt},
    error::ParseError,
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

pub fn parse_method(node: Node, code: &str) -> Result<MethodAst, ParseError> {
    let return_type = node
        .child_by_field_name("type")
        .ok_or(ParseError::FieldNotFound("tyoe".to_string()))?
        .utf8_text(code.as_bytes())
        .map_err(|err| ParseError::Utf8Encoding(err.to_string()))?
        .to_string();

    let name = node
        .child_by_field_name("name")
        .ok_or(ParseError::FieldNotFound("name".to_string()))?
        .utf8_text(code.as_bytes())
        .map_err(|err| ParseError::Utf8Encoding(err.to_string()))?
        .to_string();

    let params_node = node
        .child_by_field_name("parameters")
        .ok_or(ParseError::FieldNotFound("parameters".to_string()))?;

    let params = parse_parameters(params_node, code)?;
    let params_types = parse_parameters_to_datatypes(params_node, code)?;

    let body_node = node
        .child_by_field_name("body")
        .ok_or(ParseError::FieldNotFound("body".to_string()))?;

    let body = parse_block(body_node, code)?;
    Ok(MethodAst {
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
            let name = param
                .child_by_field_name("name")
                .ok_or(ParseError::FieldNotFound("parameter name".to_string()))?
                .utf8_text(source.as_bytes())
                .map_err(|err| ParseError::Utf8Encoding(err.to_string()))?
                .to_string();

            let datatype = param
                .child_by_field_name("type")
                .ok_or(ParseError::FieldNotFound("parameter name".to_string()))?
                .utf8_text(source.as_bytes())
                .map_err(|err| ParseError::Utf8Encoding(err.to_string()))?
                .to_string();

            Ok(Parameter::new(name, datatype))
        })
        .collect()
}

fn parse_parameters_to_datatypes(node: Node, source: &str) -> Result<Vec<String>, ParseError> {
    node.named_children(&mut node.walk())
        .filter(|n| n.kind() == "formal_parameter")
        .map(|param| {
            let type_node = param
                .child_by_field_name("type")
                .ok_or(ParseError::FieldNotFound("parameter type".to_string()))?;
            let type_text = type_node
                .utf8_text(source.as_bytes())
                .map_err(|err| ParseError::Utf8Encoding(err.to_string()))?
                .to_string();
            Ok(type_text)
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
            let expr_node = node
                .named_child(0)
                .ok_or(ParseError::FieldNotFound("return expression".to_string()))?;
            let expr = parse_expr(expr_node, source)?;
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

            let datatype = node
                .child_by_field_name("type")
                .ok_or(ParseError::FieldNotFound(
                    "variable declaration datatype".to_string(),
                ))?
                .utf8_text(source.as_bytes())
                .map_err(|err| ParseError::Utf8Encoding(err.to_string()))?
                .to_string();

            let name_node = declarator
                .child_by_field_name("name")
                .ok_or(ParseError::FieldNotFound("variable name".to_string()))?;

            let name = name_node
                .utf8_text(source.as_bytes())
                .map_err(|err| ParseError::Utf8Encoding(err.to_string()))?
                .to_string();

            let value = if let Some(value_node) = declarator.child_by_field_name("value") {
                Some(parse_expr(value_node, source)?)
            } else {
                None
            };

            Ok(Stmt::Declaration {
                name,
                dtype: datatype,
                value: value.ok_or(ParseError::FieldNotFound("variable value".to_string()))?,
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

        "string_literal" => {
            let text = node
                .utf8_text(source.as_bytes())
                .map_err(|err| ParseError::Utf8Encoding(err.to_string()))?;
            Ok(Expr::Literal(strip_quotes(text)))
        }

        "decimal_integer_literal" => {
            let text = node
                .utf8_text(source.as_bytes())
                .map_err(|err| ParseError::Utf8Encoding(err.to_string()))?;
            Ok(Expr::Literal(strip_quotes(text)))
        }

        "identifier" => {
            let name = node
                .utf8_text(source.as_bytes())
                .map_err(|err| ParseError::Utf8Encoding(err.to_string()))?
                .to_string();
            Ok(Expr::Var(name))
        }

        "binary_expression" => {
            let op_node = node
                .child_by_field_name("operator")
                .ok_or(ParseError::FieldNotFound("operator".to_string()))?;
            let op = op_node
                .utf8_text(source.as_bytes())
                .map_err(|err| ParseError::Utf8Encoding(err.to_string()))?;

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
            let name_node = node
                .child_by_field_name("name")
                .ok_or(ParseError::FieldNotFound("method name".to_string()))?;
            let name = name_node
                .utf8_text(source.as_bytes())
                .map_err(|err| ParseError::Utf8Encoding(err.to_string()))?
                .to_string();

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

fn strip_quotes(s: &str) -> String {
    s.trim_matches('"').to_string()
}
