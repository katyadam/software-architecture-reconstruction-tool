use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConfigurationData {
    pub service_descriptions: Vec<ServiceDescription>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Default, Clone)]
pub struct ServiceDescription {
    pub name: String,
    pub base_dir_path: String,
    pub urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_analysis: Option<JavaAnalysisSettings>,
}

/// Stable build-input settings; resolved paths and dependencies live in a build manifest.
#[derive(Debug, Serialize, Deserialize, ToSchema, Default, Clone)]
pub struct JavaAnalysisSettings {
    #[serde(default)]
    pub maven_profiles: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifacts_file: Option<String>,
}
