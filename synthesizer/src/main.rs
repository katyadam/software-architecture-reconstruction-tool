use std::{env, sync::Arc};

use actix_web::{App, HttpServer, middleware::Logger, web};
use clients::http::client::HttpClient;
use models::Entity;
use neo4rs::Graph;

use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    bucket::get_bucket,
    connectors::manager_connector::ManagerConnector,
    contextmap::{
        builder::ContextMapBuilderImpl,
        dto::{GetContextMapErrorReponse, PostContextMap},
        repository::ContextMapRepositoryImpl,
        service::ContextMapServiceImpl,
    },
    db_setup::{setup_contextmap_db, setup_imcg_db, setup_sdg_db},
    s3::{client::S3Client, service::S3Service},
    sdg::{
        builder::SdgBuilderImpl,
        dto::{GetSDGErrorReponse, PostSDGErrorResponse},
        model::SDG,
        repository::SdgRepositoryImpl,
        service::SdgServiceImpl,
    },
};

use awc::Client;

mod bucket;
mod connectors;
mod contextmap;
mod db_setup;
mod errors;
mod imcg;
mod s3;
mod sdg;

#[derive(OpenApi)]
#[openapi(
    paths(
        s3::controller::create_views,
        contextmap::controller::create_context_map,
        contextmap::controller::get_context_map,
        contextmap::controller::delete_context_map,
        sdg::controller::create_sdg,
        sdg::controller::get_sdg,
        sdg::controller::delete_sdg
    ),
    components(schemas(
        PostContextMap,
        Entity,
        GetContextMapErrorReponse,
        SDG,
        PostSDGErrorResponse,
        GetSDGErrorReponse,
    ))
)]
struct ApiDoc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    log::info!("Logger initialized!");

    dotenvy::dotenv().ok();

    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("PORT must be a valid u16 number");

    let url: String = env::var("EXPOSE_URL").unwrap_or_else(|_| "127.0.0.1".to_string());
    let manager_url: String = env::var("MANAGER_URL").expect("MANAGER_URL must be specified!");

    let cm_graph = setup_contextmap_db().await;
    let sdg_graph = setup_sdg_db().await;
    let _imcg_graph = setup_imcg_db().await;

    HttpServer::new(move || {
        let cm_service = Arc::new(get_cm_service(cm_graph.clone()));
        let sdg_service = Arc::new(get_sdg_service(sdg_graph.clone(), &manager_url));

        // Clone Arcs to pass them where needed
        let s3_service = Arc::new(S3Service::new(
            get_s3_client(),
            Arc::clone(&cm_service),
            Arc::clone(&sdg_service),
        ));
        App::new()
            .wrap(Logger::default())
            .service(
                web::scope("/views")
                    .app_data(web::Data::new(s3_service))
                    .configure(s3::configure),
            )
            .service(
                web::scope("/context-maps")
                    .app_data(web::Data::new(cm_service))
                    .configure(contextmap::configure),
            )
            .service(
                web::scope("/sdgs")
                    .app_data(web::Data::new(sdg_service))
                    .configure(sdg::configure),
            )
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-docs/openapi.json", ApiDoc::openapi()),
            )
    })
    .bind((url, port))?
    .run()
    .await
}

fn get_cm_service(graph: Arc<Graph>) -> ContextMapServiceImpl {
    let cm_repository = ContextMapRepositoryImpl::new(graph);
    let cm_builder = ContextMapBuilderImpl::new();

    ContextMapServiceImpl::new(cm_repository, cm_builder)
}

fn get_sdg_service(graph: Arc<Graph>, manager_url: &str) -> SdgServiceImpl {
    let sdg_repository = SdgRepositoryImpl::new(graph);
    let sdg_builder = SdgBuilderImpl::new();
    let manager_connector =
        ManagerConnector::new(HttpClient::new(manager_url.to_owned(), Client::default()));
    SdgServiceImpl::new(sdg_repository, sdg_builder, manager_connector)
}

fn get_s3_client() -> S3Client {
    let bucket = get_bucket();
    S3Client::new(bucket)
}
