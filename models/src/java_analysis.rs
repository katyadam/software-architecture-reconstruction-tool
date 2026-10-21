//! Provider-neutral, build-produced Java analysis inputs.
use crate::JavaTestCase;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaBuildContext {
    pub tool: String,
    pub requested_profiles: Vec<String>,
    pub active_profiles: Vec<String>,
    pub java_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaArtifactManifest {
    pub schema_version: u32,
    pub candidate_revision: String,
    pub build_context: JavaBuildContext,
    pub module_inventory_complete: bool,
    pub modules: Vec<JavaModuleManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaModuleManifest {
    pub root: String,
    pub source_roots: Vec<String>,
    pub test_source_roots: Vec<String>,
    pub application_class_dirs: Vec<String>,
    pub test_class_dirs: Vec<String>,
    pub classpath: Vec<String>,
    pub dependencies: Vec<String>,
    pub dependency_inventory_complete: bool,
    pub test_inventory_complete: bool,
    pub tests: Vec<JavaTestCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaAnalysisContextRecord {
    pub service_name: String,
    pub service_root: String,
    pub manifest_path: String,
    pub manifest_digest: String,
    pub candidate_revision: String,
    pub build_context: JavaBuildContext,
    pub module_roots: Vec<String>,
    pub source_roots: Vec<String>,
    pub test_source_roots: Vec<String>,
}
