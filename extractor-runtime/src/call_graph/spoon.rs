use super::{java_adapter::JavaAdapterProvider, registry::CallGraphProvider};
use models::{
    JavaTestCase,
    call_graph::{CallGraphOutcome, CallGraphRequest, Language},
};
use std::{path::PathBuf, time::Duration};

/// Source-level call graph provider backed by the Spoon AST adapter.
///
/// Unlike the WALA provider this facade never invokes `ArtifactPreparationMain`:
/// Spoon works directly from `.java` source files without requiring compiled classes.
pub struct SpoonJavaProvider {
    inner: JavaAdapterProvider,
}

impl SpoonJavaProvider {
    /// Creates a Spoon provider with a caller-specified analysis deadline.
    pub fn with_timeout(jar: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self {
            inner: JavaAdapterProvider::new("spoon-java", jar, timeout, "cha_source")
                .without_artifact_preparation(),
        }
    }

    /// Analyzes discovered JUnit methods and changed callable targets through Spoon.
    pub fn analyze_test_roots(
        &self,
        source_root: &std::path::Path,
        test_roots: &[JavaTestCase],
        targets: &[String],
    ) -> CallGraphOutcome {
        self.inner
            .analyze_test_roots(source_root, test_roots, targets)
    }
}

impl CallGraphProvider for SpoonJavaProvider {
    fn language(&self) -> Language {
        self.inner.language()
    }

    fn analyze(&self, request: &CallGraphRequest) -> CallGraphOutcome {
        self.inner.analyze(request)
    }
}
