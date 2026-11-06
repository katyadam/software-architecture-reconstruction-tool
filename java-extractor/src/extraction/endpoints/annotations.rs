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

/// Cap on paths, prefixes and expanded endpoints per method (mirrors pass 2 `MAX_CHAINS`).
pub const MAX_PATHS: usize = 256;

const URI_PATH_ANNOTATIONS: &[&str] = &[
    "@RequestMapping",
    "@Path",
    "@GetMapping",
    "@PostMapping",
    "@PutMapping",
    "@DeleteMapping",
    "@PatchMapping",
];

/// Annotation name without arguments: `@Path("/a")` -> `@Path`.
fn annotation_name(annot: &str) -> &str {
    annot.split('(').next().unwrap_or(annot).trim()
}

/// `annot` is exactly `@name`, so `@PathVariable` is not `@Path`.
pub fn is_named(annot: &str, name: &str) -> bool {
    annotation_name(annot) == name
}

fn is_uri_annotation(annot: &str) -> bool {
    URI_PATH_ANNOTATIONS.iter().any(|a| is_named(annot, a))
}

pub fn are_annotations_specifying_endpoint(annotations: &[String]) -> bool {
    annotations.iter().any(|annot| {
        is_uri_annotation(annot)
            || HTTP_METHOD_PREFIXES
                .iter()
                .any(|(a, _)| annot.starts_with(a))
    })
}

/// One endpoint per (method path x HTTP method) declared by `annotations`.
pub fn expand_annotations(endpoint: Endpoint, annotations: &[String]) -> Vec<Endpoint> {
    let mut methods = vec![endpoint.http_method];
    let mut uris = vec![endpoint.uri.clone()];
    for annot in annotations {
        if let Some(method) = http_method_from_annotation(annot) {
            methods = vec![method];
        }
        if is_named(annot, "@RequestMapping") {
            let declared = request_methods(annot);
            if !declared.is_empty() {
                methods = declared;
            }
        }
        if is_uri_annotation(annot) {
            let declared = extract_uris(annot);
            if !declared.is_empty() {
                uris = declared;
            }
        }
    }
    let methods = distinct_capped(methods, "HTTP methods");
    let uris = distinct_capped(uris, "method paths");
    let pairs = uris
        .iter()
        .flat_map(|uri| methods.iter().map(move |method| (uri, method)));
    distinct_capped(pairs, "endpoint expansion")
        .into_iter()
        .map(|(uri, method)| Endpoint {
            uri: uri.clone(),
            http_method: *method,
            ..endpoint.clone()
        })
        .collect()
}

/// First `MAX_PATHS` distinct items in order; one warning on truncation.
pub fn distinct_capped<T: PartialEq>(items: impl IntoIterator<Item = T>, what: &str) -> Vec<T> {
    let mut out = Vec::new();
    for item in items {
        if out.contains(&item) {
            continue;
        }
        if out.len() == MAX_PATHS {
            log::warn!("{what} truncated at {MAX_PATHS}");
            break;
        }
        out.push(item);
    }
    out
}

fn http_method_from_annotation(annot: &str) -> Option<HttpMethod> {
    HTTP_METHOD_PREFIXES
        .iter()
        .find(|(prefix, _)| annot.starts_with(prefix))
        .map(|(_, method)| method.clone())
}

/// Top-level arguments of `annot`: `@A(x, y = z)` -> `["x", "y = z"]`.
fn annotation_args(annot: &str) -> Vec<&str> {
    let Some(start) = annot.find('(') else {
        return vec![];
    };
    let end = annot.rfind(')').unwrap_or(annot.len());
    annot
        .get(start + 1..end)
        .map(split_top_level_args)
        .unwrap_or_default()
}

/// Elements of `value`: `{a, b}` -> `[a, b]`, `a` -> `[a]`.
fn elements(value: &str) -> Vec<&str> {
    match value.strip_prefix('{').and_then(|v| v.strip_suffix('}')) {
        Some(inner) => split_top_level_args(inner),
        None => vec![value],
    }
}

/// `method = RequestMethod.POST` or `method = {RequestMethod.GET, ...}`; empty when absent.
fn request_methods(annot: &str) -> Vec<HttpMethod> {
    annotation_args(annot)
        .into_iter()
        .filter_map(named_arg)
        .filter(|(name, _)| *name == "method")
        .flat_map(|(_, value)| elements(value))
        .filter_map(|m| m.rsplit('.').next()?.trim().parse().ok())
        .collect()
}

/// Raw path elements of a mapping annotation, quotes kept:
/// `@RequestMapping({"/a", X})` -> `["\"/a\"", "X"]`.
pub fn path_values(annot: &str) -> Vec<&str> {
    // Unnamed first argument -> "/users"; otherwise named `value = ...` | `path = ...`
    annotation_args(annot)
        .into_iter()
        .enumerate()
        .find_map(|(i, arg)| match named_arg(arg) {
            Some((name, value)) if name == "value" || name == "path" => Some(value),
            None if i == 0 => Some(arg),
            _ => None,
        })
        .map(elements)
        .unwrap_or_default()
}

/// Every path of a mapping annotation: `@GetMapping({"/a", "/b"})` -> `["/a", "/b"]`.
/// Non-literal values are returned raw (`path = Constants.X` -> `Constants.X`).
pub fn extract_uris(annot: &str) -> Vec<String> {
    path_values(annot)
        .into_iter()
        .map(|v| first_string_literal(v).unwrap_or(v).to_string())
        .collect()
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
pub fn first_string_literal(value: &str) -> Option<&str> {
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
    use models::HttpMethod;

    use models::Endpoint;

    use super::{MAX_PATHS, expand_annotations, extract_uris, is_named, request_methods};

    /// First path, as callers took it before arrays were expanded.
    fn uri(annot: &str) -> Option<String> {
        extract_uris(annot).into_iter().next()
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

    #[test]
    fn uris_of_arrays() {
        assert_eq!(
            extract_uris(r#"@GetMapping({"/a", "/b"})"#),
            vec!["/a", "/b"]
        );
        assert_eq!(
            extract_uris(r#"@RequestMapping(path = {"/a", "/b"}, produces = X)"#),
            vec!["/a", "/b"]
        );
        assert_eq!(extract_uris(r#"@GetMapping("/a")"#), vec!["/a"]);
        assert!(extract_uris("@GetMapping").is_empty());
    }

    #[test]
    fn uris_of_non_literals() {
        assert_eq!(
            extract_uris("@RequestMapping(Paths.BASE)"),
            vec!["Paths.BASE"]
        );
        assert_eq!(
            extract_uris(r#"@RequestMapping({Paths.A, "/b"})"#),
            vec!["Paths.A", "/b"]
        );
    }

    #[test]
    fn methods_single_and_array() {
        assert_eq!(
            request_methods(r#"@RequestMapping(value = "/a", method = RequestMethod.POST)"#),
            vec![HttpMethod::POST]
        );
        assert_eq!(
            request_methods(
                r#"@RequestMapping(value = "/a", method = {RequestMethod.GET, RequestMethod.PUT})"#
            ),
            vec![HttpMethod::GET, HttpMethod::PUT]
        );
        assert_eq!(
            request_methods("@RequestMapping(method = DELETE)"),
            vec![HttpMethod::DELETE]
        );
        assert!(request_methods(r#"@RequestMapping("/a")"#).is_empty());
    }

    #[test]
    fn path_variable_is_not_path() {
        assert!(!is_named(r#"@PathVariable("id")"#, "@Path"));
        assert!(!is_named(r#"@PathParam("id")"#, "@Path"));
        assert!(is_named(r#"@Path("/a")"#, "@Path"));
        assert!(is_named("@Path", "@Path"));
    }

    fn expanded(annot: &str) -> Vec<(HttpMethod, String)> {
        expand_annotations(Endpoint::default(), &[annot.to_string()])
            .into_iter()
            .map(|e| (e.http_method, e.uri))
            .collect()
    }

    #[test]
    fn duplicate_methods_and_paths_deduped() {
        assert_eq!(
            expanded(
                r#"@RequestMapping(value = {"/a", "/a"}, method = {RequestMethod.GET, RequestMethod.GET})"#
            ),
            vec![(HttpMethod::GET, "/a".to_string())]
        );
    }

    #[test]
    fn oversize_path_array_truncated() {
        let paths: Vec<String> = (0..MAX_PATHS + 10).map(|i| format!("\"/p{i}\"")).collect();
        let annot = format!(
            "@RequestMapping(value = {{{}}}, method = {{RequestMethod.GET, RequestMethod.PUT}})",
            paths.join(", ")
        );
        let out = expanded(&annot);
        assert_eq!(out.len(), MAX_PATHS);
        assert_eq!(out[0], (HttpMethod::GET, "/p0".to_string()));
    }
}
