use std::collections::HashMap;

use serde::Deserialize;

/// Body of `POST /v1/systemone`. See https://docs.typesafe.ai/api.md
#[derive(Debug, Clone, Deserialize)]
pub struct SystemOneResponse {
    /// Concrete model that answered, e.g. `jev-1.13.0`.
    pub model: String,
    /// Keyed by the question ids sent in the request.
    pub answers: HashMap<String, Answer>,
    pub usage: Usage,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    /// Probability the statement is true, 0 (no) -> 1 (yes).
    Noul { noul: f64 },
    Choice {
        /// Highest-probability option.
        choice: String,
        /// Every option -> probability; sums to 1.
        probabilities: HashMap<String, f64>,
        confidence: f64,
    },
    Score {
        /// Probability-weighted level; may land between levels.
        score: f64,
        /// Level index ("0", "1", ...) -> level description.
        legend: HashMap<String, String>,
        /// Level index -> probability; sums to 1.
        probabilities: HashMap<String, f64>,
        confidence: f64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_documented_example() {
        let body = r#"{
            "model": "jev-1.13.0",
            "answers": {
                "department": { "type": "choice", "choice": "technical", "confidence": 0.78,
                    "probabilities": { "technical": 0.85, "sales": 0.0, "billing": 0.15 } },
                "frustration": { "type": "score", "score": 1.0, "confidence": 1.0,
                    "legend": { "0": "Calm", "1": "Frustrated", "2": "Very angry" },
                    "probabilities": { "0": 0.0, "1": 1.0, "2": 0.0 } },
                "is_urgent": { "type": "noul", "noul": 1.0 }
            },
            "usage": { "input_tokens": 392, "output_tokens": 65 }
        }"#;
        let resp: SystemOneResponse = serde_json::from_str(body).expect("valid example");
        assert_eq!(resp.model, "jev-1.13.0");
        assert!(matches!(resp.answers["is_urgent"], Answer::Noul { noul } if noul == 1.0));
        assert!(
            matches!(&resp.answers["department"], Answer::Choice { choice, .. } if choice == "technical")
        );
        assert!(matches!(resp.answers["frustration"], Answer::Score { score, .. } if score == 1.0));
    }
}
