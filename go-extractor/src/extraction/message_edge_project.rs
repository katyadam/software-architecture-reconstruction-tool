use std::collections::{HashMap, HashSet};

use models::{
    CallStatement, CommunicationProtocol, MessageDestinationKind, MessageEdge, MessageRole,
    ParsedCallable, ir::project::TypedFileRecord,
};

use super::shared::package_path;

const MAX_RESOLUTION_DEPTH: usize = 4;

/// Replaces Go message edges with concrete variants derived from project call sites.
pub(super) fn resolve_message_edges(files: &mut [TypedFileRecord]) {
    let snapshots = files
        .iter()
        .filter(|file| file.language == models::ir::language::Language::Go)
        .map(FileSnapshot::from)
        .collect::<Vec<_>>();
    let known_values = known_values(&snapshots);

    for file in files
        .iter_mut()
        .filter(|file| file.language == models::ir::language::Language::Go)
    {
        file.raw_message_edges = file
            .raw_message_edges
            .iter()
            .flat_map(|edge| {
                let Some(source) = snapshot_for(&snapshots, &file.file_path) else {
                    return vec![edge.clone()];
                };
                let Some(callable) = source
                    .callables
                    .iter()
                    .find(|callable| callable.metadata.hash == edge.function_hash)
                else {
                    return vec![edge.clone()];
                };
                let resolved = matching_calls(callable, source, &snapshots)
                    .into_iter()
                    .flat_map(|invocation| {
                        resolve_invocation(
                            edge,
                            callable,
                            invocation,
                            &snapshots,
                            &known_values,
                            0,
                            &mut HashSet::new(),
                        )
                    })
                    .collect::<Vec<_>>();
                if resolved.is_empty() {
                    vec![edge.clone()]
                } else {
                    resolved
                }
            })
            .collect();
    }
}

struct FileSnapshot {
    file_path: String,
    import_modules: Vec<String>,
    callables: Vec<ParsedCallable>,
    calls: Vec<CallStatement>,
    assignments: Vec<(String, String)>,
}

struct Invocation<'a> {
    call: &'a CallStatement,
    file: &'a FileSnapshot,
}

impl From<&TypedFileRecord> for FileSnapshot {
    /// Retains the call, callable, and assignment metadata needed for project resolution.
    fn from(file: &TypedFileRecord) -> Self {
        Self {
            file_path: file.file_path.clone(),
            import_modules: file
                .imports
                .iter()
                .map(|import| import.orig_module.clone())
                .collect(),
            callables: file.callables.clone(),
            calls: file.call_statements.clone(),
            assignments: file
                .assignments
                .values()
                .map(|assignment| (assignment.variable_name.clone(), assignment.value.clone()))
                .collect(),
        }
    }
}

/// Finds the snapshot belonging to a known project file.
fn snapshot_for<'a>(files: &'a [FileSnapshot], file_path: &str) -> Option<&'a FileSnapshot> {
    files.iter().find(|file| file.file_path == file_path)
}

/// Finds calls to a callable from its own package or an importing Go package.
fn matching_calls<'a>(
    callable: &'a ParsedCallable,
    target_file: &FileSnapshot,
    files: &'a [FileSnapshot],
) -> Vec<Invocation<'a>> {
    let calls = files
        .iter()
        .filter(|file| package_matches(file, target_file))
        .flat_map(|file| file.calls.iter().map(move |call| Invocation { call, file }))
        .filter(|invocation| {
            invocation.call.function_name.rsplit('.').next()
                == Some(callable.metadata.name.as_str())
                && invocation.call.arguments.len() == callable.metadata.parameters.len()
        })
        .collect::<Vec<_>>();
    if !calls.is_empty() || !matches!(callable.metadata.namespace, models::Namespace::Class(_)) {
        return calls;
    }

    files
        .iter()
        .flat_map(|file| file.calls.iter().map(move |call| Invocation { call, file }))
        .filter(|invocation| {
            invocation.call.function_name.rsplit('.').next()
                == Some(callable.metadata.name.as_str())
                && invocation.call.arguments.len() == callable.metadata.parameters.len()
        })
        .collect()
}

/// Checks whether a caller belongs to or imports the callable's package.
fn package_matches(caller: &FileSnapshot, target_file: &FileSnapshot) -> bool {
    let package = package_path(&target_file.file_path);
    if package_path(&caller.file_path) == package {
        return true;
    }
    let suffix = package
        .rsplit('/')
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("/");
    caller
        .import_modules
        .iter()
        .any(|module| module.replace('\\', "/").ends_with(&suffix))
}

/// Resolves one invocation and follows its enclosing wrapper when it has callers.
fn resolve_invocation(
    edge: &MessageEdge,
    callable: &ParsedCallable,
    invocation: Invocation<'_>,
    files: &[FileSnapshot],
    known_values: &HashMap<String, Vec<String>>,
    depth: usize,
    visited: &mut HashSet<String>,
) -> Vec<MessageEdge> {
    let resolved = resolve_edge(edge, callable, &invocation, files, known_values);
    if depth >= MAX_RESOLUTION_DEPTH {
        return resolved;
    }
    let Some(parent) = invocation.file.callables.iter().find(|candidate| {
        invocation.call.enclosing_function_hash.as_ref() == Some(&candidate.metadata.hash)
            || invocation.call.enclosing_function_name.as_deref()
                == Some(candidate.metadata.name.as_str())
            || invocation.call.enclosing_function_name.as_deref()
                == Some(candidate.metadata.signature.as_str())
    }) else {
        return resolved;
    };
    let visit_key = format!("{}:{}", invocation.file.file_path, parent.metadata.hash);
    if !visited.insert(visit_key) {
        return resolved;
    }
    let parent_calls = matching_calls(parent, invocation.file, files);
    if parent_calls.is_empty() {
        return resolved;
    }
    let propagated = resolved
        .iter()
        .flat_map(|edge| {
            parent_calls.iter().flat_map(|parent_call| {
                let mut route = visited.clone();
                resolve_invocation(
                    edge,
                    parent,
                    Invocation {
                        call: parent_call.call,
                        file: parent_call.file,
                    },
                    files,
                    known_values,
                    depth + 1,
                    &mut route,
                )
            })
        })
        .collect::<Vec<_>>();
    if propagated.is_empty() {
        resolved
    } else {
        propagated
    }
}

/// Resolves all parameter-backed transport fields for one concrete invocation.
fn resolve_edge(
    edge: &MessageEdge,
    callable: &ParsedCallable,
    invocation: &Invocation<'_>,
    files: &[FileSnapshot],
    known_values: &HashMap<String, Vec<String>>,
) -> Vec<MessageEdge> {
    let bindings = callable
        .metadata
        .parameters
        .iter()
        .zip(&invocation.call.arguments)
        .map(|(parameter, argument)| (parameter.name.as_str(), argument.value.as_str()))
        .collect::<HashMap<_, _>>();
    let aliases = snapshot_for(files, &callable.metadata.file_path)
        .map(|file| file.assignments.iter().cloned().collect::<HashMap<_, _>>())
        .unwrap_or_default();
    let exchanges = values_for(edge.exchange.as_deref(), &bindings, &aliases, known_values);
    let routing_keys = values_for(
        edge.routing_key.as_deref(),
        &bindings,
        &aliases,
        known_values,
    );
    let queues = values_for(edge.queue.as_deref(), &bindings, &aliases, known_values);
    let topics = values_for(edge.topic.as_deref(), &bindings, &aliases, known_values);

    cartesian_edges(edge, exchanges, routing_keys, queues, topics)
        .into_iter()
        // QueueBind commonly reuses one loop variable for its queue and routing key.
        // Keep those expansions paired instead of constructing a cross-product.
        .filter(|resolved| {
            edge.role != MessageRole::Binding
                || edge.queue != edge.routing_key
                || resolved.queue == resolved.routing_key
        })
        .map(|edge| MessageEdge {
            file_path: invocation.file.file_path.clone(),
            ..edge
        })
        .collect()
}

/// Collects statically known configuration values by selector suffix.
fn known_values(files: &[FileSnapshot]) -> HashMap<String, Vec<String>> {
    let mut values = HashMap::<String, Vec<String>>::new();
    for (name, value) in files.iter().flat_map(|file| &file.assignments) {
        let literals = string_literals(value);
        if literals.is_empty() {
            continue;
        }
        for suffix in selector_suffixes(name) {
            let entry = values.entry(suffix).or_default();
            for literal in &literals {
                if !entry.contains(literal) {
                    entry.push(literal.clone());
                }
            }
        }
    }
    values
}

/// Produces selector suffixes that are specific enough to avoid field-name collisions.
fn selector_suffixes(value: &str) -> Vec<String> {
    let parts = value.split('.').collect::<Vec<_>>();
    (1..=parts.len())
        .map(|length| parts[parts.len() - length..].join("."))
        .collect()
}

/// Resolves a field through parameter bindings, queue lists, and known configuration selectors.
fn values_for(
    value: Option<&str>,
    bindings: &HashMap<&str, &str>,
    aliases: &HashMap<String, String>,
    known_values: &HashMap<String, Vec<String>>,
) -> Vec<Option<String>> {
    let Some(value) = value else {
        return vec![None];
    };
    let resolved = binding_values(value, bindings, aliases, 0);
    let values = resolved
        .iter()
        .flat_map(|value| concrete_values(value, known_values))
        .collect::<Vec<_>>();
    if values.is_empty() {
        resolved.into_iter().map(Some).collect()
    } else {
        values.into_iter().map(Some).collect()
    }
}

/// Applies a parameter binding while retaining a selector suffix such as `.Name`.
fn binding_values(
    value: &str,
    bindings: &HashMap<&str, &str>,
    aliases: &HashMap<String, String>,
    depth: usize,
) -> Vec<String> {
    if depth >= MAX_RESOLUTION_DEPTH {
        return vec![value.to_string()];
    }
    if let Some(resolved) = bindings.get(value) {
        return vec![(*resolved).to_string()];
    }
    let Some((root, suffix)) = value.split_once('.') else {
        return aliases
            .get(value)
            .map(|alias| binding_values(alias, bindings, aliases, depth + 1))
            .unwrap_or_else(|| vec![value.to_string()]);
    };
    let bound = bindings
        .get(root)
        .map(|value| (*value).to_string())
        .or_else(|| aliases.get(root).cloned());
    let Some(bound) = bound else {
        return vec![value.to_string()];
    };
    let items = composite_items(&bound);
    if items.is_empty() {
        binding_values(&format!("{bound}.{suffix}"), bindings, aliases, depth + 1)
    } else {
        items
            .into_iter()
            .map(|item| format!("{item}.{suffix}"))
            .collect()
    }
}

/// Converts literals and unambiguous selector values into concrete message fields.
fn concrete_values(value: &str, known_values: &HashMap<String, Vec<String>>) -> Vec<String> {
    let literals = string_literals(value);
    if !literals.is_empty() {
        return literals;
    }
    selector_suffixes(value)
        .into_iter()
        .find_map(|suffix| {
            known_values
                .get(&suffix)
                .filter(|values| values.len() == 1)
                .cloned()
        })
        .unwrap_or_default()
}

/// Extracts top-level items from a Go composite literal such as `[]QueueConfig{a, b}`.
fn composite_items(value: &str) -> Vec<String> {
    let Some(start) = value.find('{') else {
        return vec![];
    };
    let Some(end) = value.rfind('}') else {
        return vec![];
    };
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut item_start = start + 1;
    for (index, character) in value[start + 1..end].char_indices() {
        match character {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let item = value[item_start..start + 1 + index].trim();
                if !item.is_empty() {
                    items.push(item.to_string());
                }
                item_start = start + 2 + index;
            }
            _ => {}
        }
    }
    let item = value[item_start..end].trim();
    if !item.is_empty() {
        items.push(item.to_string());
    }
    items
}

/// Produces one edge for every concrete combination of resolved transport fields.
fn cartesian_edges(
    edge: &MessageEdge,
    exchanges: Vec<Option<String>>,
    routing_keys: Vec<Option<String>>,
    queues: Vec<Option<String>>,
    topics: Vec<Option<String>>,
) -> Vec<MessageEdge> {
    let mut edges = Vec::new();
    for exchange in &exchanges {
        for routing_key in &routing_keys {
            for queue in &queues {
                for topic in &topics {
                    let destination = destination(edge, exchange, routing_key, queue, topic);
                    edges.push(MessageEdge {
                        destination,
                        exchange: exchange.clone(),
                        routing_key: routing_key.clone(),
                        queue: queue.clone(),
                        topic: topic.clone(),
                        ..edge.clone()
                    });
                }
            }
        }
    }
    edges
}

/// Rebuilds the destination from resolved RabbitMQ or Kafka transport fields.
fn destination(
    edge: &MessageEdge,
    exchange: &Option<String>,
    routing_key: &Option<String>,
    queue: &Option<String>,
    topic: &Option<String>,
) -> String {
    if edge.protocol == CommunicationProtocol::Kafka
        || matches!(edge.destination_kind, MessageDestinationKind::Topic)
    {
        return topic.clone().unwrap_or_else(|| edge.destination.clone());
    }
    match edge.role {
        MessageRole::Producer | MessageRole::Binding => match (exchange, routing_key) {
            (Some(exchange), Some(routing_key)) if !exchange.is_empty() => {
                format!("{exchange}:{routing_key}")
            }
            (_, Some(routing_key)) => routing_key.clone(),
            (Some(exchange), _) => exchange.clone(),
            _ => queue.clone().unwrap_or_else(|| edge.destination.clone()),
        },
        MessageRole::Consumer | MessageRole::QueueDeclaration | MessageRole::TopicDeclaration => {
            queue.clone().unwrap_or_else(|| edge.destination.clone())
        }
    }
}

/// Extracts interpreted and raw string literals from a Go expression.
fn string_literals(raw: &str) -> Vec<String> {
    let mut values = Vec::new();
    let bytes = raw.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let delimiter = bytes[index];
        if delimiter != b'"' && delimiter != b'`' {
            index += 1;
            continue;
        }
        let start = index + 1;
        index += 1;
        while index < bytes.len() {
            if delimiter == b'"' && bytes[index] == b'\\' {
                index += 2;
                continue;
            }
            if bytes[index] == delimiter {
                values.push(raw[start..index].to_string());
                index += 1;
                break;
            }
            index += 1;
        }
    }
    values
}
