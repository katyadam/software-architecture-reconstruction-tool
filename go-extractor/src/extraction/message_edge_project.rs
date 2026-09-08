use std::collections::{HashMap, HashSet};

use models::{
    CallStatement, CommunicationProtocol, MessageDestinationKind, MessageEdge, MessageRole,
    ParsedCallable,
    ir::{
        ast::{Expr, Stmt},
        project::TypedFileRecord,
    },
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
                let known_values = known_values_for_source(&snapshots, &source.file_path);
                let declared =
                    resolve_declared_edge(edge, callable, source, &snapshots, &known_values);
                if (edge.role != MessageRole::Producer && callable.metadata.name.starts_with("New"))
                    || has_ambiguous_method_name(callable, &snapshots)
                {
                    return declared;
                }
                let mut resolved = Vec::new();
                for invocation in matching_calls(callable, source, &snapshots) {
                    for edge in &declared {
                        resolved.extend(resolve_invocation(
                            edge,
                            callable,
                            invocation,
                            &snapshots,
                            &known_values,
                            0,
                            &mut HashSet::new(),
                        ));
                    }
                }
                if resolved.is_empty() {
                    declared
                } else {
                    resolved
                }
            })
            .collect();
    }
}

/// Returns true when multiple method implementations share a name across the project.
fn has_ambiguous_method_name(callable: &ParsedCallable, files: &[FileSnapshot]) -> bool {
    matches!(callable.metadata.namespace, models::Namespace::Class(_))
        && files
            .iter()
            .flat_map(|file| &file.callables)
            .filter(|candidate| candidate.metadata.name == callable.metadata.name)
            .count()
            > 1
}

struct FileSnapshot {
    file_path: String,
    import_modules: Vec<String>,
    callables: Vec<ParsedCallable>,
    calls: Vec<CallStatement>,
    assignments: Vec<(String, String)>,
}

#[derive(Clone, Copy)]
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
    let matching_definitions = files
        .iter()
        .flat_map(|file| &file.callables)
        .filter(|candidate| candidate.metadata.name == callable.metadata.name)
        .count();
    if matching_definitions != 1 {
        return Vec::new();
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
    resolve_edge_fields(
        edge,
        callable,
        &bindings,
        &invocation.file.file_path,
        files,
        known_values,
    )
}

/// Resolves constructor-backed transport fields without applying any call arguments.
fn resolve_declared_edge(
    edge: &MessageEdge,
    callable: &ParsedCallable,
    source: &FileSnapshot,
    files: &[FileSnapshot],
    known_values: &HashMap<String, Vec<String>>,
) -> Vec<MessageEdge> {
    resolve_edge_fields(
        edge,
        callable,
        &HashMap::new(),
        &source.file_path,
        files,
        known_values,
    )
}

/// Resolves transport fields using the supplied parameter bindings and source file path.
fn resolve_edge_fields(
    edge: &MessageEdge,
    callable: &ParsedCallable,
    bindings: &HashMap<&str, &str>,
    file_path: &str,
    files: &[FileSnapshot],
    known_values: &HashMap<String, Vec<String>>,
) -> Vec<MessageEdge> {
    let aliases = snapshot_for(files, &callable.metadata.file_path)
        .map(|file| file.assignments.iter().cloned().collect::<HashMap<_, _>>())
        .unwrap_or_default();
    let exchanges = values_for(
        edge.exchange.as_deref(),
        &bindings,
        &aliases,
        known_values,
        callable,
        files,
    );
    let routing_keys = values_for(
        edge.routing_key.as_deref(),
        &bindings,
        &aliases,
        known_values,
        callable,
        files,
    );
    let queues = values_for(
        edge.queue.as_deref(),
        &bindings,
        &aliases,
        known_values,
        callable,
        files,
    );
    let topics = values_for(
        edge.topic.as_deref(),
        &bindings,
        &aliases,
        known_values,
        callable,
        files,
    );

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
            file_path: file_path.to_string(),
            ..edge
        })
        .collect()
}

/// Collects statically known configuration values by selector suffix.
fn known_values(files: &[FileSnapshot]) -> HashMap<String, Vec<String>> {
    known_values_from(files.iter())
}

/// Collects statically known configuration values from a set of file snapshots.
fn known_values_from<'a>(
    files: impl IntoIterator<Item = &'a FileSnapshot>,
) -> HashMap<String, Vec<String>> {
    let mut values = HashMap::<String, Vec<String>>::new();
    for (name, value) in files.into_iter().flat_map(|file| &file.assignments) {
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

/// Collects constants from the service subtree containing the adapter source file.
fn known_values_for_source(
    files: &[FileSnapshot],
    file_path: &str,
) -> HashMap<String, Vec<String>> {
    let Some(root) = service_root(file_path) else {
        return known_values(files);
    };
    known_values_from(
        files
            .iter()
            .filter(|file| service_root(&file.file_path) == Some(root)),
    )
}

/// Returns the path prefix preceding a Go service's `internal` package.
fn service_root(file_path: &str) -> Option<&str> {
    file_path
        .split("/internal/")
        .next()
        .filter(|root| *root != file_path)
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
    callable: &ParsedCallable,
    files: &[FileSnapshot],
) -> Vec<Option<String>> {
    let Some(value) = value else {
        return vec![None];
    };
    let resolved = receiver_accessor_values(value, callable, files)
        .or_else(|| receiver_field_values(value, callable, files))
        .or_else(|| producer_event_key(value, callable))
        .unwrap_or_else(|| binding_values(value, bindings, aliases, 0));
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

/// Resolves receiver accessor methods that return a constructor-backed field.
fn receiver_accessor_values(
    value: &str,
    callable: &ParsedCallable,
    files: &[FileSnapshot],
) -> Option<Vec<String>> {
    let value = value.strip_suffix("()")?;
    let (receiver, method) = value.split_once('.')?;
    let receiver_type = match &callable.metadata.namespace {
        models::Namespace::Class(name) => name,
        models::Namespace::Module(_) => return None,
    };
    if method_receiver_name(callable) != Some(receiver) {
        return None;
    }
    files
        .iter()
        .flat_map(|file| &file.callables)
        .find(|candidate| {
            candidate.metadata.name == method
                && matches!(&candidate.metadata.namespace, models::Namespace::Class(name) if name == receiver_type)
        })
        .and_then(|accessor| {
            accessor.ast.statements.iter().find_map(|statement| match statement {
                Stmt::Return(Expr::Attr { object, field })
                    if matches!(object.as_ref(), Expr::Var(name) if name == receiver) =>
                {
                    receiver_field_values(&format!("{receiver}.{field}"), callable, files)
                }
                _ => None,
            })
        })
}

/// Derives a typed AMQP adapter's static routing-key constant from its producer name.
fn producer_event_key(value: &str, callable: &ParsedCallable) -> Option<Vec<String>> {
    if !value.ends_with(".Key()") {
        return None;
    }
    let producer = match &callable.metadata.namespace {
        models::Namespace::Class(name) => name.strip_suffix("Producer")?,
        models::Namespace::Module(_) => return None,
    };
    Some(vec![format!("constant.{producer}Key")])
}

/// Resolves `receiver.field` values initialized in constructors for the receiver type.
fn receiver_field_values(
    value: &str,
    callable: &ParsedCallable,
    files: &[FileSnapshot],
) -> Option<Vec<String>> {
    let (receiver, selector) = value.split_once('.')?;
    let (field, suffix) = selector
        .split_once('.')
        .map_or((selector, None), |(field, suffix)| (field, Some(suffix)));
    let receiver_type = match &callable.metadata.namespace {
        models::Namespace::Class(name) => name,
        models::Namespace::Module(_) => return None,
    };
    if method_receiver_name(callable) != Some(receiver) {
        return None;
    }

    files
        .iter()
        .flat_map(|file| &file.callables)
        .filter(|candidate| candidate.metadata.name.starts_with("New"))
        .filter_map(|constructor| constructor_field_value(constructor, receiver_type, field))
        .map(|values| {
            suffix.map_or(values.clone(), |suffix| {
                values
                    .iter()
                    .map(|value| format!("{value}.{suffix}"))
                    .collect()
            })
        })
        .next()
}

/// Extracts the variable name from a Go method receiver in its callable signature.
fn method_receiver_name(callable: &ParsedCallable) -> Option<&str> {
    callable
        .metadata
        .signature
        .strip_prefix("func (")?
        .split_once(')')?
        .0
        .split_whitespace()
        .next()
}

/// Returns a constructor field after resolving constructor-local variables.
fn constructor_field_value(
    constructor: &ParsedCallable,
    receiver_type: &str,
    field: &str,
) -> Option<Vec<String>> {
    let mut locals = HashMap::new();
    for statement in &constructor.ast.statements {
        match statement {
            Stmt::Declaration { name, value, .. } | Stmt::Assignment { name, value } => {
                if let Some(value) = struct_field_value(value, receiver_type, field, &locals) {
                    return Some(vec![value]);
                }
                locals.insert(name.as_str(), expression_text(value, &locals));
            }
            Stmt::Return(value) => {
                if let Some(value) = struct_field_value(value, receiver_type, field, &locals) {
                    return Some(vec![value]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Reads one field from a constructor's concrete receiver struct literal.
fn struct_field_value(
    expression: &Expr,
    receiver_type: &str,
    field: &str,
    locals: &HashMap<&str, String>,
) -> Option<String> {
    let Expr::StructLiteral { type_name, fields } = expression else {
        return None;
    };
    type_name
        .as_deref()
        .filter(|name| name.trim_start_matches('&').ends_with(receiver_type))?;
    fields
        .iter()
        .find(|(name, _)| name == field)
        .map(|(_, value)| expression_text(value, locals))
}

/// Converts the subset of constructor expressions used for transport fields into text.
fn expression_text<'a>(expression: &'a Expr, locals: &HashMap<&str, String>) -> String {
    match expression {
        Expr::Literal(value) | Expr::Var(value) => locals
            .get(value.as_str())
            .cloned()
            .unwrap_or_else(|| value.clone()),
        Expr::Attr { object, field } => format!("{}.{}", expression_text(object, locals), field),
        _ => String::new(),
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
    let bound = aliases.get(&bound).cloned().unwrap_or(bound);
    if let Some(values) = composite_selector_values(&bound, suffix) {
        return values;
    }
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

/// Resolves a selector path from a Go struct literal passed through a constructor.
fn composite_selector_values(value: &str, selector: &str) -> Option<Vec<String>> {
    let (field, remainder) = selector
        .split_once('.')
        .map_or((selector, None), |(field, remainder)| {
            (field, Some(remainder))
        });
    let selected = composite_field_value(value, field)?;
    match remainder {
        Some(remainder) => composite_selector_values(selected, remainder),
        None => Some(vec![selected.to_string()]),
    }
}

/// Reads a named top-level field from a Go composite literal without flattening nested values.
fn composite_field_value<'a>(value: &'a str, field: &str) -> Option<&'a str> {
    let start = value.find('{')? + 1;
    let end = value.rfind('}')?;
    let body = &value[start..end];
    let mut depth = 0usize;
    let mut item_start = 0usize;
    for (index, character) in body
        .char_indices()
        .chain(std::iter::once((body.len(), ',')))
    {
        match character {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let item = body[item_start..index].trim();
                if let Some((name, value)) = item.split_once(':')
                    && name.trim() == field
                {
                    return Some(value.trim());
                }
                item_start = index + 1;
            }
            _ => {}
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::composite_selector_values;

    #[test]
    fn resolves_nested_constructor_configuration_selectors() {
        let config = r#"Config{
            OrderActionExchange: "order-action-exchange",
            PaymentQueue: QueueConfig{Name: "payment-action-queue"},
        }"#;

        assert_eq!(
            composite_selector_values(config, "OrderActionExchange"),
            Some(vec!["\"order-action-exchange\"".to_string()])
        );
        assert_eq!(
            composite_selector_values(config, "PaymentQueue.Name"),
            Some(vec!["\"payment-action-queue\"".to_string()])
        );
    }
}
