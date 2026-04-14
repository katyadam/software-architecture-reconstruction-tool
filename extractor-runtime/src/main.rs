use std::env;

use crate::{
    api::{
        connectors::{
            manager_connector::ManagerConnector, s3_connector::S3Connector,
            synthesizer_connector::SynthesizerConnector,
        },
        dto::MultipleFileUploadSchema,
        service::ExtractorRuntimeServiceImpl,
    },
    bucket::get_bucket,
    client::s3::client::S3Client,
};
use actix_cors::Cors;
use actix_web::{App, HttpServer, middleware::Logger, web};
use awc::Client;
use clients::http::client::HttpClient;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod api;
mod bucket;
mod client;
mod error;
#[macro_use]
mod macros;
mod pipeline;

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

    let _testing_client_url: String =
        env::var("TEST_CLIENT_URL").unwrap_or_else(|_| "127.0.0.1".to_string());

    HttpServer::new(move || {
        let bucket = get_bucket();

        let manager_url: String = env::var("MANAGER_URL")
            .unwrap_or_else(|_| "http://localhost:8081".to_string())
            .parse()
            .expect("MANAGER_URL must be a valid String");

        let synthesizer_url: String = env::var("SYNTHESIZER_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string())
            .parse()
            .expect("SYNTHESIZER_URL must be a valid String");
        let s3_client = S3Client::new(bucket);

        let synthesizer_connector =
            SynthesizerConnector::new(HttpClient::new(synthesizer_url, Client::default()));
        let manager_connector =
            ManagerConnector::new(HttpClient::new(manager_url, Client::default()));
        let s3_connector = S3Connector::new(s3_client);

        let extractor_service = ExtractorRuntimeServiceImpl::new(
            manager_connector,
            synthesizer_connector,
            s3_connector,
        );
        App::new()
            .wrap(Logger::default())
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header(),
            )
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
