use tree_sitter::Node;

use crate::{
    ast::{Expr, MethodAst, Stmt},
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

    let body_node = node
        .child_by_field_name("body")
        .ok_or(ParseError::FieldNotFound("body".to_string()))?;
    let body = parse_block(body_node, code)?;

    Ok(MethodAst { name, params, body })
}

fn parse_parameters(node: Node, source: &str) -> Result<Vec<String>, ParseError> {
    node.named_children(&mut node.walk())
        .filter(|n| n.kind() == "formal_parameter")
        .map(|param| {
            let name_node = param
                .child_by_field_name("name")
                .ok_or(ParseError::FieldNotFound("parameter name".to_string()))?;
            let name_text = name_node
                .utf8_text(source.as_bytes())
                .map_err(|err| ParseError::Utf8Encoding(err.to_string()))?
                .to_string();
            Ok(name_text)
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

        "local_variable_declaration" => {
            let declarator = node
                .child_by_field_name("declarator")
                .or_else(|| node.named_child(0))
                .ok_or(ParseError::FieldNotFound("variable declarator".to_string()))?;

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

            Ok(Stmt::Assign {
                name,
                value: value.ok_or(ParseError::FieldNotFound("variable value".to_string()))?,
            })
        }

        _ => Ok(Stmt::Empty),
    }
}

fn parse_expr(node: Node, source: &str) -> Result<Expr, ParseError> {
    match node.kind() {
        "string_literal" => {
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
