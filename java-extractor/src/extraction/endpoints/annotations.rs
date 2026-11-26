use models::{Endpoint, HttpMethod};
use tree_sitter::Node;

static HTTP_METHOD_ANNOTATIONS: &[(&str, HttpMethod)] = &[
    ("GetMapping", HttpMethod::GET),
    ("GET", HttpMethod::GET),
    ("PostMapping", HttpMethod::POST),
    ("POST", HttpMethod::POST),
    ("PutMapping", HttpMethod::PUT),
    ("PUT", HttpMethod::PUT),
    ("DeleteMapping", HttpMethod::DELETE),
    ("DELETE", HttpMethod::DELETE),
    ("PatchMapping", HttpMethod::PATCH),
    ("PATCH", HttpMethod::PATCH),
];

const URI_PATH_ANNOTATIONS: &[&str] = &[
    "RequestMapping",
    "Path",
    "GetMapping",
    "PostMapping",
    "PutMapping",
    "DeleteMapping",
    "PatchMapping",
];

/// Simple name of an annotation: `@org.x.FeignClient(..)` -> `FeignClient`.
pub fn annotation_name(annot: &str) -> &str {
    let head = annot.split('(').next().unwrap_or(annot);
    let head = head.trim().trim_start_matches('@');
    head.rsplit('.').next().unwrap_or(head).trim()
}

pub fn are_annotations_specifying_endpoint(annotations: &[String]) -> bool {
    annotations.iter().any(|annot| {
        let name = annotation_name(annot);
        URI_PATH_ANNOTATIONS.contains(&name) || http_method_from_annotation(annot).is_some()
    })
}

pub fn apply_annotations(endpoint: &mut Endpoint, annotations: &[String]) {
    for annot in annotations {
        // 1. HTTP method
        if let Some(method) = http_method_from_annotation(annot) {
            endpoint.http_method = method;
        }

        // 2. URI path
        if URI_PATH_ANNOTATIONS.contains(&annotation_name(annot))
            && let Some(uri) = extract_uris(annot).into_iter().next()
        {
            endpoint.uri = uri;
        }
    }
}

fn http_method_from_annotation(annot: &str) -> Option<HttpMethod> {
    let name = annotation_name(annot);
    if name == "RequestMapping" {
        return annotation_args(annot)
            .into_iter()
            .find(|(arg, _)| *arg == Some("method"))
            .and_then(|(_, value)| request_method(value));
    }
    HTTP_METHOD_ANNOTATIONS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, method)| *method)
}

/// `RequestMethod.POST` or `{RequestMethod.POST, ...}` -> `POST` (first item).
fn request_method(value: &str) -> Option<HttpMethod> {
    let first = array_items(value).into_iter().next()?;
    let verb = first.rsplit('.').next()?.trim();
    HttpMethod::ALL
        .into_iter()
        .find(|m| format!("{m:?}") == verb)
}

/// String-literal paths of an annotation's `value`/`path` (or unnamed) argument.
/// `@RequestMapping({"/a", "/b"})` -> `["/a", "/b"]`. Non-literals are ignored.
pub fn extract_uris(annot: &str) -> Vec<String> {
    annotation_args(annot)
        .into_iter()
        .filter(|(name, _)| matches!(name, None | Some("value") | Some("path")))
        .flat_map(|(_, value)| array_items(value))
        .filter(|item| item.len() >= 2 && item.starts_with('"') && item.ends_with('"'))
        .map(|item| item[1..item.len() - 1].to_string())
        .collect()
}

/// Top-level arguments of an annotation as `(name, value)`; unnamed -> `None`.
fn annotation_args(annot: &str) -> Vec<(Option<&str>, &str)> {
    let (Some(start), Some(end)) = (annot.find('('), annot.rfind(')')) else {
        return Vec::new();
    };
    if end <= start {
        return Vec::new();
    }
    split_top_level(&annot[start + 1..end], ',')
        .into_iter()
        .filter(|arg| !arg.is_empty())
        .map(|arg| match split_top_level(arg, '=').as_slice() {
            [name, _, ..] if is_identifier(name) => {
                let value = arg.split_once('=').map_or(arg, |(_, v)| v);
                (Some(*name), value.trim())
            }
            _ => (None, arg),
        })
        .collect()
}

fn is_identifier(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// `{a, b}` -> `[a, b]`; anything else -> `[value]`.
fn array_items(value: &str) -> Vec<&str> {
    match value.strip_prefix('{').and_then(|v| v.strip_suffix('}')) {
        Some(inner) => split_top_level(inner, ',')
            .into_iter()
            .filter(|item| !item.is_empty())
            .collect(),
        None => vec![value],
    }
}

/// Splits on `sep` outside string literals and braces/parens; parts are trimmed.
fn split_top_level(s: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut in_str, mut escaped, mut last) = (0i32, false, false, 0);
    for (i, c) in s.char_indices() {
        if in_str {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' | '(' => depth += 1,
            '}' | ')' => depth -= 1,
            _ if c == sep && depth == 0 => {
                parts.push(s[last..i].trim());
                last = i + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(s[last..].trim());
    parts
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
