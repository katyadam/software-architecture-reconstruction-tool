use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::resolver::query::CandidateService;

/// Sage's answer to a single closed-set classification question.
pub struct SageResponse {
    /// The chosen service name (a member of the candidate set), or `None` to abstain.
    pub service: Option<String>,
    pub evidence: Vec<String>,
    pub reasoning: Option<String>,
}

/// Errors that can occur when querying Sage.
#[derive(Debug, Error)]
pub enum SageError {
    #[error("network failure: {0}")]
    Network(#[from] async_openai::error::OpenAIError),

    #[error("JSON parse failure: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("Sage returned no supporting evidence")]
    MissingEvidence,

    #[error("service {service:?} is not a candidate")]
    NotACandidate { service: String },
}

/// Wire shape of the JSON object the LLM must return.
#[derive(Deserialize, Serialize)]
pub(crate) struct LlmJson {
    pub service: Option<String>,
    pub evidence: Vec<String>,
    pub reasoning: Option<String>,
}

/// Validate an LLM answer against the closed-set contract.
///
/// A chosen `service` MUST be a candidate name and MUST be grounded: at least
/// one evidence string appears (case-insensitive) as a substring of `context`.
/// A `null` service is a valid abstain and needs no grounding.
pub(crate) fn validate(
    json: LlmJson,
    candidates: &[CandidateService],
    context: &str,
) -> Result<SageResponse, SageError> {
    if let Some(service) = &json.service {
        if !candidates.iter().any(|c| &c.name == service) {
            return Err(SageError::NotACandidate {
                service: service.clone(),
            });
        }
        if json.evidence.is_empty() {
            return Err(SageError::MissingEvidence);
        }
        let context_lc = context.to_lowercase();
        let grounded = json
            .evidence
            .iter()
            .any(|e| context_lc.contains(&e.to_lowercase()));
        if !grounded {
            return Err(SageError::MissingEvidence);
        }
    }
    Ok(SageResponse {
        service: json.service,
        evidence: json.evidence,
        reasoning: json.reasoning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates() -> Vec<CandidateService> {
        vec![
            CandidateService {
                name: "medical-data-service".to_string(),
                url: "http://medical-data-service:8000".to_string(),
            },
            CandidateService {
                name: "clinical-data-service".to_string(),
                url: "http://clinical-data-service:8000".to_string(),
            },
        ]
    }

    const CONTEXT: &str = "client class MedicalDataServiceClient expression self._mds_url + url";

    fn valid_json() -> LlmJson {
        LlmJson {
            service: Some("medical-data-service".to_string()),
            evidence: vec!["MedicalDataServiceClient".to_string()],
            reasoning: Some("class name matches the service".to_string()),
        }
    }

    #[test]
    fn validate_accepts_candidate_with_grounded_evidence() {
        let resp = validate(valid_json(), &candidates(), CONTEXT).unwrap();
        assert_eq!(resp.service.unwrap(), "medical-data-service");
    }

    #[test]
    fn validate_rejects_non_candidate() {
        let mut json = valid_json();
        json.service = Some("ghost-service".to_string());
        assert!(matches!(
            validate(json, &candidates(), CONTEXT),
            Err(SageError::NotACandidate { service }) if service == "ghost-service"
        ));
    }

    #[test]
    fn validate_rejects_empty_evidence_when_service_chosen() {
        let mut json = valid_json();
        json.evidence = vec![];
        assert!(matches!(
            validate(json, &candidates(), CONTEXT),
            Err(SageError::MissingEvidence)
        ));
    }

    #[test]
    fn validate_rejects_ungrounded_evidence_when_service_chosen() {
        let mut json = valid_json();
        json.evidence = vec!["totally-unrelated-token".to_string()];
        assert!(matches!(
            validate(json, &candidates(), CONTEXT),
            Err(SageError::MissingEvidence)
        ));
    }

    #[test]
    fn validate_accepts_null_service_without_grounding() {
        let json = LlmJson {
            service: None,
            evidence: vec![],
            reasoning: None,
        };
        assert!(
            validate(json, &candidates(), CONTEXT)
                .unwrap()
                .service
                .is_none()
        );
    }

    #[test]
    fn validate_grounding_is_case_insensitive() {
        let mut json = valid_json();
        json.evidence = vec!["medicaldataserviceclient".to_string()];
        assert!(validate(json, &candidates(), CONTEXT).is_ok());
    }

    #[test]
    fn llm_json_deserializes_null_fields() {
        let raw = r#"{"service":null,"evidence":["x"],"reasoning":null}"#;
        let parsed: LlmJson = serde_json::from_str(raw).unwrap();
        assert!(parsed.service.is_none());
        assert!(parsed.reasoning.is_none());
        assert_eq!(parsed.evidence, vec!["x".to_string()]);
    }

    #[test]
    fn llm_json_round_trip() {
        let json = valid_json();
        let serialized = serde_json::to_string(&json).unwrap();
        let deserialized: LlmJson = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.service, json.service);
        assert_eq!(deserialized.evidence, json.evidence);
    }

    #[test]
    fn error_display_not_a_candidate_includes_service() {
        let err = SageError::NotACandidate {
            service: "foo".to_string(),
        };
        assert!(err.to_string().contains("foo"));
    }

    #[test]
    fn error_display_missing_evidence_mentions_evidence() {
        let err = SageError::MissingEvidence;
        assert!(err.to_string().to_lowercase().contains("evidence"));
    }
}
