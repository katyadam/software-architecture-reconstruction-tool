use crate::resolver::query::{CandidateService, ClassifyContext};

/// The closed-set system contract (plan §6.1). The model chooses exactly one
/// service from the candidate set, or null; it never generates a URL or value.
pub fn build_system_message() -> String {
    r#"You are a software-architecture analysis assistant. A REST client in one
microservice calls another microservice over HTTP. Given a call site whose
target URL could NOT be resolved statically, identify which microservice it
targets.

Choose exactly one service from CANDIDATE SERVICES, or null if none clearly
matches. Respond with a single JSON object and nothing else:

{
  "service": "<exact name from CANDIDATE SERVICES, or null>",
  "evidence": ["<token from CONTEXT that justifies the choice>", "..."],
  "reasoning": "<one short sentence>"
}

Rules:
- "service" MUST be copied verbatim from CANDIDATE SERVICES, or be null.
- Do NOT invent a service. Do NOT output a URL or a value.
- "evidence" must cite concrete tokens present in CONTEXT (class, variable,
  import). An answer with no grounding token is invalid.
- No text outside the JSON. No markdown."#
        .to_string()
}

/// Render the task + context message (plan §6.2): the origin service, a
/// precedence-ordered CALL SITE block (client class strongest, identifiers
/// weakest), and the CANDIDATE SERVICES list.
pub fn build_context_message(ctx: &ClassifyContext, candidates: &[CandidateService]) -> String {
    let client_class = ctx.client_class.as_deref().unwrap_or("(unknown)");
    let imports = if ctx.imports.is_empty() {
        "(none)".to_string()
    } else {
        ctx.imports.join(", ")
    };
    let identifiers = if ctx.operand_identifiers.is_empty() {
        "(none)".to_string()
    } else {
        ctx.operand_identifiers.join(", ")
    };
    let candidate_lines = candidates
        .iter()
        .map(|c| format!("  - {}    ({})", c.name, c.url))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "Which microservice does this call target?\n\
\n\
ORIGIN SERVICE: {origin}        (exclude as the answer; this is the caller)\n\
\n\
CALL SITE\n\
  client class : {client_class}      <- strongest signal\n\
  imports      : {imports}\n\
  expression   : {expression}\n\
  identifiers  : {identifiers}            <- weakest signal\n\
\n\
CANDIDATE SERVICES\n\
{candidate_lines}",
        origin = ctx.origin_service,
        expression = ctx.expression,
    )
}

/// Concatenate the grounding-relevant context tokens (client class, imports,
/// expression, identifiers) into one string. Used by `validate` to confirm the
/// model's evidence tokens actually appear in the call-site context.
pub fn context_grounding_string(ctx: &ClassifyContext) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(class) = &ctx.client_class {
        parts.push(class.clone());
    }
    parts.extend(ctx.imports.iter().cloned());
    parts.push(ctx.expression.clone());
    parts.extend(ctx.operand_identifiers.iter().cloned());
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ClassifyContext {
        ClassifyContext {
            origin_service: "app-service".to_string(),
            client_class: Some("MedicalDataServiceClient".to_string()),
            imports: vec!["custom_clients.mds_client.MedicalDataServiceClient".to_string()],
            expression: "self._mds_url + url".to_string(),
            operand_identifiers: vec!["self._mds_url".to_string(), "url".to_string()],
        }
    }

    fn candidates() -> Vec<CandidateService> {
        vec![CandidateService {
            name: "medical-data-service".to_string(),
            url: "http://medical-data-service:8000".to_string(),
        }]
    }

    #[test]
    fn system_message_mentions_service_and_evidence() {
        let msg = build_system_message();
        assert!(msg.contains("\"service\""));
        assert!(msg.contains("\"evidence\""));
    }

    #[test]
    fn context_message_includes_origin_candidate_class_and_expression() {
        let msg = build_context_message(&ctx(), &candidates());
        assert!(msg.contains("app-service"));
        assert!(msg.contains("medical-data-service"));
        assert!(msg.contains("http://medical-data-service:8000"));
        assert!(msg.contains("MedicalDataServiceClient"));
        assert!(msg.contains("self._mds_url + url"));
    }

    #[test]
    fn grounding_string_concatenates_signals() {
        let g = context_grounding_string(&ctx());
        assert!(g.contains("MedicalDataServiceClient"));
        assert!(g.contains("self._mds_url + url"));
        assert!(g.contains("url"));
    }
}
