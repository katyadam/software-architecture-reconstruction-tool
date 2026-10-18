//! Opt-in linked-binary package evidence; does not change protocol-1 discovery.
use crate::go_impact::{GoAnalysisScope, GoImpactReport, GoStageReport, GoStageStatus};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GoSelectionPolicy {
    Conservative,
    Packages,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoPackageTestScope {
    #[serde(rename = "linked-binary")]
    LinkedBinary,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoEffectiveBuildContext {
    pub go_version: String,
    pub goos: String,
    pub goarch: String,
    pub cgo_enabled: String,
    pub architecture_features: BTreeMap<String, String>,
    pub goexperiment: String,
    pub build_tags: String,
    pub environment: BTreeMap<String, String>,
    pub build_flags: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoPackageProvenanceKind {
    VerifiedModule,
    ModuleCache,
    StandardLibrary,
    GeneratedTestHarness,
    LocalReplacement,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoPackageProvenance {
    pub kind: GoPackageProvenanceKind,
    pub directory: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_version: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoPackageNode {
    pub id: String,
    pub import_path: String,
    pub imports: Vec<String>,
    pub provenance: GoPackageProvenance,
    pub source_files: Vec<String>,
    pub test_files: Vec<String>,
    pub for_test: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoExecutionPackage {
    pub import_path: String,
    pub relative_directory: String,
    pub roots: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoPackageBlocker {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    pub diagnostic: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoPackageInventoryOutcome {
    pub schema_version: u32,
    pub provider_id: String,
    pub status: GoStageStatus,
    pub source_root: String,
    pub scope: Option<GoAnalysisScope>,
    pub effective_build_context: Option<GoEffectiveBuildContext>,
    pub complete: bool,
    pub packages: Vec<GoPackageNode>,
    pub execution_packages: Vec<GoExecutionPackage>,
    pub blockers: Vec<GoPackageBlocker>,
    pub diagnostics: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoExecutionPackageDecision {
    pub package: GoExecutionPackage,
    pub argument: String,
    pub changed_owners: Vec<String>,
    pub dependency_intersection: Vec<String>,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoPackageImpactReport {
    #[serde(flatten)]
    pub base: GoImpactReport,
    pub requested_policy: GoSelectionPolicy,
    pub declared_test_scope: GoPackageTestScope,
    pub effective_build_context: Option<GoEffectiveBuildContext>,
    pub package_evidence: GoStageReport,
    pub execution_inventory_complete: bool,
    pub discovered_execution_packages: Vec<GoExecutionPackage>,
    pub selected_execution_packages: Vec<GoExecutionPackageDecision>,
    pub unselected_execution_packages: Vec<GoExecutionPackageDecision>,
}
