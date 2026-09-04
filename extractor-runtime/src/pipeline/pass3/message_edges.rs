use std::collections::{HashMap, HashSet};

use models::{
    CallStatement, MessageDestinationKind, MessageEdge, MessageRole, ParsedCallable, Scope,
    ir::{
        ast::Expr,
        project::{ProjectIR, TypedFileRecord},
    },
};

use crate::pipeline::pass3::{
    env::{Env, build_constants_env, build_file_env},
    pass_module::PerFileModuleConsts,
};

const DESTINATION_SEPARATOR: &str = ":";
const SELF_ATTRIBUTE_PREFIX: &str = "self.";
const INITIALIZER_FUNCTION_PREFIX: &str = "__init__(";
const CONDITIONAL_FALLBACK_SEPARATOR: &str = " else ";
const ATTRIBUTE_SEPARATOR: char = '.';
const SERVICE_PATH_SEPARATORS: &[char] = &['/', '\\', '-', '_'];
const ENVIRONMENT_NAME_SEPARATOR: &str = "_";

/// Resolves all message edges using project-wide and per-file constant environments.
pub(super) fn evaluate_message_edges(
    project_ir: &ProjectIR,
    external_constants: &HashMap<String, String>,
    per_file_attrs: &HashMap<String, HashMap<String, String>>,
    per_file_module_consts: &PerFileModuleConsts,
) -> Vec<MessageEdge> {
    let constants_env = build_constants_env(&project_ir.constants, external_constants);

    project_ir
        .files
        .iter()
        .filter(|f| !f.raw_message_edges.is_empty())
        .flat_map(|file| {
            let file_env = build_file_env(
                file,
                project_ir,
                &constants_env,
                per_file_attrs,
                per_file_module_consts,
            );
            let mut seen = HashSet::new();
            file.raw_message_edges
                .iter()
                .flat_map(|edge| evaluate_edge_instances(edge, file, &file_env, project_ir))
                .filter(|edge| seen.insert(edge.clone()))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn evaluate_edge_instances(
    edge: &MessageEdge,
    file: &TypedFileRecord,
    file_env: &Env,
    project_ir: &ProjectIR,
) -> Vec<MessageEdge> {
    let Some(callable) = find_enclosing_callable(edge, file) else {
        return evaluate_single_edge_variants(edge, file, file_env);
    };
    let callable_env = build_callable_env(file, file_env, callable);
    if callable.metadata.parameters.is_empty() {
        return evaluate_single_edge_variants(edge, file, &callable_env);
    }

    let callsites = find_wrapper_callsites(&callable, project_ir);
    if callsites.is_empty() {
        return evaluate_single_edge_variants(edge, file, &callable_env);
    }

    callsites
        .into_iter()
        .flat_map(|(caller_file, callsite)| {
            let caller_file_env = build_file_env(
                caller_file,
                project_ir,
                &build_constants_env(&project_ir.constants, &HashMap::new()),
                &HashMap::new(),
                &HashMap::new(),
            );
            let caller_env = build_caller_env(caller_file, &caller_file_env, callsite);
            let specialized_env =
                bind_callable_parameters(&callable, callsite, caller_file, &caller_env);
            let callable_env = build_callable_env(file, &specialized_env, callable);
            evaluate_single_edge_variants(edge, file, &callable_env)
        })
        .collect()
}

/// Resolves an edge's fields and derives its final destination from its role and transport data.
fn evaluate_single_edge(edge: &MessageEdge, file: &TypedFileRecord, env: &Env) -> MessageEdge {
    evaluate_single_edge_variants(edge, file, env)
        .into_iter()
        .next()
        .unwrap_or_else(|| edge.clone())
}

fn evaluate_single_edge_variants(
    edge: &MessageEdge,
    file: &TypedFileRecord,
    env: &Env,
) -> Vec<MessageEdge> {
    let exchanges = resolve_option_values(edge.exchange.as_deref(), file, env);
    let topics = resolve_option_values(edge.topic.as_deref(), file, env);
    let routing_keys = resolve_option_values(edge.routing_key.as_deref(), file, env);
    let queues = resolve_option_values(edge.queue.as_deref(), file, env);
    let handlers = resolve_option_values(edge.handler.as_deref(), file, env);

    let exchanges = ensure_non_empty_option_set(exchanges);
    let topics = ensure_non_empty_option_set(topics);
    let routing_keys = ensure_non_empty_option_set(routing_keys);
    let queues = ensure_non_empty_option_set(queues);
    let handlers = ensure_non_empty_option_set(handlers);

    let mut variants = Vec::new();
    for exchange in &exchanges {
        for topic in &topics {
            for routing_key in &routing_keys {
                for queue in &queues {
                    for handler in &handlers {
                        let destination = match edge.role {
                            MessageRole::Producer => match (exchange, routing_key) {
                                _ if matches!(
                                    edge.destination_kind,
                                    MessageDestinationKind::Topic
                                ) =>
                                {
                                    topic.clone().unwrap_or_else(|| edge.destination.clone())
                                }
                                (Some(exchange), Some(routing_key)) if !exchange.is_empty() => {
                                    format!("{exchange}{DESTINATION_SEPARATOR}{routing_key}")
                                }
                                (_, Some(routing_key)) => routing_key.clone(),
                                (Some(exchange), _) => exchange.clone(),
                                _ => edge.destination.clone(),
                            },
                            MessageRole::Binding => match (exchange, routing_key) {
                                (Some(exchange), Some(routing_key)) if !exchange.is_empty() => {
                                    format!("{exchange}{DESTINATION_SEPARATOR}{routing_key}")
                                }
                                _ => topic
                                    .clone()
                                    .or_else(|| queue.clone())
                                    .unwrap_or_else(|| edge.destination.clone()),
                            },
                            MessageRole::Consumer
                            | MessageRole::QueueDeclaration
                            | MessageRole::TopicDeclaration => topic
                                .clone()
                                .or_else(|| queue.clone())
                                .unwrap_or_else(|| edge.destination.clone()),
                        };

                        let destination_kind = match edge.role {
                            MessageRole::Producer
                                if !matches!(
                                    edge.destination_kind,
                                    MessageDestinationKind::Topic
                                ) =>
                            {
                                if exchange.as_ref().is_some_and(|value| !value.is_empty()) {
                                    MessageDestinationKind::ExchangeRoutingKey
                                } else {
                                    MessageDestinationKind::Queue
                                }
                            }
                            _ => edge.destination_kind.clone(),
                        };

                        variants.push(MessageEdge {
                            destination_kind,
                            ..edge.clone_with_resolved_destination(
                                destination,
                                topic.clone(),
                                exchange.clone(),
                                routing_key.clone(),
                                queue.clone(),
                                handler.clone(),
                            )
                        });
                    }
                }
            }
        }
    }

    variants
}

/// Resolves a value through the supported sources in precedence order.
fn resolve_value(raw: &str, file: &TypedFileRecord, env: &Env) -> String {
    resolve_values(raw, file, env)
        .into_iter()
        .next()
        .unwrap_or_default()
}

fn resolve_values(raw: &str, file: &TypedFileRecord, env: &Env) -> Vec<String> {
    resolve_values_inner(raw, file, env, &mut HashSet::new())
}

fn resolve_values_inner(
    raw: &str,
    file: &TypedFileRecord,
    env: &Env,
    visited: &mut HashSet<String>,
) -> Vec<String> {
    let trimmed = raw.trim();
    if let Some(values) = parse_iterable_literal(trimmed) {
        return values;
    }

    let cleaned = statix::strings::clean_python_string(trimmed);
    if cleaned.is_empty() {
        return vec![cleaned];
    }
    if !visited.insert(cleaned.clone()) {
        return vec![cleaned];
    }

    let resolved = resolve_many_from_env(&cleaned, env)
        .or_else(|| resolve_self_attr(&cleaned, file, env).map(|value| vec![value]))
        .or_else(|| {
            resolve_from_env_by_attr(&cleaned, &file.file_path, env).map(|value| vec![value])
        })
        .or_else(|| resolve_conditional_fallback(&cleaned, file, env).map(|value| vec![value]))
        .unwrap_or_else(|| vec![cleaned.clone()]);

    resolved
        .into_iter()
        .flat_map(|value| {
            if value == cleaned {
                vec![value]
            } else {
                resolve_values_inner(&value, file, env, visited)
            }
        })
        .collect()
}
/// Resolves `self.*` references assigned in the class initializer.
fn resolve_self_attr(raw: &str, file: &TypedFileRecord, env: &Env) -> Option<String> {
    if !raw.starts_with(SELF_ATTRIBUTE_PREFIX) {
        return None;
    }

    for (key, assignment) in &file.assignments {
        if key.variable_name != raw {
            continue;
        }
        if !matches!(&key.scope, Scope::Function(name) if name.starts_with(INITIALIZER_FUNCTION_PREFIX))
        {
            continue;
        }

        return Some(resolve_value(&assignment.value, file, env));
    }

    None
}

/// Extracts and resolves the fallback branch of a conditional expression.
fn resolve_conditional_fallback(raw: &str, file: &TypedFileRecord, env: &Env) -> Option<String> {
    let (_condition, fallback) = raw.split_once(CONDITIONAL_FALLBACK_SEPARATOR)?;
    Some(resolve_value(fallback, file, env))
}

/// Resolves an exact name from the evaluated environment.
fn resolve_from_env(raw: &str, env: &Env) -> Option<String> {
    let (_, expr) = env.get(raw)?;
    expr_to_string(expr)
}

fn resolve_many_from_env(raw: &str, env: &Env) -> Option<Vec<String>> {
    let (_, expr) = env.get(raw)?;
    expr_to_values(expr)
}

/// Resolves an attribute by matching environment names and preferring the local service.
fn resolve_from_env_by_attr(raw: &str, file_path: &str, env: &Env) -> Option<String> {
    let attr = raw
        .rsplit(ATTRIBUTE_SEPARATOR)
        .next()
        .unwrap_or(raw)
        .to_ascii_uppercase();
    if attr.is_empty() {
        return None;
    }

    let mut candidates = env
        .iter()
        .filter_map(|(name, (_, expr))| {
            let upper_name = name.to_ascii_uppercase();
            if upper_name == attr
                || upper_name.ends_with(&format!("{ENVIRONMENT_NAME_SEPARATOR}{attr}"))
            {
                expr_to_string(expr).map(|value| (name.as_str(), value))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|(left, _), (right, _)| {
        service_name_score(right, file_path).cmp(&service_name_score(left, file_path))
    });

    candidates.into_iter().next().map(|(_, value)| value)
}

/// Scores how strongly an environment name matches the service path.
fn service_name_score(env_name: &str, file_path: &str) -> usize {
    let env_name = env_name.to_ascii_lowercase();
    file_path
        .split(SERVICE_PATH_SEPARATORS)
        .filter(|part| part.len() > 2 && env_name.contains(&part.to_ascii_lowercase()))
        .count()
}

/// Converts statically evaluable expressions into strings.
fn expr_to_string(expr: &Expr) -> Option<String> {
    expr_to_values(expr).and_then(|values| values.into_iter().next())
}

fn expr_to_values(expr: &Expr) -> Option<Vec<String>> {
    match expr {
        Expr::Literal(value) => {
            Some(parse_iterable_literal(value).unwrap_or_else(|| vec![value.clone()]))
        }
        Expr::Var(value) => {
            Some(parse_iterable_literal(value).unwrap_or_else(|| vec![value.clone()]))
        }
        Expr::Concat(left, right) => {
            let left = expr_to_values(left)?;
            let right = expr_to_values(right)?;
            Some(
                left.into_iter()
                    .flat_map(|left| right.iter().map(move |right| format!("{left}{right}")))
                    .collect(),
            )
        }
        Expr::Joined { vals } => {
            let mut parts = vec![String::new()];
            for value in vals {
                let resolved = expr_to_values(value)?;
                parts = parts
                    .into_iter()
                    .flat_map(|prefix| {
                        resolved
                            .iter()
                            .map(move |suffix| format!("{prefix}{suffix}"))
                    })
                    .collect();
            }
            Some(parts)
        }
        _ => None,
    }
}

fn resolve_option_values(
    raw: Option<&str>,
    file: &TypedFileRecord,
    env: &Env,
) -> Vec<Option<String>> {
    match raw {
        Some(value) => resolve_values(value, file, env)
            .into_iter()
            .map(Some)
            .collect(),
        None => vec![None],
    }
}

fn ensure_non_empty_option_set(values: Vec<Option<String>>) -> Vec<Option<String>> {
    if values.is_empty() {
        vec![None]
    } else {
        values
    }
}

fn parse_iterable_literal(raw: &str) -> Option<Vec<String>> {
    let trimmed = raw.trim();
    if !(trimmed.starts_with("[]") && trimmed.contains('{') && trimmed.ends_with('}')) {
        return None;
    }
    let body = trimmed
        .split_once('{')
        .map(|(_, rest)| rest.trim_end_matches('}'))
        .unwrap_or_default();
    let values = body
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(statix::strings::clean_python_string)
        .collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn find_enclosing_callable<'a>(
    edge: &MessageEdge,
    file: &'a TypedFileRecord,
) -> Option<&'a ParsedCallable> {
    file.callables
        .iter()
        .find(|pc| pc.metadata.hash == edge.function_hash)
        .or_else(|| pc_by_name(edge.function_name.as_str(), file))
}

fn pc_by_name<'a>(name: &str, file: &'a TypedFileRecord) -> Option<&'a ParsedCallable> {
    file.callables.iter().find(|pc| pc.metadata.name == name)
}

fn find_wrapper_callsites<'a>(
    callable: &ParsedCallable,
    project_ir: &'a ProjectIR,
) -> Vec<(&'a TypedFileRecord, &'a CallStatement)> {
    let target_name = simple_callable_name(&callable.metadata.name);
    project_ir
        .files
        .iter()
        .flat_map(|file| {
            file.call_statements
                .iter()
                .filter(|call| call_invokes_callable(call, target_name))
                .map(move |call| (file, call))
        })
        .collect()
}

fn simple_callable_name(name: &str) -> &str {
    name.split('(').next().unwrap_or(name).trim()
}

fn call_invokes_callable(call: &CallStatement, target_name: &str) -> bool {
    let function_name = call.function_name.trim();
    function_name == target_name || function_name.ends_with(&format!(".{target_name}"))
}

fn build_caller_env(file: &TypedFileRecord, file_env: &Env, call: &CallStatement) -> Env {
    let mut env = file_env.clone();
    let scope = Scope::from_enclosing_function(call.enclosing_function_name.clone());
    let scoped_assignments = file
        .assignments
        .iter()
        .filter(|(key, _)| key.scope == scope)
        .map(|(_, assignment)| assignment)
        .collect::<Vec<_>>();

    for _ in 0..scoped_assignments.len().max(1) {
        for assignment in &scoped_assignments {
            let resolved = resolve_values(&assignment.value, file, &env);
            env.insert(
                assignment.variable_name.clone(),
                (Some("String".to_string()), values_to_expr(resolved)),
            );
        }
    }

    env
}

fn build_callable_env(file: &TypedFileRecord, base_env: &Env, callable: &ParsedCallable) -> Env {
    let mut env = base_env.clone();
    let scope = Scope::from_enclosing_function(Some(callable.metadata.signature.clone()));
    let scoped_assignments = file
        .assignments
        .iter()
        .filter(|(key, _)| key.scope == scope)
        .map(|(_, assignment)| assignment)
        .collect::<Vec<_>>();

    for _ in 0..scoped_assignments.len().max(1) {
        for assignment in &scoped_assignments {
            let resolved = resolve_values(&assignment.value, file, &env);
            env.insert(
                assignment.variable_name.clone(),
                (Some("String".to_string()), values_to_expr(resolved)),
            );
        }
    }

    env
}

fn bind_callable_parameters(
    callable: &ParsedCallable,
    call: &CallStatement,
    file: &TypedFileRecord,
    caller_env: &Env,
) -> Env {
    let mut env = caller_env.clone();
    let positional_args = call
        .arguments
        .iter()
        .filter(|arg| arg.assigned_variable.is_empty())
        .collect::<Vec<_>>();
    let mut positional_index = 0usize;

    for parameter in &callable.metadata.parameters {
        if matches!(parameter.name.as_str(), "self" | "cls") {
            continue;
        }

        let argument = call
            .arguments
            .iter()
            .find(|arg| arg.assigned_variable == parameter.name)
            .or_else(|| {
                let arg = positional_args.get(positional_index).copied();
                if arg.is_some() {
                    positional_index += 1;
                }
                arg
            });

        if let Some(argument) = argument {
            let resolved = resolve_values(&argument.value, file, &env);
            env.insert(
                parameter.name.clone(),
                (
                    parameter
                        .datatype
                        .clone()
                        .or_else(|| argument.datatype.clone()),
                    values_to_expr(resolved),
                ),
            );
        } else if let Some(default_value) = &parameter.initial_value {
            let resolved = resolve_values(default_value, file, &env);
            env.insert(
                parameter.name.clone(),
                (parameter.datatype.clone(), values_to_expr(resolved)),
            );
        }
    }

    env
}

fn values_to_expr(values: Vec<String>) -> Expr {
    if values.len() <= 1 {
        Expr::Literal(values.into_iter().next().unwrap_or_default())
    } else {
        Expr::Literal(format!(
            "[]string{{{}}}",
            values
                .into_iter()
                .map(|value| format!("\"{value}\""))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}
