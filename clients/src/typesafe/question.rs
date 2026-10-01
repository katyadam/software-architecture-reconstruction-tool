use std::collections::BTreeMap;

use serde::Serialize;

/// One entry of the request `questions` map. See https://docs.typesafe.ai/api.md
///
/// The API also accepts objects/arrays for instructions and criteria; plain
/// strings cover our questions, so only those are modeled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    /// Yes/no; answer is probability of yes.
    Noul {
        instructions: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        criteria: Option<NoulCriteria>,
    },
    /// Pick one option. Option -> description (`None` -> `null`, no detail).
    /// At most 255 options.
    Choice {
        instructions: String,
        criteria: BTreeMap<String, Option<String>>,
    },
    /// Rate on ordered levels, lowest first. 2..=10 levels.
    Score {
        instructions: String,
        criteria: Vec<String>,
    },
}

/// What a yes / a no means.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NoulCriteria {
    #[serde(rename = "true")]
    pub yes: String,
    #[serde(rename = "false")]
    pub no: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_to_documented_shape() {
        let noul = Question::Noul {
            instructions: "Does this convey urgency?".to_string(),
            criteria: Some(NoulCriteria {
                yes: "Explicitly time-sensitive".to_string(),
                no: "No urgency expressed".to_string(),
            }),
        };
        assert_eq!(
            serde_json::to_value(&noul).expect("serializable"),
            json!({
                "type": "noul",
                "instructions": "Does this convey urgency?",
                "criteria": { "true": "Explicitly time-sensitive", "false": "No urgency expressed" }
            })
        );

        let choice = Question::Choice {
            instructions: "Which team?".to_string(),
            criteria: BTreeMap::from([
                ("billing".to_string(), Some("Payments".to_string())),
                ("other".to_string(), None),
            ]),
        };
        assert_eq!(
            serde_json::to_value(&choice).expect("serializable"),
            json!({
                "type": "choice",
                "instructions": "Which team?",
                "criteria": { "billing": "Payments", "other": null }
            })
        );

        let score = Question::Score {
            instructions: "How frustrated?".to_string(),
            criteria: vec!["Calm".to_string(), "Very angry".to_string()],
        };
        assert_eq!(
            serde_json::to_value(&score).expect("serializable"),
            json!({ "type": "score", "instructions": "How frustrated?", "criteria": ["Calm", "Very angry"] })
        );
    }
}
