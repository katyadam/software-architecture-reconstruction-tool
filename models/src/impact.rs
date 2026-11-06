use serde::{Deserialize, Serialize};

/// Classifies the granularity at which a Git change can be mapped safely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangedElementKind {
    /// A changed range belongs to one source callable.
    Callable,
    /// A changed range belongs to a class but not to one callable.
    Class,
    /// A change requires selecting every test in its Maven module.
    Module,
}

/// Identifies a changed Java source element and its conservative Maven-module scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedElement {
    pub module_root: String,
    pub source_path: String,
    pub kind: ChangedElementKind,
    pub callable_signature: Option<String>,
}

/// Identifies one discovered Java test method using its JVM-level method identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaTestCase {
    pub module_root: String,
    pub class_name: String,
    pub method_name: String,
    pub descriptor: String,
    pub callable_signature: String,
    pub source_path: String,
}

/// Explains why conservative impact analysis selected a test.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionReason {
    /// A selected test reaches a changed callable through the local call graph.
    ChangedCallable { signature: String },
    /// A selected test reaches a callable in a changed class.
    ChangedClass { class_name: String },
    /// Analysis uncertainty selected every test in the affected Maven module.
    ModuleFallback { diagnostic: String },
    /// A failed external analysis selected every test in the affected Maven module.
    AnalysisFailure { diagnostic: String },
}

/// Couples one selected test with every independent reason that selected it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedTest {
    pub test: JavaTestCase,
    pub reasons: Vec<SelectionReason>,
}

/// Carries the complete, auditable result of one conservative test-impact analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestImpactResult {
    pub baseline_revision: String,
    pub candidate_revision: String,
    pub changed_elements: Vec<ChangedElement>,
    pub selected_tests: Vec<SelectedTest>,
    pub unselected_tests: Vec<JavaTestCase>,
    pub diagnostics: Vec<String>,
}

impl TestImpactResult {
    /// Creates an impact result with explicit revisions, selections, and diagnostics.
    pub fn new(
        baseline_revision: String,
        candidate_revision: String,
        changed_elements: Vec<ChangedElement>,
        selected_tests: Vec<SelectedTest>,
        unselected_tests: Vec<JavaTestCase>,
    ) -> Self {
        Self {
            baseline_revision,
            candidate_revision,
            changed_elements,
            selected_tests,
            unselected_tests,
            diagnostics: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ChangedElement, ChangedElementKind, JavaTestCase, SelectedTest, SelectionReason,
        TestImpactResult,
    };

    #[test]
    fn serializes_a_fallback_selected_test_without_losing_its_descriptor() {
        let result = TestImpactResult::new(
            "baseline".into(),
            "candidate".into(),
            vec![ChangedElement {
                module_root: "orders".into(),
                source_path: "orders/pom.xml".into(),
                kind: ChangedElementKind::Module,
                callable_signature: None,
            }],
            vec![SelectedTest {
                test: JavaTestCase {
                    module_root: "orders".into(),
                    class_name: "Lorders/OrderServiceTest".into(),
                    method_name: "createsOrder".into(),
                    descriptor: "()V".into(),
                    callable_signature: "OrderServiceTest.createsOrder()".into(),
                    source_path: "orders/src/test/java/orders/OrderServiceTest.java".into(),
                },
                reasons: vec![SelectionReason::ModuleFallback {
                    diagnostic: "Changed Maven POM".into(),
                }],
            }],
            vec![],
        );

        let decoded: TestImpactResult =
            serde_json::from_str(&serde_json::to_string(&result).expect("result serializes"))
                .expect("result deserializes");

        assert_eq!(decoded, result);
        assert_eq!(decoded.selected_tests[0].test.descriptor, "()V");
        assert!(matches!(
            decoded.selected_tests[0].reasons.as_slice(),
            [SelectionReason::ModuleFallback { .. }]
        ));
    }
}
