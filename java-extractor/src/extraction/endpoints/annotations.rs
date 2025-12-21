use models::{Endpoint, HttpMethod};

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

pub fn apply_annotations(endpoint: &mut Endpoint, annotations: Vec<String>) {
    for annot in annotations {
        // 1. HTTP method
        if let Some(method) = http_method_from_annotation(&annot) {
            endpoint.http_method = method;
        }

        // 2. URI path
        if URI_PATH_ANNOTATIONS.iter().any(|a| annot.starts_with(a)) {
            if let Some(uri) = extract_uri(&annot) {
                endpoint.uri = uri;
            }
        }
    }
}

fn http_method_from_annotation(annot: &str) -> Option<HttpMethod> {
    HTTP_METHOD_PREFIXES
        .iter()
        .find(|(prefix, _)| annot.starts_with(prefix))
        .map(|(_, method)| method.clone())
}

pub fn extract_uri(annot: &str) -> Option<String> {
    let start = annot.find('(')? + 1;
    let end = annot.rfind(')')?;
    let inside = annot[start..end].trim();

    // Case 1: unnamed argument → "/users"
    if inside.starts_with('"') {
        return Some(inside.trim_matches('"').to_string());
    }

    // Case 2: named argument → value = "/users" | path = "/users"
    if let Some((name, value)) = inside.split_once('=') {
        let name = name.trim();
        let value = value.trim();

        if name == "value" || name == "path" {
            return Some(value.trim_matches('"').to_string());
        }
    }

    None
}
