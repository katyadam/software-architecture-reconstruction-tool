// Debugging integration test — requires live Ollama at localhost:11434.
// Run with: cargo test -p sage -- --ignored

use sage::resolver::{
    client::{SageClient, SageQuery},
    query::{CandidateService, ClassifyContext, QueryKind},
};

/// Fixture: empaia/app-service — a MedicalDataServiceClient call whose target
/// URL (`self._mds_url + url`) could not be resolved statically. The closed-set
/// classifier should pick `medical-data-service` from the candidate set.
#[tokio::test]
#[ignore = "requires live Ollama at localhost:11434"]
async fn mds_client_classifies_to_medical_data_service() {
    let candidates = vec![
        CandidateService {
            name: "medical-data-service".to_string(),
            url: "http://medical-data-service:8000".to_string(),
        },
        CandidateService {
            name: "clinical-data-service".to_string(),
            url: "http://clinical-data-service:8000".to_string(),
        },
        CandidateService {
            name: "job-service".to_string(),
            url: "http://job-service:8000".to_string(),
        },
    ];

    let context = ClassifyContext {
        origin_service: "app-service".to_string(),
        client_class: Some("MedicalDataServiceClient".to_string()),
        imports: vec![
            "app_service.api.v3.custom_clients.mds_client.MedicalDataServiceClient".to_string(),
        ],
        expression: "self._mds_url + url".to_string(),
        operand_identifiers: vec!["self._mds_url".to_string(), "url".to_string()],
    };

    let client = SageClient::new("http://localhost:11434/v1", "qwen2.5-coder:7b");
    let query = SageQuery {
        kind: QueryKind::ClassifyTargetService { candidates },
        context,
    };

    let result = client.query(query).await;

    match result {
        Ok(resp) => {
            println!("service:   {:?}", resp.service);
            println!("evidence:  {:?}", resp.evidence);
            println!("reasoning: {:?}", resp.reasoning);
            assert_eq!(resp.service.as_deref(), Some("medical-data-service"));
        }
        Err(e) => panic!("query failed: {e}"),
    }
}

/// A thin-signal call site: no client class, generic `base_url`. The classifier
/// is allowed to abstain (`None`) or pick a candidate; either is a valid,
/// non-panicking outcome. Exercises the end-to-end path, not correctness.
#[tokio::test]
#[ignore = "requires live Ollama at localhost:11434"]
async fn thin_signal_call_site_runs() {
    let candidates = vec![CandidateService {
        name: "marketplace-service".to_string(),
        url: "http://marketplace-service:8000".to_string(),
    }];

    let context = ClassifyContext {
        origin_service: "app-service".to_string(),
        client_class: None,
        imports: vec![],
        expression: "self._base_url + \"/v1/customer\"".to_string(),
        operand_identifiers: vec!["self._base_url".to_string()],
    };

    let client = SageClient::new("http://localhost:11434/v1", "qwen2.5-coder:7b");
    let query = SageQuery {
        kind: QueryKind::ClassifyTargetService { candidates },
        context,
    };

    match client.query(query).await {
        Ok(resp) => {
            println!("service:   {:?}", resp.service);
            println!("evidence:  {:?}", resp.evidence);
        }
        Err(e) => println!("query failed: {e}"),
    }
}
