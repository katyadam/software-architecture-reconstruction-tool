// Debugging integration test — requires live Ollama at localhost:11434.
// Run with: cargo test -p sage -- --ignored

use models::ir::language::Language;
use sage::resolver::{
    client::{QueryKind, SageClient, SageQuery},
    code::CodeSnippet,
    facts::FactBundle,
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
    };

    let client = SageClient::new("http://localhost:11434/v1", "qwen2.5-coder:7b", 0.7, 150);
    let query = SageQuery {
        bundle,
        kind: QueryKind::ResolveBuilder {
            chain: r#"getServiceUrl("ts-order-service") + "/api/v1/orders/""#.to_string(),
        },
        variables_map: Vec::new(),
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

/// Fixture: empaia/app-service/app_service/api/v3/custom_clients/mps_client.py
///
/// MarketplaceServiceClient stores mps_url as self._base_url in __init__.
/// Methods build URLs from self._base_url, which the static pass cannot resolve.
/// sage should trace mps_url from the instantiation site and resolve the base URL.
#[tokio::test]
#[ignore = "requires live Ollama at localhost:11434"]
async fn mps_client_constructor_injected_url_resolves() {
    let bundle = FactBundle {
        sites: vec![CodeSnippet {
            code: String::from(
                r#"
class MarketplaceServiceClient:
    def __init__(self, http_client, mps_url, organization_id):
        self._http_client = http_client
        self._headers = {"organization-id": organization_id}
        self._base_url = f"{mps_url}/v1"

    async def get_configuration(self, app_id):
        url = f"{self._base_url}/customer/apps/{app_id}/config"
        return await self._http_client.get(url, headers=self._headers)

    async def get_ead(self, app_id):
        url = f"{self._base_url}/customer/apps/{app_id}"
        app = await self._http_client.get(url, headers=self._headers)
        return app["ead"]
"#,
            ),
            language: Language::Python,
        }],
    };

    let client = SageClient::new("http://localhost:11434/v1", "qwen2.5-coder:7b", 0.7, 150);
    let query = SageQuery {
        bundle,
        kind: QueryKind::ResolveLookup {
            lookup_key: "self._base_url".to_string(),
        },
        variables_map: Vec::new(),
    };

    let result = client.query(query).await;

    match result {
        Ok(resp) => {
            println!("resolved:   {:?}", resp.resolved);
            println!("confidence: {}", resp.confidence);
            println!("evidence:   {:?}", resp.evidence);
            println!("reasoning:  {:?}", resp.reasoning);
        }
        Err(e) => println!("query failed: {e}"),
    }
}
