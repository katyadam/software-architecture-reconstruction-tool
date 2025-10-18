use std::sync::Arc;

use actix_web::{App, http::StatusCode, test, web};
use neo4rs::Graph;
use synthesizer::contextmap::{
    self, builder::ContextMapBuilderImpl, dto::PostContextMap, model::ContextMap,
    repository::ContextMapRepositoryImpl, service::ContextMapServiceImpl,
};
use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};
use uuid::Uuid;

use crate::contextmap::data::test_entity_email;

fn get_cm_service(graph: Arc<Graph>) -> ContextMapServiceImpl {
    let cm_repository = ContextMapRepositoryImpl::new(graph);
    let cm_builder = ContextMapBuilderImpl::new();

    ContextMapServiceImpl::new(cm_repository, cm_builder)
}

#[actix_web::test]
async fn test_create_context_map_returns_201() {
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
    // let configuration = configure_test_webapp(graph);

    // init test app
    let app = test::init_service(
        App::new().service(
            web::scope("/context-maps")
                .app_data(web::Data::new(Arc::new(get_cm_service(graph.clone()))))
                .configure(contextmap::configure),
        ),
    )
    .await;

    let dto = PostContextMap {
        entities: vec![test_entity_email()],
        codebase_uuid: Uuid::parse_str("f42a8512-e2d4-48c8-bad1-623be3ea0548").unwrap(),
    };

    let req = test::TestRequest::post()
        .uri("/context-maps")
        .set_json(&dto)
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::CREATED);

    let body: ContextMap = test::read_body_json(resp).await;
    assert_eq!(
        ContextMap {
            entities: vec![test_entity_email()],
            dependencies: vec![]
        },
        body
    );
}
