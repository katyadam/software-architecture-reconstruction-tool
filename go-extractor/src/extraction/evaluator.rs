use std::collections::{HashMap, HashSet};

use models::{
    ParsedCallable,
    ir::ast::{Expr, Stmt},
};

use super::shared::{apply_sprintf, evaluate_expression_text};

pub(super) fn evaluate_call_text(
    expression: &str,
    callables: &[ParsedCallable],
    scope: &HashMap<String, String>,
) -> Option<String> {
    let mut visiting = HashSet::new();
    evaluate_call_text_inner(expression, callables, scope, &mut visiting)
        .or_else(|| evaluate_embedded_call(expression, callables, scope, &mut visiting))
}

fn evaluate_embedded_call(
    expression: &str,
    callables: &[ParsedCallable],
    scope: &HashMap<String, String>,
    visiting: &mut HashSet<String>,
) -> Option<String> {
    for (open, character) in expression.char_indices() {
        if character != '(' {
            continue;
        }
        let name_start = expression[..open]
            .char_indices()
            .rev()
            .find(|(_, character)| !is_callable_name_character(*character))
            .map(|(index, character)| index + character.len_utf8())
            .unwrap_or(0);
        if name_start == open {
            continue;
        }
        let Some(close) = matching_parenthesis(expression, open) else {
            continue;
        };
        let candidate = &expression[name_start..=close];
        let Some(value) = evaluate_call_text_inner(candidate, callables, scope, visiting) else {
            continue;
        };

        let mut resolved = String::with_capacity(expression.len() + value.len());
        resolved.push_str(&expression[..name_start]);
        resolved.push_str(&value);
        resolved.push_str(&expression[close + 1..]);
        return Some(resolved);
    }
    None
}

fn is_callable_name_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '.')
}

fn matching_parenthesis(expression: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    for (offset, character) in expression[open..].char_indices() {
        match character {
            '"' | '`' if quote.is_none() => quote = Some(character),
            character if quote == Some(character) => quote = None,
            '(' if quote.is_none() => depth += 1,
            ')' if quote.is_none() => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn evaluate_call_text_inner(
    expression: &str,
    callables: &[ParsedCallable],
    scope: &HashMap<String, String>,
    visiting: &mut HashSet<String>,
) -> Option<String> {
    let expression = expression.trim();
    let (name, arguments) = expression.split_once('(')?;
    let arguments = arguments.strip_suffix(')')?;
    let args = split_arguments(arguments)
        .into_iter()
        .map(|argument| resolve_text(argument, callables, scope, visiting))
        .collect::<Vec<_>>();
    evaluate_callable(name.trim(), &args, callables, scope, visiting)
}

fn evaluate_callable(
    name: &str,
    args: &[String],
    callables: &[ParsedCallable],
    scope: &HashMap<String, String>,
    visiting: &mut HashSet<String>,
) -> Option<String> {
    let mut matches = callables.iter().filter(|callable| {
        callable.metadata.name == name && callable.metadata.parameters.len() == args.len()
    });
    let callable = matches.next()?;
    if matches.next().is_some() || !visiting.insert(callable.metadata.hash.clone()) {
        return None;
    }

    let mut env = scope.clone();
    for (parameter, value) in callable.metadata.parameters.iter().zip(args) {
        env.insert(parameter.name.clone(), value.clone());
    }
    let result = evaluate_statements(&callable.ast.statements, callables, &mut env, visiting);
    visiting.remove(&callable.metadata.hash);
    result
}

fn evaluate_statements(
    statements: &[Stmt],
    callables: &[ParsedCallable],
    env: &mut HashMap<String, String>,
    visiting: &mut HashSet<String>,
) -> Option<String> {
    for statement in statements {
        match statement {
            Stmt::Declaration { name, value, .. } | Stmt::Assignment { name, value } => {
                if let Some(value) = evaluate_expr(value, callables, env, visiting) {
                    env.insert(name.clone(), value);
                }
            }
            Stmt::Return(value) => return evaluate_expr(value, callables, env, visiting),
            _ => {}
        }
    }
    None
}

fn evaluate_expr(
    expression: &Expr,
    callables: &[ParsedCallable],
    env: &HashMap<String, String>,
    visiting: &mut HashSet<String>,
) -> Option<String> {
    match expression {
        Expr::Literal(value) => Some(value.clone()),
        Expr::Var(name) => Some(resolve_text(name, callables, env, visiting)),
        Expr::Concat(left, right) => Some(
            evaluate_expr(left, callables, env, visiting)?
                + &evaluate_expr(right, callables, env, visiting)?,
        ),
        Expr::StructLiteral { .. } => None,
        Expr::Attr { object, field } => {
            let key = format!("{}.{}", expression_name(object)?, field);
            Some(resolve_text(&key, callables, env, visiting))
        }
        Expr::Call {
            name,
            receiver,
            args,
        } if name == "index" => {
            let collection = expression_name(receiver.as_deref()?)?;
            let key = evaluate_expr(args.first()?, callables, env, visiting)?;
            env.get(&format!("{collection}[{key}]")).cloned()
        }
        Expr::Call {
            name,
            receiver,
            args,
        } if name == "Sprintf" && expression_name(receiver.as_deref()?)? == "fmt" => {
            let format = evaluate_expr(args.first()?, callables, env, visiting)?;
            let values = args
                .iter()
                .skip(1)
                .map(|arg| evaluate_expr(arg, callables, env, visiting))
                .collect::<Option<Vec<_>>>()?;
            Some(apply_sprintf(&format, &values))
        }
        Expr::Call {
            name,
            receiver: None,
            args,
        } => {
            let values = args
                .iter()
                .map(|arg| evaluate_expr(arg, callables, env, visiting))
                .collect::<Option<Vec<_>>>()?;
            evaluate_callable(name, &values, callables, env, visiting)
        }
        Expr::Call { receiver, .. } => receiver
            .as_deref()
            .and_then(|receiver| evaluate_expr(receiver, callables, env, visiting)),
        Expr::Joined { vals } if vals.len() == 1 => {
            evaluate_expr(&vals[0], callables, env, visiting)
        }
        Expr::Empty | Expr::Joined { .. } => None,
    }
}

fn expression_name(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Var(name) => Some(name.clone()),
        Expr::StructLiteral { .. } => None,
        Expr::Attr { object, field } => Some(format!("{}.{}", expression_name(object)?, field)),
        _ => None,
    }
}

fn resolve_text(
    value: &str,
    callables: &[ParsedCallable],
    scope: &HashMap<String, String>,
    visiting: &mut HashSet<String>,
) -> String {
    let resolved = evaluate_expression_text(value, scope);
    evaluate_call_text_inner(&resolved, callables, scope, visiting).unwrap_or(resolved)
}

fn split_arguments(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut quote = None;
    let mut start = 0usize;

    for (index, character) in input.char_indices() {
        match character {
            '"' | '`' if quote.is_none() => quote = Some(character),
            character if quote == Some(character) => quote = None,
            '(' | '[' | '{' if quote.is_none() => depth += 1,
            ')' | ']' | '}' if quote.is_none() => depth -= 1,
            ',' if quote.is_none() && depth == 0 => {
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
