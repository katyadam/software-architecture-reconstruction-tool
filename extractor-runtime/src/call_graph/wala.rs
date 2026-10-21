use super::{
    java_adapter::{DEFAULT_ANALYSIS_TIMEOUT, JavaAdapterProvider},
    registry::CallGraphProvider,
};
use models::{
    JavaTestCase,
    call_graph::{CallGraphOutcome, CallGraphRequest, Language},
};
use std::{path::PathBuf, time::Duration};

/// Compatibility facade for the independent WALA adapter.
pub struct WalaJavaProvider {
    inner: JavaAdapterProvider,
}

impl WalaJavaProvider {
    /// Creates a WALA provider that launches the adapter JAR at `jar`.
    pub fn new(jar: impl Into<PathBuf>) -> Self {
        Self::with_timeout(jar, DEFAULT_ANALYSIS_TIMEOUT)
    }

    /// Creates a WALA provider with the legacy WALA algorithm environment setting.
    pub fn with_timeout(jar: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self {
            inner: JavaAdapterProvider::new(
                "wala-java",
                jar,
                timeout,
                std::env::var("WALA_ALGORITHM").unwrap_or_else(|_| "cha".into()),
            )
            .without_artifact_preparation(),
        }
    }

    /// Analyzes discovered JUnit methods and changed callable targets through WALA.
    pub fn analyze_test_roots(
        &self,
        source_root: &std::path::Path,
        test_roots: &[JavaTestCase],
        targets: &[String],
    ) -> CallGraphOutcome {
        self.inner
            .analyze_test_roots(source_root, test_roots, targets)
    }

    pub fn prepare_module_inputs(
        &self,
        module: &std::path::Path,
    ) -> Result<crate::java_analysis::JavaBytecodeInputs, String> {
        self.inner.prepare_module_inputs(module)
    }

    pub fn analyze_prepared_test_roots(
        &self,
        root: &std::path::Path,
        tests: &[JavaTestCase],
        targets: &[String],
        bytecode: &crate::java_analysis::JavaBytecodeInputs,
    ) -> CallGraphOutcome {
        self.inner
            .analyze_prepared_test_roots(root, tests, targets, bytecode)
    }

    pub fn analyze_prepared(
        &self,
        request: &CallGraphRequest,
        bytecode: &crate::java_analysis::JavaBytecodeInputs,
    ) -> CallGraphOutcome {
        self.inner.analyze_prepared(request, bytecode)
    }

    pub fn analyze_modules(
        &self,
        project_root: &std::path::Path,
        modules: &[PathBuf],
        tests: &[JavaTestCase],
        targets: &[String],
    ) -> CallGraphOutcome {
        self.inner
            .analyze_modules(project_root, modules, tests, targets)
    }
}

impl CallGraphProvider for WalaJavaProvider {
    fn language(&self) -> Language {
        self.inner.language()
    }

    fn analyze(&self, request: &CallGraphRequest) -> CallGraphOutcome {
        self.inner.analyze(request)
    }
}
