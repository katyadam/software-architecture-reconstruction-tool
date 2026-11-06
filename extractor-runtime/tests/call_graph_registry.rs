use extractor_runtime::call_graph::{CallGraphProvider, ProviderRegistry};
use models::call_graph::{CallGraphOutcome, CallGraphRequest, CallGraphStatus, Language};

struct JavaProvider;

impl CallGraphProvider for JavaProvider {
    fn language(&self) -> Language {
        Language::Java
    }

    fn analyze(&self, request: &CallGraphRequest) -> CallGraphOutcome {
        CallGraphOutcome::ok(request.clone(), "test-java", "test")
    }
}

#[test]
fn routes_a_request_to_its_registered_language_provider() {
    let registry = ProviderRegistry::new(vec![Box::new(JavaProvider)]);
    let request = CallGraphRequest::new("/service", Language::Java);

    let outcome = registry.analyze(&request);

    assert_eq!(outcome.status, CallGraphStatus::Ok);
    assert_eq!(outcome.provider_id, "test-java");
    assert_eq!(outcome.source_root, "/service");
}

#[test]
fn reports_unsupported_for_unregistered_languages() {
    let registry = ProviderRegistry::new(vec![]);

    for language in [Language::Go, Language::Python] {
        let outcome = registry.analyze(&CallGraphRequest::new("/service", language));
        assert_eq!(outcome.status, CallGraphStatus::Unsupported);
        assert!(outcome.edges.is_empty());
    }
}
