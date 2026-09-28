use models::call_graph::{CallGraphOutcome, CallGraphRequest, Language};

pub trait CallGraphProvider: Send + Sync {
    fn language(&self) -> Language;
    fn analyze(&self, request: &CallGraphRequest) -> CallGraphOutcome;
}

pub struct ProviderRegistry {
    providers: Vec<Box<dyn CallGraphProvider>>,
}
impl ProviderRegistry {
    pub fn new(providers: Vec<Box<dyn CallGraphProvider>>) -> Self {
        Self { providers }
    }
    pub fn analyze(&self, request: &CallGraphRequest) -> CallGraphOutcome {
        self.providers
            .iter()
            .find(|provider| provider.language() == request.language)
            .map(|provider| provider.analyze(request))
            .unwrap_or_else(|| CallGraphOutcome::unsupported(request))
    }
}
