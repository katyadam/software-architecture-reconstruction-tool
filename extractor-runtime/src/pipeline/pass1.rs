use std::collections::HashMap;

use models::{
    ProtoService,
    api::ExtractionError,
    ir::{language::Language, syntax::FileRecord},
};
use regex::Regex;

/// Counterpart to [`dispatch`]: extracts a single file into a [`FileRecord`]
/// (Pass 1 only — no cross-file resolution).
pub fn dispatch_syntactic(
    text: &str,
    file_path: &str,
) -> Result<Option<FileRecord>, ExtractionError> {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str());
    match ext {
        Some("java") => java_extractor::extraction::extract_syntactic(text, file_path).map(Some),
        Some("py") => {
            python_extractor::extraction::parse::extract_syntactic(text, file_path).map(Some)
        }
        Some("proto") => Ok(Some(extract_proto_syntactic(text, file_path))),
        _ => Ok(None),
    }
}

fn extract_proto_syntactic(text: &str, file_path: &str) -> FileRecord {
    let service_re =
        Regex::new(r"(?s)\bservice\s+(?P<service>[A-Za-z_][A-Za-z0-9_]*)\s*\{(?P<body>.*?)\}")
            .expect("valid protobuf service regex");
    let rpc_re = Regex::new(r"\brpc\s+(?P<operation>[A-Za-z_][A-Za-z0-9_]*)\s*\(")
        .expect("valid protobuf rpc regex");

    FileRecord {
        file_path: file_path.to_string(),
        // `.proto` is language-neutral. This value is not used for source parsing.
        language: Language::Java,
        imports: vec![],
        entities: vec![],
        endpoints: vec![],
        callables: vec![],
        call_statements: vec![],
        assignments: HashMap::new(),
        enums: vec![],
        raw_restcalls: vec![],
        proto_services: service_re
            .captures_iter(text)
            .map(|service| ProtoService {
                name: service["service"].to_string(),
                operations: rpc_re
                    .captures_iter(&service["body"])
                    .map(|rpc| rpc["operation"].to_string())
                    .collect(),
                file_path: file_path.to_string(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::dispatch_syntactic;

    #[test]
    fn indexes_proto_service_operations() {
        let record = dispatch_syntactic(
            "service DocumentService { rpc GetDocument (Request) returns (Document); rpc DeleteDocument (Request) returns (Empty); }",
            "document.proto",
        ).unwrap().expect("protobuf should be dispatched");
        assert_eq!(record.proto_services[0].name, "DocumentService");
        assert_eq!(
            record.proto_services[0].operations,
            vec!["GetDocument", "DeleteDocument"]
        );
    }
}
