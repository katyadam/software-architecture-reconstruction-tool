use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Java,
    Go,
    Python,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallGraphRequest {
    pub source_root: String,
    pub language: Language,
}
impl CallGraphRequest {
    pub fn new(source_root: impl Into<String>, language: Language) -> Self {
        Self {
            source_root: source_root.into(),
            language,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallGraphStatus {
    Ok,
    NoEntrypoints,
    Unsupported,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodRef {
    pub language: Language,
    pub declaring_type: String,
    pub member_name: String,
    pub descriptor: String,
    pub source_path: Option<String>,
    pub source_line: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawCallEdge {
    pub caller: MethodRef,
    pub callee: MethodRef,
    pub provider_id: String,
    pub algorithm: String,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedCallEdge {
    pub source_id: String,
    pub target_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallGraphOutcome {
    pub schema_version: u32,
    pub status: CallGraphStatus,
    pub provider_id: String,
    pub source_root: String,
    pub algorithm: String,
    pub diagnostics: Vec<String>,
    pub edges: Vec<RawCallEdge>,
}
impl CallGraphOutcome {
    pub fn ok(
        request: CallGraphRequest,
        provider_id: impl Into<String>,
        algorithm: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: 1,
            status: CallGraphStatus::Ok,
            provider_id: provider_id.into(),
            source_root: request.source_root,
            algorithm: algorithm.into(),
            diagnostics: vec![],
            edges: vec![],
        }
    }
    pub fn unsupported(request: &CallGraphRequest) -> Self {
        Self {
            schema_version: 1,
            status: CallGraphStatus::Unsupported,
            provider_id: "".into(),
            source_root: request.source_root.clone(),
            algorithm: "".into(),
            diagnostics: vec![format!("No call-graph provider for {:?}", request.language)],
            edges: vec![],
        }
    }
}
