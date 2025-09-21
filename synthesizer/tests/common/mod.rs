use std::sync::Arc;

use actix_web::web::{self, Data, ServiceConfig};
use neo4rs::Graph;
use synthesizer::contextmap::{self};

pub fn configure_test_webapp(graph: Arc<Graph>) -> impl Fn(&mut ServiceConfig) {
    move |config: &mut ServiceConfig| {
        config
            .service(web::scope("/context-maps").configure(contextmap::configure))
            .app_data(Data::new(graph.clone()));
    }
}
