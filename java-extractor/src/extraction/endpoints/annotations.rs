use models::{Endpoint, HttpMethod};
use tree_sitter::Node;

static HTTP_METHOD_PREFIXES: &[(&str, HttpMethod)] = &[
    ("@GetMapping", HttpMethod::GET),
    ("@GET", HttpMethod::GET),
    ("@PostMapping", HttpMethod::POST),
    ("@POST", HttpMethod::POST),
    ("@PutMapping", HttpMethod::PUT),
    ("@PUT", HttpMethod::PUT),
    ("@DeleteMapping", HttpMethod::DELETE),
    ("@DELETE", HttpMethod::DELETE),
    ("@PatchMapping", HttpMethod::PATCH),
    ("@PATCH", HttpMethod::PATCH),
];

const URI_PATH_ANNOTATIONS: &[&str] = &[
    "@RequestMapping",
    "@Path",
    "@GetMapping",
    "@PostMapping",
    "@PutMapping",
    "@DeleteMapping",
    "@PatchMapping",
];

pub fn are_annotations_specifying_endpoint(annotations: &[String]) -> bool {
    annotations.iter().any(|annot| {
        URI_PATH_ANNOTATIONS.iter().any(|a| annot.starts_with(a))
            || HTTP_METHOD_PREFIXES
                .iter()
                .any(|(a, _)| annot.starts_with(a))
    })
}

pub fn apply_annotations(endpoint: &mut Endpoint, annotations: &[String]) {
    for annot in annotations {
        // 1. HTTP method
        if let Some(method) = http_method_from_annotation(annot) {
            endpoint.http_method = method;
        }

        // 2. URI path
        if URI_PATH_ANNOTATIONS.iter().any(|a| annot.starts_with(a))
            && let Some(uri) = extract_uri(annot)
        {
            endpoint.uri = uri;
        }
    }
}

fn http_method_from_annotation(annot: &str) -> Option<HttpMethod> {
    HTTP_METHOD_PREFIXES
        .iter()
        .find(|(prefix, _)| annot.starts_with(prefix))
        .map(|(_, method)| method.clone())
}

/// Extracts the URI path from a mapping annotation, e.g.
/// `@GetMapping(path = "/a", produces = X)` -> `/a`, `@RequestMapping({"/a", "/b"})` -> `/a`.
/// Non-literal values are returned raw (`path = Constants.X` -> `Constants.X`).
pub fn extract_uri(annot: &str) -> Option<String> {
    let start = annot.find('(')? + 1;
    let end = annot.rfind(')')?;
    let args = split_top_level_args(annot.get(start..end)?);

    // Unnamed first argument -> "/users"; otherwise named `value = ...` | `path = ...`
    let value = args
        .iter()
        .enumerate()
        .find_map(|(i, arg)| match named_arg(arg) {
            Some((name, value)) if name == "value" || name == "path" => Some(value),
            None if i == 0 => Some(*arg),
            _ => None,
        })?;
    Some(first_string_literal(value).unwrap_or(value).to_string())
}

/// Splits annotation arguments on commas outside string literals, braces and parens.
fn split_top_level_args(inside: &str) -> Vec<&str> {
    let (mut args, mut depth, mut in_str, mut escaped, mut last) =
        (Vec::new(), 0usize, false, false, 0);
    for (i, c) in inside.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if in_str => escaped = true,
            '"' => in_str = !in_str,
            '{' | '(' if !in_str => depth += 1,
            '}' | ')' if !in_str => depth = depth.saturating_sub(1),
            ',' if !in_str && depth == 0 => {
                args.push(inside[last..i].trim());
                last = i + 1;
            }
            _ => {}
        }
    }
    args.push(inside[last..].trim());
    args.retain(|a| !a.is_empty());
    args
}

/// `name = value` -> `Some((name, value))` when `name` is an identifier.
fn named_arg(arg: &str) -> Option<(&str, &str)> {
    let (name, value) = arg.split_once('=')?;
    let name = name.trim();
    (!name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .then_some((name, value.trim()))
}

/// Contents of the first `"..."` literal in `value`, if any.
fn first_string_literal(value: &str) -> Option<&str> {
    let rest = &value[value.find('"')? + 1..];
    let mut escaped = false;
    let end = rest.char_indices().find_map(|(i, c)| {
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '"' => return Some(i),
            _ => {}
        }
        None
    })?;
    Some(&rest[..end])
}

/// Collects every `annotation` and `marker_annotation` child of `modifiers_node`
/// as raw source text (e.g. `"@GetMapping(\"/users\")"` or `"@Override"`).
pub fn get_annotations_from_modifiers(modifiers_node: Node, code: &str) -> Vec<String> {
    let mut annotations = Vec::new();

    let mut cursor = modifiers_node.walk();
    for child in modifiers_node.children(&mut cursor) {
        if child.kind() == "annotation" || child.kind() == "marker_annotation" {
            annotations.push(code[child.start_byte()..child.end_byte()].to_string());
        }
    }

    annotations
}

#[cfg(test)]
mod tests {
    use super::extract_uri;

    fn uri(annot: &str) -> Option<String> {
        extract_uri(annot)
    }

    #[test]
    fn unnamed_literal() {
        assert_eq!(uri(r#"@GetMapping("/users")"#).as_deref(), Some("/users"));
        assert_eq!(uri(r#"@GetMapping( "/users" )"#).as_deref(), Some("/users"));
    }

    #[test]
    fn named_value_or_path() {
        assert_eq!(
            uri(r#"@GetMapping(value = "/users")"#).as_deref(),
            Some("/users")
        );
        assert_eq!(
            uri(r#"@GetMapping(path = "/users")"#).as_deref(),
            Some("/users")
        );
    }

    #[test]
    fn multiple_attributes() {
        assert_eq!(
            uri(r#"@GetMapping( path = "/organizations", produces = MediaType.APPLICATION_JSON_VALUE )"#)
                .as_deref(),
            Some("/organizations")
        );
        assert_eq!(
            uri(r#"@PostMapping(value = "/a", consumes = X, produces = Y)"#).as_deref(),
            Some("/a")
        );
        assert_eq!(
            uri(r#"@GetMapping(produces = X, path = "/a")"#).as_deref(),
            Some("/a")
        );
        assert_eq!(
            uri(r#"@GetMapping("/a", produces = X)"#).as_deref(),
            Some("/a")
        );
    }

    #[test]
    fn arrays_take_first_path() {
        assert_eq!(
            uri(r#"@RequestMapping(value = {"/a", "/b"})"#).as_deref(),
            Some("/a")
        );
        assert_eq!(uri(r#"@RequestMapping({"/a"})"#).as_deref(), Some("/a"));
    }

    #[test]
    fn comma_or_equals_inside_literal() {
        assert_eq!(
            uri(r#"@GetMapping("/a,b=c", produces = X)"#).as_deref(),
            Some("/a,b=c")
        );
        assert_eq!(
            uri(r#"@GetMapping(path = "/x=1,2")"#).as_deref(),
            Some("/x=1,2")
        );
    }

    #[test]
    fn non_literal_and_missing_path() {
        assert_eq!(
            uri("@RequestMapping(path = Constants.X)").as_deref(),
            Some("Constants.X")
        );
        assert_eq!(uri("@GetMapping(produces = X)"), None);
        assert_eq!(uri("@GetMapping()"), None);
        assert_eq!(uri("@GetMapping"), None);
    }
}
