use std::sync::Arc;

use actix_web::web::{self, Data, ServiceConfig};
use neo4rs::Graph;
use synthesizer::contextmap::{
    self, builder::ContextMapBuilderImpl, repository::ContextMapRepositoryImpl,
    service::ContextMapServiceImpl,
};

#[allow(dead_code)]
pub fn configure_test_webapp(graph: Arc<Graph>) -> impl Fn(&mut ServiceConfig) {
    move |config: &mut ServiceConfig| {
        let cm_service = get_cm_service(graph.clone());
        config.service(
            web::scope("/context-maps")
                .app_data(Data::new(cm_service))
                .configure(contextmap::configure),
        );
    }
}

fn get_cm_service(graph: Arc<Graph>) -> ContextMapServiceImpl {
    let cm_repository = ContextMapRepositoryImpl::new(graph);
    let cm_builder = ContextMapBuilderImpl::new();

    ContextMapServiceImpl::new(cm_repository, cm_builder)
}
