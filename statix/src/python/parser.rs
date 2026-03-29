use std::collections::HashSet;

use tree_sitter::Node;

use crate::{
    ast::{CallableAst, Expr, Parameter, Stmt},
    error::ParseError,
};

// TODO: find better ways to look for all functions, this iterates over all possible nodes in the tree
pub fn find_function_nodes(root: Node) -> Vec<Node> {
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

pub fn parse_python_function(node: Node, code: &str) -> Result<CallableAst, ParseError> {
    let name = node
        .child_by_field_name("name")
        .ok_or(ParseError::FieldNotFound("name".to_string()))?
        .utf8_text(code.as_bytes())
        .map_err(|err| ParseError::Utf8Encoding(err.to_string()))?
        .to_string();

    let return_type = if let Some(rt_node) = node.child_by_field_name("return_type") {
        rt_node
            .utf8_text(code.as_bytes())
            .map_err(|err| ParseError::Utf8Encoding(err.to_string()))?
            .to_string()
    } else {
        "Any".to_string()
    };

    let params_node = node
        .child_by_field_name("parameters")
        .ok_or(ParseError::FieldNotFound("parameters".to_string()))?;

    let params = parse_parameters(params_node, code)?;
    let param_types: Vec<String> = params.iter().map(|p| p.datatype.clone()).collect();

    let body_node = node
        .child_by_field_name("body")
        .ok_or(ParseError::FieldNotFound("body".to_string()))?;

    let mut declared_vars = HashSet::new();
    for param in &params {
        declared_vars.insert(param.name.clone());
    }

    let body = parse_block(body_node, code, &mut declared_vars)?;

    Ok(CallableAst {
        return_type: return_type.clone(),
        header: return_type + " " + &name + &format!("({})", param_types.join(",")),
        params,
        body,
    })
}

fn parse_parameters(node: Node, source: &str) -> Result<Vec<Parameter>, ParseError> {
    let mut params = Vec::new();
    let mut cursor = node.walk();

    for param in node.named_children(&mut cursor) {
        let name: String;
        let mut datatype = "Any".to_string();
        let mut default_value = None;

        match param.kind() {
            "identifier" => {
                name = param.utf8_text(source.as_bytes()).unwrap().to_string();
            }
            "typed_parameter" => {
                let name_node = param.child(0).unwrap();
                let type_node = param.child_by_field_name("type").unwrap();
                name = name_node.utf8_text(source.as_bytes()).unwrap().to_string();
                datatype = type_node.utf8_text(source.as_bytes()).unwrap().to_string();
            }
            "default_parameter" => {
                let name_node = param.child_by_field_name("name").unwrap();
                let value_node = param.child_by_field_name("value").unwrap();
                name = name_node.utf8_text(source.as_bytes()).unwrap().to_string();
                default_value = Some(value_node.utf8_text(source.as_bytes()).unwrap().to_string());
            }
            _ => continue,
        }
        params.push(Parameter::new(name, datatype, default_value));
    }
    Ok(params)
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
                let inner = child.named_child(0).unwrap();
                if inner.kind() == "assignment" {
                    let left_node = inner.child_by_field_name("left").unwrap();
                    let name = left_node.utf8_text(source.as_bytes()).unwrap().to_string();

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
                                dtype: "Any".to_string(),
                                value,
                            });
                        }
                    } else {
                        stmts.push(Stmt::Declaration {
                            name,
                            dtype: "Any".to_string(),
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
        "string" => Ok(Expr::Literal(clean_python_string(
            node.utf8_text(source.as_bytes()).unwrap(),
        ))),
        "integer" => Ok(Expr::Literal(
            node.utf8_text(source.as_bytes()).unwrap().to_string(),
        )),
        "true" => Ok(Expr::Literal("True".to_string())),
        "false" => Ok(Expr::Literal("False".to_string())),
        "identifier" => Ok(Expr::Var(
            node.utf8_text(source.as_bytes()).unwrap().to_string(),
        )),

        "call" => {
            let function_node = node.child_by_field_name("function").unwrap();
            let args_node = node.child_by_field_name("arguments").unwrap();

            let name = function_node
                .utf8_text(source.as_bytes())
                .unwrap()
                .to_string();
            let mut args = Vec::new();
            for arg in args_node.named_children(&mut args_node.walk()) {
                // Python adds punctuation (commas) as named children in some versions,
                // but usually named_children skips them.
                args.push(parse_expr(arg, source)?);
            }
            Ok(Expr::Call { name, args })
        }

        "binary_operator" => {
            let left = parse_expr(node.child_by_field_name("left").unwrap(), source)?;
            let right = parse_expr(node.child_by_field_name("right").unwrap(), source)?;
            let operator = node
                .child_by_field_name("operator")
                .unwrap()
                .utf8_text(source.as_bytes())
                .unwrap();

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
            let count = child.named_child_count();
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

fn clean_python_string(s: &str) -> String {
    let s = s.trim();

    if let Some(quote_start) = s.find(['"', '\'']) {
        let content = &s[quote_start..];
        return strip_python_quotes(content);
    }

    s.to_string()
}

fn strip_python_quotes(s: &str) -> String {
    if s.starts_with("\"\"\"") && s.ends_with("\"\"\"") && s.len() >= 6 {
        return s[3..s.len() - 3].to_string();
    }
    if s.starts_with("'''") && s.ends_with("'''") && s.len() >= 6 {
        return s[3..s.len() - 3].to_string();
    }

    if (s.starts_with('"') && s.ends_with('"'))
        || (s.starts_with('\'') && s.ends_with('\'')) && s.len() >= 2
    {
        return s[1..s.len() - 1].to_string();
    }

    s.to_string()
}
