use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Sage's answer to a single resolution question.
pub struct SageResponse {
    pub resolved: Option<String>,
    /// Normalised confidence score in [0.0, 1.0].
    pub confidence: f32,
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

    #[error("confidence {confidence} is below threshold")]
    LowConfidence { confidence: f32 },

    #[error("Sage returned no supporting evidence")]
    MissingEvidence,
}

/// Wire shape of the JSON object the LLM must return.
#[derive(Deserialize, Serialize)]
pub(crate) struct LlmJson {
    pub resolved: Option<String>,
    pub confidence: f32,
    pub evidence: Vec<String>,
    pub reasoning: Option<String>,
}

pub(crate) fn validate(json: LlmJson, threshold: f32) -> Result<SageResponse, SageError> {
    if json.confidence < threshold {
        return Err(SageError::LowConfidence {
            confidence: json.confidence,
        });
    }
    if json.evidence.is_empty() {
        return Err(SageError::MissingEvidence);
    }
    Ok(SageResponse {
        resolved: json.resolved,
        confidence: json.confidence,
        evidence: json.evidence,
        reasoning: json.reasoning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_json() -> LlmJson {
        LlmJson {
            resolved: Some("http://user-service:8080".to_string()),
            confidence: 0.9,
            evidence: vec!["BASE_URL constant".to_string()],
            reasoning: Some("direct constant match".to_string()),
        }
    }

    #[test]
    fn validate_accepts_sufficient_confidence() {
        let resp = validate(valid_json(), 0.7).unwrap();
        assert_eq!(resp.confidence, 0.9);
        assert_eq!(resp.resolved.unwrap(), "http://user-service:8080");
    }

    #[test]
    fn validate_rejects_confidence_below_threshold() {
        let mut json = valid_json();
        json.confidence = 0.5;
        assert!(matches!(
            validate(json, 0.7),
            Err(SageError::LowConfidence { confidence }) if (confidence - 0.5).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn validate_rejects_empty_evidence() {
        let mut json = valid_json();
        json.evidence = vec![];
        assert!(matches!(
            validate(json, 0.7),
            Err(SageError::MissingEvidence)
        ));
    }

    #[test]
    fn validate_allows_null_resolved() {
        let mut json = valid_json();
        json.resolved = None;
        assert!(validate(json, 0.7).unwrap().resolved.is_none());
    }

    #[test]
    fn validate_exact_threshold_is_accepted() {
        let mut json = valid_json();
        json.confidence = 0.7;
        assert!(validate(json, 0.7).is_ok());
    }

    #[test]
    fn llm_json_deserializes_null_fields() {
        let raw = r#"{"resolved":null,"confidence":0.8,"evidence":["x"],"reasoning":null}"#;
        let parsed: LlmJson = serde_json::from_str(raw).unwrap();
        assert!(parsed.resolved.is_none());
        assert!(parsed.reasoning.is_none());
        assert_eq!(parsed.confidence, 0.8);
    }

    #[test]
    fn llm_json_round_trip() {
        let json = valid_json();
        let serialized = serde_json::to_string(&json).unwrap();
        let deserialized: LlmJson = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.resolved, json.resolved);
        assert_eq!(deserialized.confidence, json.confidence);
        assert_eq!(deserialized.evidence, json.evidence);
    }

    #[test]
    fn error_display_low_confidence_includes_value() {
        let err = SageError::LowConfidence { confidence: 0.42 };
        assert!(err.to_string().contains("0.42"));
    }

    #[test]
    fn error_display_missing_evidence_mentions_evidence() {
        let err = SageError::MissingEvidence;
        assert!(err.to_string().to_lowercase().contains("evidence"));
    }
}
