use std::{env, sync::Arc};

use actix_web::{App, HttpServer, middleware::Logger, web};
use models::Entity;
use neo4rs::Graph;

use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    bucket::get_bucket,
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
mod bucket;
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

    let cm_graph = setup_contextmap_db().await;
    let sdg_graph = setup_sdg_db().await;
    let imcg_graph = setup_imcg_db().await;

    HttpServer::new(move || {
        let cm_service = get_cm_service(cm_graph.clone());
        let sdg_service = get_sdg_service(sdg_graph.clone());
        let s3_service = S3Service::new(get_s3_client(), cm_service, sdg_service);
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

fn get_sdg_service(graph: Arc<Graph>) -> SdgServiceImpl {
    let sdg_repository = SdgRepositoryImpl::new(graph);
    let sdg_builder = SdgBuilderImpl::new();

    SdgServiceImpl::new(sdg_repository, sdg_builder)
}

fn get_s3_client() -> S3Client {
    let bucket = get_bucket();
    S3Client::new(bucket)
}
