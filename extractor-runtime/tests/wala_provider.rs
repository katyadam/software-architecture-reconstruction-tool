use extractor_runtime::call_graph::{CallGraphProvider, wala::WalaJavaProvider};
use models::call_graph::{CallGraphRequest, CallGraphStatus, Language};

#[test]
fn missing_adapter_jar_is_a_non_fatal_failed_outcome() {
    let provider = WalaJavaProvider::new("/does-not-exist/wala.jar");
    let result = provider.analyze(&CallGraphRequest::new("/service", Language::Java));
    assert_eq!(result.status, CallGraphStatus::Failed);
    assert!(result.edges.is_empty());
}
