use std::sync::Arc;

use actix_web::{App, http::StatusCode, test};
use neo4rs::Graph;
use synthesizer::contextmap::{
    dto::{PostContextMap, PostContextMapErrorResponse},
    model::ContextMap,
};
use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

use crate::{common::configure_test_webapp, contextmap::data::test_entity_email};

#[actix_web::test]
async fn test_create_context_map_returns_202() {
    let _ = env_logger::builder().is_test(true).try_init();
    let graph = Arc::new(
        Graph::new("127.0.0.1:7687", "neo4j", "password")
            .await
            .unwrap(),
    );

    GenericImage::new("neo4j", "latest")
        .with_exposed_port(testcontainers::core::ContainerPort::Tcp(7474))
        .with_network("bridge")
        .with_env_var("NEO4J_AUTH", "neo4j/password")
        .start()
        .await
        .expect("Neo4J Docker Container Started");

    // get a configuration closure
    let configuration = configure_test_webapp(graph);

    // init test app
    let app = test::init_service(App::new().configure(configuration)).await;

    let dto = PostContextMap {
        entities: vec![test_entity_email()],
        codebase_uuid: "f42a8512-e2d4-48c8-bad1-623be3ea0548".to_string(),
    };

    let req = test::TestRequest::post()
        .uri("/context-maps")
        .set_json(&dto)
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let body: PostContextMapErrorResponse = test::read_body_json(resp).await;
    assert_eq!(
        ContextMap {
            entities: vec![test_entity_email()],
            dependencies: vec![]
        },
        body.context_map
    );
}
