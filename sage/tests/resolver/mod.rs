// Debugging integration test — requires live Ollama at localhost:11434.
// Run with: cargo test -p sage -- --ignored

use models::ir::{
    language::{Framework, Language},
    project::ConstantValue,
};
use sage::resolver::{
    client::{QueryKind, SageClient, SageQuery},
    code::{CodeSnippet, Symbol, SymbolKind},
    facts::FactBundle,
    messages::Message,
};

/// Fixture: java_billing_service/client/OrderClient.java
///
/// OrderClient builds the URL via getServiceUrl("ts-order-service") + "/api/v1/orders/".
/// getServiceUrl() returns "http://" + serviceName. The static pass cannot trace
/// this; sage should resolve it to "http://ts-order-service/api/v1/orders/".
#[tokio::test]
#[ignore = "requires live Ollama at localhost:11434"]
async fn order_client_builder_chain_resolves_to_full_url() {
    let bundle = FactBundle {
        sites: vec![CodeSnippet {
            code: r#"String order_service_url = getServiceUrl("ts-order-service");
restTemplate.exchange(order_service_url + "/api/v1/orders/", HttpMethod.GET, requestEntity, new ParameterizedTypeReference<List<Order>>() {});"#
                .to_string(),
            language: Language::Java,
        }],
        frameworks: vec![Framework::Spring],
        local_scope: vec![
            Symbol {
                name: "order_service_url".to_string(),
                value: None,
                datatype: Some("String".to_string()),
                kind: SymbolKind::Named,
            },
            Symbol {
                name: "restTemplate".to_string(),
                value: None,
                datatype: Some("RestTemplate".to_string()),
                kind: SymbolKind::Named,
            },
        ],
        imported_scope: vec![],
        class_or_module_attrs: vec![],
        constants: vec![ConstantValue {
            name: "getServiceUrl".to_string(),
            value: r#""http://" + serviceName"#.to_string(),
            source_file: "java_billing_service/client/OrderClient.java".to_string(),
        }],
        others: vec![Message {
            text: r#"getServiceUrl(String serviceName) { return "http://" + serviceName; }"#
                .to_string(),
        }],
    };

    let client = SageClient::new("http://localhost:11434/v1", "qwen2.5-coder:7b", 0.7);
    let query = SageQuery {
        bundle,
        kind: QueryKind::ResolveBuilder {
            chain: r#"getServiceUrl("ts-order-service") + "/api/v1/orders/""#.to_string(),
        },
    };

    let result = client.query(query).await;

    match result {
        Ok(resp) => {
            println!("resolved:   {:?}", resp.resolved);
            println!("confidence: {}", resp.confidence);
            println!("evidence:   {:?}", resp.evidence);
            println!("reasoning:  {:?}", resp.reasoning);
            assert!(resp.confidence >= 0.7);
        }
        Err(e) => panic!("query failed: {e}"),
    }
}
