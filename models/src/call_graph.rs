use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// Identifies the implementation language handled by a call-graph provider.
pub enum Language {
    Java,
    Go,
    Python,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Describes one source subtree to analyze with its implementation language.
pub struct CallGraphRequest {
    pub source_root: String,
    pub language: Language,
}
impl CallGraphRequest {
    /// Creates a request for analyzing `source_root` with the provider for `language`.
    pub fn new(source_root: impl Into<String>, language: Language) -> Self {
        Self {
            source_root: source_root.into(),
            language,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Reports whether a provider successfully produced a call graph.
pub enum CallGraphStatus {
    Ok,
    NoEntrypoints,
    Unsupported,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Provides the provider-independent identity of a method participating in an edge.
pub struct MethodRef {
    pub language: Language,
    pub declaring_type: String,
    pub member_name: String,
    pub descriptor: String,
    pub source_path: Option<String>,
    pub source_line: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Represents an unresolved edge reported by a language-specific call-graph provider.
pub struct RawCallEdge {
    pub caller: MethodRef,
    pub callee: MethodRef,
    pub provider_id: String,
    pub algorithm: String,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
/// Represents an edge whose endpoints have been matched to VoyantClair callable IDs.
pub struct ResolvedCallEdge {
    pub source_id: String,
    pub target_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Carries the result, metadata, and diagnostics of a call-graph-provider invocation.
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
    /// Creates an initially empty successful outcome that a provider can populate with edges.
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
    /// Creates an outcome explaining that no registered provider supports the requested language.
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
