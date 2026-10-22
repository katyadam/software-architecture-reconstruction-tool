use crate::call_graph::Language;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PythonStageStatus {
    Skipped,
    Ok,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PythonStageReport {
    pub status: PythonStageStatus,
    pub diagnostics: Vec<String>,
    pub duration_ms: u64,
}

/// Identifies one discovered Python test function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PythonTestCase {
    /// Repository-relative path to the test file (e.g. `tests/test_service.py`).
    pub source_path: String,
    /// Dotted module path derived from the source path (e.g. `tests.test_service`).
    pub module_path: String,
    /// Qualified name of the test function or method (e.g. `TestClass.test_foo` or `test_foo`).
    pub qualified_name: String,
    /// Unqualified function name (e.g. `test_foo`).
    pub function_name: String,
    /// Optional class name when the test is a method on a `unittest.TestCase` subclass.
    pub class_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PythonChangeKind {
    Callable,
    Module,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PythonChangedElement {
    pub source_path: String,
    pub old_path: Option<String>,
    pub kind: PythonChangeKind,
    /// Qualified name of the changed callable (e.g. `MyClass.method` or `function`).
    pub target: Option<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PythonSelectionMode {
    NoChanges,
    ConservativeAll,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PythonSelectionReason {
    ChangedCallable { target: String },
    ConservativeFallback { diagnostic: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PythonSelectedTest {
    pub test: PythonTestCase,
    pub reasons: Vec<PythonSelectionReason>,
}

/// Advisory graph suggestion: pyan3 reached this test from a changed callable.
/// Does not imply the test can be excluded if it is absent — selection is always conservative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PythonSuggestedTest {
    pub test: PythonTestCase,
    /// Changed callable targets (dotted, no module prefix) that transitively reach this test.
    pub changed_targets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PythonImpactReport {
    pub schema_version: u32,
    pub language: Language,
    pub project_root: String,
    pub baseline_revision: String,
    pub candidate_revision: String,
    pub changed_elements: Vec<PythonChangedElement>,
    pub discovered_tests: Vec<PythonTestCase>,
    pub test_inventory_complete: bool,
    pub graph_suggested_tests: Vec<PythonSuggestedTest>,
    pub selected_tests: Vec<PythonSelectedTest>,
    pub unselected_tests: Vec<PythonTestCase>,
    pub selection_mode: PythonSelectionMode,
    pub run_all_tests: bool,
    pub discovery: PythonStageReport,
    pub mapping: PythonStageReport,
    pub analysis: PythonStageReport,
    pub diagnostics: Vec<String>,
}
