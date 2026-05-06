use std::collections::HashMap;

use models::assignments::{Scope, VariableAddress};

use crate::resolver::{code::SymbolKind, facts::FactBundle, query::QueryKind};

pub fn build_system_message() -> String {
    r#"You are an architecture analysis assistant. Your task is to resolve
unresolved REST call-site expressions in source code.

You MUST respond with a single JSON object and nothing else:
{
  "resolved": "<string or null>",
  "confidence": <0.0-1.0>,
  "evidence": ["<citation1>", "..."],
  "reasoning": "<optional string>"
}

Rules:
- "resolved" is the concrete URL or value you determined, or null if unknown.
- "confidence" must reflect how certain you are (1.0 = certain).
- "evidence" must list the specific symbols, constants, or lines that support your answer.
- Do not add any text outside the JSON object."#
        .to_string()
}

pub fn build_facts_message(bundle: &FactBundle) -> String {
    format!(
        "FRAMEWORKS: {}\nSYMBOLS:\n{}CONSTANTS:\n{}\nOTHER:\n{}\nSITES:\n`{}`",
        fmt_frameworks(bundle),
        fmt_symbols(bundle),
        fmt_constants(bundle),
        fmt_others(bundle),
        fmt_sites(bundle),
    )
}

pub fn build_question_message(kind: &QueryKind) -> String {
    match kind {
        QueryKind::ResolveEnvVar { var_name } => format!(
            "What is the value of the environment variable `{var_name}`? \
Look at the symbols and constants provided. \
Return null if you cannot determine it with confidence >= 0.7."
        ),
        QueryKind::ResolveBuilder { chain } => format!(
            "What URL does the builder chain `{chain}` produce? \
Trace each method call in the chain using the provided symbols. \
Return null if you cannot determine it with confidence >= 0.7."
        ),
        QueryKind::ResolveLookup { lookup_key } => format!(
            "What value does the map or registry lookup for key `{lookup_key}` return? \
Use the provided constants and symbols. \
Return null if you cannot determine it with confidence >= 0.7."
        ),
        QueryKind::ResolveFrameworkRoute { route_pattern } => format!(
            "What is the full resolved URL for the route pattern `{route_pattern}`? \
Include any base path from class-level annotations visible in the symbols. \
Return null if you cannot determine it with confidence >= 0.7."
        ),
        QueryKind::ResolveReflective { target } => format!(
            "What concrete class or URL does the reflective reference `{target}` resolve to? \
Use the provided symbols and constants. \
Return null if you cannot determine it with confidence >= 0.7."
        ),
        QueryKind::ClassifyHttpCall { call_expr } => format!(
            "What HTTP method and URL does the call expression `{call_expr}` represent? \
Return the URL as `resolved` and include the HTTP method in `reasoning`. \
Return null if you cannot determine it with confidence >= 0.7."
        ),
    }
}

pub fn build_variables_message(map: &HashMap<VariableAddress, String>) -> String {
    if map.is_empty() {
        return "Known VARIABLES and their VALUES:\n  none".to_string();
    }
    let lines = map
        .iter()
        .map(|(addr, value)| {
            format!(
                "  [{}|{}|{}|{}] = {}",
                addr.microservice,
                addr.file,
                fmt_scope(&addr.key.scope),
                addr.key.variable_name,
                value
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("VARIABLES:\n{lines}")
}

fn fmt_scope(scope: &Scope) -> String {
    match scope {
        Scope::Global => "Global".to_string(),
        Scope::Class(name) => format!("Class:{name}"),
        Scope::Function(name) => format!("Function:{name}"),
    }
}

fn fmt_frameworks(bundle: &FactBundle) -> String {
    if bundle.frameworks.is_empty() {
        return "none".to_string();
    }
    bundle
        .frameworks
        .iter()
        .map(|f| f.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn fmt_symbols(bundle: &FactBundle) -> String {
    let lines: String = bundle
        .local_scope
        .iter()
        .chain(bundle.imported_scope.iter())
        .chain(bundle.class_or_module_attrs.iter())
        .map(|sym| {
            let value = sym.value.as_deref().unwrap_or("?");
            let datatype = sym.datatype.as_deref().unwrap_or("?");
            let kind = match &sym.kind {
                SymbolKind::Named => "local".to_string(),
                SymbolKind::Imported { target_file } => format!("imported from {target_file}"),
                SymbolKind::Attr { class } => format!("attr of {class}"),
            };
            format!("  {} ({}) -> {} : {}\n", sym.name, kind, value, datatype)
        })
        .collect();
    if lines.is_empty() {
        "  none\n".to_string()
    } else {
        lines
    }
}

fn fmt_constants(bundle: &FactBundle) -> String {
    if bundle.constants.is_empty() {
        return "none".to_string();
    }
    bundle
        .constants
        .iter()
        .map(|c| format!("  {} = {} (from {})", c.name, c.value, c.source_file))
        .collect::<Vec<_>>()
        .join("\n")
}

fn fmt_others(bundle: &FactBundle) -> String {
    if bundle.others.is_empty() {
        return "none".to_string();
    }
    bundle
        .others
        .iter()
        .map(|m| format!("  {}", m.text))
        .collect::<Vec<_>>()
        .join("\n")
}

fn fmt_sites(bundle: &FactBundle) -> String {
    bundle
        .sites
        .iter()
        .map(|site| format!("Language: {}\nCode Snippet: {}", site.language, site.code))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::{
        code::{CodeSnippet, Symbol, SymbolKind},
        facts::FactBundle,
        messages::Message,
        query::QueryKind,
    };
    use models::ir::{
        language::{Framework, Language},
        project::ConstantValue,
    };

    fn empty_bundle() -> FactBundle {
        FactBundle {
            sites: vec![],
            frameworks: vec![],
            local_scope: vec![],
            imported_scope: vec![],
            class_or_module_attrs: vec![],
            constants: vec![],
            others: vec![],
        }
    }

    fn full_bundle() -> FactBundle {
        FactBundle {
            sites: vec![CodeSnippet {
                code: "restTemplate.getForObject(BASE_URL, String.class)".to_string(),
                language: Language::Java,
            }],
            frameworks: vec![Framework::Spring],
            local_scope: vec![Symbol {
                name: "restTemplate".to_string(),
                value: None,
                datatype: Some("RestTemplate".to_string()),
                kind: SymbolKind::Named,
            }],
            imported_scope: vec![Symbol {
                name: "UserClient".to_string(),
                value: None,
                datatype: None,
                kind: SymbolKind::Imported {
                    target_file: "com/example/UserClient.java".to_string(),
                },
            }],
            class_or_module_attrs: vec![Symbol {
                name: "BASE_URL".to_string(),
                value: None,
                datatype: Some("String".to_string()),
                kind: SymbolKind::Attr {
                    class: "UserServiceClient".to_string(),
                },
            }],
            constants: vec![ConstantValue {
                name: "BASE_URL".to_string(),
                value: "http://user-service:8080".to_string(),
                source_file: "application.properties".to_string(),
            }],
            others: vec![Message {
                text: "injected via @Value".to_string(),
            }],
        }
    }

    #[test]
    fn system_message_contains_json_contract() {
        let msg = build_system_message();
        assert!(msg.contains("\"resolved\""));
        assert!(msg.contains("\"confidence\""));
        assert!(msg.contains("\"evidence\""));
        assert!(msg.contains("\"reasoning\""));
    }

    #[test]
    fn facts_empty_bundle_shows_none_everywhere() {
        let msg = build_facts_message(&empty_bundle());
        assert!(msg.contains("FRAMEWORKS: none"));
        assert!(msg.contains("CONSTANTS:\nnone"));
        assert!(msg.contains("OTHER:\nnone"));
        assert!(msg.contains("  none\n"));
    }

    #[test]
    fn facts_full_bundle_includes_framework() {
        let msg = build_facts_message(&full_bundle());
        assert!(msg.contains("Spring"));
    }

    #[test]
    fn facts_full_bundle_includes_all_symbol_kinds() {
        let msg = build_facts_message(&full_bundle());
        assert!(msg.contains("restTemplate (local)"));
        assert!(msg.contains("imported from com/example/UserClient.java"));
        assert!(msg.contains("attr of UserServiceClient"));
    }

    #[test]
    fn facts_full_bundle_includes_constant_with_source() {
        let msg = build_facts_message(&full_bundle());
        assert!(msg.contains("BASE_URL = http://user-service:8080"));
        assert!(msg.contains("application.properties"));
    }

    #[test]
    fn facts_full_bundle_includes_other_messages() {
        let msg = build_facts_message(&full_bundle());
        assert!(msg.contains("injected via @Value"));
    }

    #[test]
    fn facts_full_bundle_includes_site_language_and_code() {
        let msg = build_facts_message(&full_bundle());
        assert!(msg.contains("Language: Java"));
        assert!(msg.contains("restTemplate.getForObject"));
    }

    #[test]
    fn facts_multiple_frameworks_are_comma_separated() {
        let mut bundle = empty_bundle();
        bundle.frameworks = vec![Framework::Spring, Framework::FastAPI];
        let msg = build_facts_message(&bundle);
        assert!(msg.contains("Spring"));
        assert!(msg.contains("FastAPI"));
        assert!(msg.contains(", "));
    }

    #[test]
    fn question_resolve_env_var_embeds_name() {
        let msg = build_question_message(&QueryKind::ResolveEnvVar {
            var_name: "MY_VAR".to_string(),
        });
        assert!(msg.contains("`MY_VAR`"));
    }

    #[test]
    fn question_resolve_builder_embeds_chain() {
        let msg = build_question_message(&QueryKind::ResolveBuilder {
            chain: "builder.host(x).port(y)".to_string(),
        });
        assert!(msg.contains("`builder.host(x).port(y)`"));
    }

    #[test]
    fn question_resolve_lookup_embeds_key() {
        let msg = build_question_message(&QueryKind::ResolveLookup {
            lookup_key: "SERVICE_KEY".to_string(),
        });
        assert!(msg.contains("`SERVICE_KEY`"));
    }

    #[test]
    fn question_resolve_framework_route_embeds_pattern() {
        let msg = build_question_message(&QueryKind::ResolveFrameworkRoute {
            route_pattern: "/api/v1/{id}".to_string(),
        });
        assert!(msg.contains("`/api/v1/{id}`"));
    }

    #[test]
    fn question_resolve_reflective_embeds_target() {
        let msg = build_question_message(&QueryKind::ResolveReflective {
            target: "com.example.Service".to_string(),
        });
        assert!(msg.contains("`com.example.Service`"));
    }

    #[test]
    fn question_classify_http_call_embeds_expr() {
        let msg = build_question_message(&QueryKind::ClassifyHttpCall {
            call_expr: "client.post(\"/users\")".to_string(),
        });
        assert!(msg.contains("`client.post(\"/users\")`"));
    }

    #[test]
    fn all_question_kinds_include_confidence_threshold() {
        let kinds = [
            QueryKind::ResolveEnvVar {
                var_name: "X".to_string(),
            },
            QueryKind::ResolveBuilder {
                chain: "X".to_string(),
            },
            QueryKind::ResolveLookup {
                lookup_key: "X".to_string(),
            },
            QueryKind::ResolveFrameworkRoute {
                route_pattern: "X".to_string(),
            },
            QueryKind::ResolveReflective {
                target: "X".to_string(),
            },
            QueryKind::ClassifyHttpCall {
                call_expr: "X".to_string(),
            },
        ];
        for kind in &kinds {
            let msg = build_question_message(kind);
            assert!(
                msg.contains("0.7"),
                "missing confidence threshold in: {msg}"
            );
        }
    }
}
