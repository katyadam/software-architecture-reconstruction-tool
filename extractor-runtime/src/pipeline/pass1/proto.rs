use std::{collections::HashMap, sync::OnceLock};

use models::{
    ProtoService,
    ir::{language::Language, syntax::FileRecord},
};
use regex::Regex;

const SERVICE_KEYWORD: &str = "service";
const RPC_KEYWORD: &str = "rpc";
const SERVICE_PATTERN: &str = r"\b{}\s+(?P<service>[A-Za-z_][A-Za-z0-9_]*)\s*\{";
const RPC_PATTERN: &str = r"\b{}\s+(?P<operation>[A-Za-z_][A-Za-z0-9_]*)\s*\(";

static SERVICE_REGEX: OnceLock<Regex> = OnceLock::new();
static RPC_REGEX: OnceLock<Regex> = OnceLock::new();

fn service_regex() -> &'static Regex {
    SERVICE_REGEX.get_or_init(|| {
        Regex::new(&SERVICE_PATTERN.replace("{}", SERVICE_KEYWORD))
            .expect("valid protobuf service regex")
    })
}

fn rpc_regex() -> &'static Regex {
    RPC_REGEX.get_or_init(|| {
        Regex::new(&RPC_PATTERN.replace("{}", RPC_KEYWORD)).expect("valid protobuf rpc regex")
    })
}

/// Extracts protobuf service declarations for the project-level gRPC contract index.
pub(super) fn extract_syntactic(text: &str, file_path: &str) -> FileRecord {
    FileRecord {
        file_path: file_path.to_string(),
        // `.proto` is language-neutral; Java is the existing neutral enum value.
        language: Language::Java,
        imports: vec![],
        entities: vec![],
        endpoints: vec![],
        callables: vec![],
        call_statements: vec![],
        assignments: HashMap::new(),
        enums: vec![],
        raw_restcalls: vec![],
        raw_message_edges: vec![],
        proto_services: service_regex()
            .captures_iter(text)
            .filter_map(|service| {
                let body = service_body(text, service.get(0)?.end())?;
                Some(ProtoService {
                    name: service["service"].to_string(),
                    operations: rpc_regex()
                        .captures_iter(body)
                        .map(|rpc| rpc["operation"].to_string())
                        .collect(),
                    file_path: file_path.to_string(),
                })
            })
            .collect(),
    }
}

/// Return the content of a `service { ... }` block beginning just after its
/// opening brace. RPC declarations commonly use `{}` bodies, so a non-greedy
/// regex would stop after the first operation instead of the service block.
fn service_body(text: &str, body_start: usize) -> Option<&str> {
    let mut depth = 1;
    for (offset, character) in text[body_start..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[body_start..body_start + offset]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::extract_syntactic;

    #[test]
    fn indexes_proto_service_operations() {
        let record = extract_syntactic(
            "service DocumentService { rpc GetDocument (Request) returns (Document); rpc DeleteDocument (Request) returns (Empty); }",
            "document.proto",
        );

        assert_eq!(record.proto_services[0].name, "DocumentService");
        assert_eq!(
            record.proto_services[0].operations,
            vec!["GetDocument", "DeleteDocument"]
        );
    }

    #[test]
    fn indexes_every_operation_with_rpc_bodies() {
        let record = extract_syntactic(
            "service TodoService { rpc Create (Request) returns (Todo) {} rpc Read (Request) returns (Todo) {} rpc Delete (Request) returns (Empty) {} }",
            "todo.proto",
        );

        assert_eq!(record.proto_services[0].name, "TodoService");
        assert_eq!(
            record.proto_services[0].operations,
            vec!["Create", "Read", "Delete"]
        );
    }
}
