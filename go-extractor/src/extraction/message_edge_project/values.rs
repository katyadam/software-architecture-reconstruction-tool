use std::collections::HashMap;

use models::{
    ParsedCallable,
    ir::ast::{Expr, Stmt},
};

use super::FileSnapshot;

const MAX_RESOLUTION_DEPTH: usize = 4;

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
pub(super) fn known_values_for_source(
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
pub(super) fn values_for(
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
                && matches!(&candidate.metadata.namespace, models::Namespace::Class(name) if *name == *receiver_type)
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
        .map(|values: Vec<String>| {
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
