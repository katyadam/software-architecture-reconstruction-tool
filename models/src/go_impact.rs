//! Native Go metadata and conservative impact reports; independent of Java TIA.
use crate::call_graph::{CallGraphOutcome, Language, MethodRef};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoAnalysisScope {
    pub dependency_bodies: String,
    pub goos: String,
    pub goarch: String,
    pub go_version: String,
    pub build_tags: String,
    pub workspace_mode: String,
    pub limitations: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoStageStatus {
    Skipped,
    Ok,
    Failed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoTestCase {
    pub method: MethodRef,
    pub execution_package: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoSourceDeclaration {
    pub member_name: String,
    pub header: String,
    pub start_line: u32,
    pub end_line: u32,
    pub body_start_line: u32,
    pub body_end_line: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoNativeDeclaration {
    pub method: MethodRef,
    pub syntax: GoSourceDeclaration,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoInventoryOutcome {
    pub schema_version: u32,
    pub provider_id: String,
    pub status: GoStageStatus,
    pub source_root: String,
    pub source_files: Vec<String>,
    pub tests: Vec<GoTestCase>,
    pub declarations: Vec<GoNativeDeclaration>,
    pub test_inventory_complete: bool,
    pub scope: Option<GoAnalysisScope>,
    pub diagnostics: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoSourceInput {
    pub source_path: String,
    pub source: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoFileDescription {
    pub source_path: String,
    pub package_name: Option<String>,
    pub declarations: Vec<GoSourceDeclaration>,
    pub diagnostics: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoDescriptionOutcome {
    pub schema_version: u32,
    pub provider_id: String,
    pub status: GoStageStatus,
    pub files: Vec<GoFileDescription>,
    pub diagnostics: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoStageReport {
    pub status: GoStageStatus,
    pub diagnostics: Vec<String>,
    pub duration_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoChangeKind {
    Callable,
    Module,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoChangedElement {
    pub source_path: String,
    pub old_path: Option<String>,
    pub kind: GoChangeKind,
    pub target: Option<String>,
    pub diagnostics: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoSelectionMode {
    NoChanges,
    ConservativeAll,
    ConservativePackages,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoSelectionReason {
    ChangedCallable {
        target: String,
    },
    ConservativeFallback {
        diagnostic: String,
    },
    PackageDependency {
        execution_package: String,
        changed_owners: Vec<String>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoSuggestedTest {
    pub test: GoTestCase,
    pub changed_targets: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoSelectedTest {
    pub test: GoTestCase,
    pub reasons: Vec<GoSelectionReason>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoImpactReport {
    pub schema_version: u32,
    pub language: Language,
    pub module_root: String,
    pub baseline_revision: String,
    pub candidate_revision: String,
    pub changed_elements: Vec<GoChangedElement>,
    pub discovered_tests: Vec<GoTestCase>,
    pub test_inventory_complete: bool,
    pub graph_suggested_tests: Vec<GoSuggestedTest>,
    pub selected_tests: Vec<GoSelectedTest>,
    pub unselected_tests: Vec<GoTestCase>,
    pub selection_mode: GoSelectionMode,
    pub run_all_tests: bool,
    pub discovery: GoStageReport,
    pub mapping: GoStageReport,
    pub analysis: GoStageReport,
    pub graph: Option<CallGraphOutcome>,
    pub scope: Option<GoAnalysisScope>,
    pub diagnostics: Vec<String>,
}
