use std::env;

use actix_web::{App, HttpServer, middleware::Logger, web};
use models::Entity;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    contextmap::dto::{GetContextMapErrorReponse, PostContextMap},
    db_setup::{setup_contextmap_db, setup_imcg_db, setup_sdg_db},
    sdg::{
        dto::{GetSDGErrorReponse, PostSDGErrorResponse},
        model::SDG,
    },
};
mod contextmap;
mod db_setup;
mod imcg;
mod sdg;

#[derive(OpenApi)]
#[openapi(
    paths(
        contextmap::controller::create_context_map,
        contextmap::controller::get_context_map,
        contextmap::controller::delete_context_map,
        sdg::controller::create_sdg,
        sdg::controller::get_sdg,
    ),
    components(schemas(
        PostContextMap,
        Entity,
        GetContextMapErrorReponse,
        SDG,
        PostSDGErrorResponse,
        GetSDGErrorReponse
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
        App::new()
            .wrap(Logger::default())
            .service(
                web::scope("/context-maps")
                    .app_data(web::Data::new(cm_graph.clone()))
                    .configure(contextmap::configure),
            )
            .service(
                web::scope("/sdgs")
                    .app_data(web::Data::new(sdg_graph.clone()))
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
