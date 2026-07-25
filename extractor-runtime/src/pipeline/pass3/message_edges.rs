use std::collections::HashMap;

use models::{
    MessageDestinationKind, MessageEdge, MessageRole, Scope,
    ir::{
        ast::Expr,
        project::{ProjectIR, TypedFileRecord},
    },
};

use crate::pipeline::pass3::{
    env::{Env, build_constants_env, build_file_env},
    pass_module::PerFileModuleConsts,
};

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
            file.raw_message_edges
                .iter()
                .map(|edge| evaluate_single_edge(edge, file, &file_env))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn evaluate_single_edge(edge: &MessageEdge, file: &TypedFileRecord, env: &Env) -> MessageEdge {
    let exchange = edge
        .exchange
        .as_ref()
        .map(|value| resolve_value(value, file, env));
    let topic = edge
        .topic
        .as_ref()
        .map(|value| resolve_value(value, file, env));
    let routing_key = edge
        .routing_key
        .as_ref()
        .map(|value| resolve_value(value, file, env));
    let queue = edge
        .queue
        .as_ref()
        .map(|value| resolve_value(value, file, env));
    let handler = edge
        .handler
        .as_ref()
        .map(|value| resolve_value(value, file, env));

    let destination = match edge.role {
        MessageRole::Producer => match (&exchange, &routing_key) {
            _ if matches!(edge.destination_kind, MessageDestinationKind::Topic) => {
                topic.clone().unwrap_or_else(|| edge.destination.clone())
            }
            (Some(exchange), Some(routing_key)) if !exchange.is_empty() => {
                format!("{exchange}:{routing_key}")
            }
            (_, Some(routing_key)) => routing_key.clone(),
            (Some(exchange), _) => exchange.clone(),
            _ => edge.destination.clone(),
        },
        MessageRole::Consumer | MessageRole::QueueDeclaration => topic
            .clone()
            .or_else(|| queue.clone())
            .unwrap_or_else(|| edge.destination.clone()),
    };

    let destination_kind = if matches!(edge.role, MessageRole::Producer)
        && exchange.as_ref().is_some_and(|value| !value.is_empty())
    {
        MessageDestinationKind::ExchangeRoutingKey
    } else {
        edge.destination_kind.clone()
    };

    MessageEdge {
        destination_kind,
        ..edge.clone_with_resolved_destination(
            destination,
            topic,
            exchange,
            routing_key,
            queue,
            handler,
        )
    }
}

fn resolve_value(raw: &str, file: &TypedFileRecord, env: &Env) -> String {
    let cleaned = statix::strings::clean_python_string(raw.trim());
    if cleaned.is_empty() {
        return cleaned;
    }

    if let Some(value) = resolve_from_env(&cleaned, env) {
        return value;
    }

    if let Some(value) = resolve_self_attr(&cleaned, file, env) {
        return value;
    }

    if let Some(value) = resolve_from_env_by_attr(&cleaned, &file.file_path, env) {
        return value;
    }

    if let Some(value) = resolve_conditional_fallback(&cleaned, file, env) {
        return value;
    }

    cleaned
}

fn resolve_self_attr(raw: &str, file: &TypedFileRecord, env: &Env) -> Option<String> {
    if !raw.starts_with("self.") {
        return None;
    }

    for (key, assignment) in &file.assignments {
        if key.variable_name != raw {
            continue;
        }
        if !matches!(&key.scope, Scope::Function(name) if name.starts_with("__init__(")) {
            continue;
        }

        return Some(resolve_value(&assignment.value, file, env));
    }

    None
}

fn resolve_conditional_fallback(raw: &str, file: &TypedFileRecord, env: &Env) -> Option<String> {
    let (_condition, fallback) = raw.split_once(" else ")?;
    Some(resolve_value(fallback, file, env))
}

fn resolve_from_env(raw: &str, env: &Env) -> Option<String> {
    let (_, expr) = env.get(raw)?;
    expr_to_string(expr)
}

fn resolve_from_env_by_attr(raw: &str, file_path: &str, env: &Env) -> Option<String> {
    let attr = raw.rsplit('.').next().unwrap_or(raw).to_ascii_uppercase();
    if attr.is_empty() {
        return None;
    }

    let mut candidates = env
        .iter()
        .filter_map(|(name, (_, expr))| {
            let upper_name = name.to_ascii_uppercase();
            if upper_name == attr || upper_name.ends_with(&format!("_{attr}")) {
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

fn service_name_score(env_name: &str, file_path: &str) -> usize {
    let env_name = env_name.to_ascii_lowercase();
    file_path
        .split(['/', '\\', '-', '_'])
        .filter(|part| part.len() > 2 && env_name.contains(&part.to_ascii_lowercase()))
        .count()
}

fn expr_to_string(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Literal(value) => Some(value.clone()),
        Expr::Var(value) => Some(value.clone()),
        Expr::Concat(left, right) => {
            let left = expr_to_string(left)?;
            let right = expr_to_string(right)?;
            Some(format!("{left}{right}"))
        }
        Expr::Joined { vals } => vals.iter().map(expr_to_string).collect::<Option<String>>(),
        _ => None,
    }
}
