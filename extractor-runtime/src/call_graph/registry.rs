use models::call_graph::{CallGraphOutcome, CallGraphRequest, Language};

/// Defines the interchangeable contract implemented by each language-specific analyzer.
pub trait CallGraphProvider: Send + Sync {
    /// Returns the language that this provider can analyze.
    fn language(&self) -> Language;
    /// Analyzes the requested source root and returns provider-neutral call-graph data.
    fn analyze(&self, request: &CallGraphRequest) -> CallGraphOutcome;
}

/// Selects the provider that supports a requested language.
pub struct ProviderRegistry {
    providers: Vec<Box<dyn CallGraphProvider>>,
}
impl ProviderRegistry {
    /// Creates a registry from the providers available in the current deployment.
    pub fn new(providers: Vec<Box<dyn CallGraphProvider>>) -> Self {
        Self { providers }
    }
    /// Delegates analysis to the matching provider or returns an unsupported result.
    pub fn analyze(&self, request: &CallGraphRequest) -> CallGraphOutcome {
        self.providers
            .iter()
            .find(|provider| provider.language() == request.language)
            .map(|provider| provider.analyze(request))
            .unwrap_or_else(|| CallGraphOutcome::unsupported(request))
    }
}
