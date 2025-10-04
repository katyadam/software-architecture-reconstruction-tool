use std::env;

use crate::{
    api::{
        connectors::{
            manager_connector::ManagerConnector, synthesizer_connector::SynthesizerConnector,
        },
        dto::MultipleFileUploadSchema,
        service::ExtractorServiceImpl,
    },
    client::http::client::HttpClient,
};
use actix_web::{App, HttpServer, middleware::Logger, web};
use awc::Client;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod api;
mod client;
mod error;
mod utils;

#[derive(OpenApi)]
#[openapi(
    paths(api::controller::process_files),
    components(schemas(MultipleFileUploadSchema))
)]
struct ApiDoc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    log::info!("Logger initialized!");

    dotenvy::dotenv().ok();

    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8082".to_string())
        .parse()
        .expect("PORT must be a valid u16 number");

    let url: String = env::var("EXPOSE_URL").unwrap_or_else(|_| "127.0.0.1".to_string());

    HttpServer::new(move || {
        let manager_url: String = env::var("MANAGER_URL")
            .unwrap_or_else(|_| "http://localhost:8081".to_string())
            .parse()
            .expect("MANAGER_URL must be a valid String");

        let synthesizer_url: String = env::var("SYNTHESIZER_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string())
            .parse()
            .expect("SYNTHESIZER_URL must be a valid String");

        let synthesizer_connector =
            SynthesizerConnector::new(HttpClient::new(synthesizer_url, Client::default()));
        let manager_connector =
            ManagerConnector::new(HttpClient::new(manager_url, Client::default()));

        let extractor_service = ExtractorServiceImpl::new(manager_connector, synthesizer_connector);
        App::new()
            .wrap(Logger::default())
            .service(
                web::scope("/process-files")
                    .app_data(web::Data::new(extractor_service))
                    .configure(api::configure),
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
