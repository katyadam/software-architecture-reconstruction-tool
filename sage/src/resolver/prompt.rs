use models::assignments::{Scope, VariableAddress};

use crate::resolver::{facts::FactBundle, query::QueryKind};

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
- "confidence" must reflect how certain you are (1.0 = certain, 0.5 = it is possible, 0 = you have not found anything).
- "evidence" must list the specific symbols, constants, or lines that support your answer.
- Do not add any text outside the JSON object.
- Do not use markdown in the response."#
        .to_string()
}

pub fn build_facts_message(bundle: &FactBundle) -> String {
    format!("SITES:\n`{}`", fmt_sites(bundle))
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

/// Render the variables block of the prompt.
///
/// `entries` is iterated in the order given by the caller; the function does
/// not re-sort or otherwise reorder the input. This is what makes prompts
/// deterministic across runs.
pub fn build_variables_message(entries: &[(VariableAddress, String)]) -> String {
    const HINT: &str = "Note: the snippet may reference a field (e.g. `self._mds_url`) whose value is set by a constructor argument defined in another file or microservice. Prefer variables whose name resembles the referenced field, even if defined elsewhere.";

    if entries.is_empty() {
        return format!("Known VARIABLES and their VALUES:\n  none\n{HINT}");
    }
    let lines = entries
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
    format!("VARIABLES:\n{lines}\n{HINT}")
}

fn fmt_scope(scope: &Scope) -> String {
    match scope {
        Scope::Global => "Global".to_string(),
        Scope::Class(name) => format!("Class:{name}"),
        Scope::Function(name) => format!("Function:{name}"),
    }
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
    use crate::resolver::{code::CodeSnippet, facts::FactBundle, query::QueryKind};
    use models::assignments::AssignmentKey;
    use models::ir::language::Language;

    fn full_bundle() -> FactBundle {
        FactBundle {
            sites: vec![CodeSnippet {
                code: "restTemplate.getForObject(BASE_URL, String.class)".to_string(),
                language: Language::Java,
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
    fn facts_full_bundle_includes_site_language_and_code() {
        let msg = build_facts_message(&full_bundle());
        assert!(msg.contains("Language: Java"));
        assert!(msg.contains("restTemplate.getForObject"));
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
    fn variables_message_empty_includes_constructor_hint() {
        let entries: Vec<(VariableAddress, String)> = Vec::new();
        let msg = build_variables_message(&entries);
        assert!(msg.contains("none"));
        assert!(msg.contains("self._mds_url"));
        assert!(msg.contains("constructor argument"));
    }

    #[test]
    fn variables_message_populated_includes_constructor_hint() {
        let entries = vec![(
            VariableAddress {
                microservice: "svc".to_string(),
                file: "f.py".to_string(),
                key: AssignmentKey {
                    scope: Scope::Global,
                    variable_name: "BASE_URL".to_string(),
                },
            },
            "http://example".to_string(),
        )];
        let msg = build_variables_message(&entries);
        assert!(msg.contains("BASE_URL"));
        assert!(msg.contains("self._mds_url"));
        assert!(msg.contains("constructor argument"));
    }

    #[test]
    fn variables_message_preserves_input_order() {
        let entries = vec![
            (
                VariableAddress {
                    microservice: "svc".to_string(),
                    file: "f.py".to_string(),
                    key: AssignmentKey {
                        scope: Scope::Global,
                        variable_name: "ZETA".to_string(),
                    },
                },
                "z".to_string(),
            ),
            (
                VariableAddress {
                    microservice: "svc".to_string(),
                    file: "f.py".to_string(),
                    key: AssignmentKey {
                        scope: Scope::Global,
                        variable_name: "ALPHA".to_string(),
                    },
                },
                "a".to_string(),
            ),
        ];
        let msg = build_variables_message(&entries);
        let zeta_pos = msg.find("ZETA").expect("zeta present");
        let alpha_pos = msg.find("ALPHA").expect("alpha present");
        assert!(
            zeta_pos < alpha_pos,
            "expected ZETA to appear before ALPHA, got: {msg}"
        );
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
