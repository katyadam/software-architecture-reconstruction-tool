use std::env;

use actix_web::{App, HttpServer, middleware::Logger, web};
use diesel::{PgConnection, r2d2::ConnectionManager};
use r2d2::Pool;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    codebase::{dto::CodebaseResponse, repository::PgCodebaseRepository},
    configuration::repository::PgConfigurationRepository,
    errors::ApiError,
    files::repository::PgFileRecordsRepository,
    project::{dto::ProjectResponse, repository::PgProjectRepository},
};

mod codebase;
mod configuration;
mod errors;
mod files;
mod project;
mod schema;

#[derive(OpenApi)]
#[openapi(
    paths(
        project::controller::create_project,
        project::controller::get_project,
        project::controller::delete_project,
        codebase::controller::create_codebase,
        codebase::controller::get_codebase,
        codebase::controller::delete_codebase,
        files::controller::add_record
    ),
    components(schemas(ProjectResponse, ApiError, CodebaseResponse))
)]
struct ApiDoc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    log::info!("Logger initialized!");

    dotenvy::dotenv().ok();

    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8081".to_string())
        .parse()
        .expect("PORT must be a valid u16 number");

    let url: String = env::var("EXPOSE_URL").unwrap_or_else(|_| "127.0.0.1".to_string());

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = set_up_database_pool(&database_url, 4);
    HttpServer::new(move || {
        let project_repo = PgProjectRepository::new(pool.clone());
        let codebase_repo = PgCodebaseRepository::new(pool.clone());
        let file_records_repo = PgFileRecordsRepository::new(pool.clone());
        let configuration_repo = PgConfigurationRepository::new(pool.clone());
        App::new()
            .wrap(Logger::default())
            .service(
                web::scope("/projects")
                    .app_data(web::Data::new(project_repo))
                    .configure(project::configure),
            )
            .service(
                web::scope("/codebases")
                    .app_data(web::Data::new(codebase_repo))
                    .configure(codebase::configure),
            )
            .service(
                web::scope("/file-records")
                    .app_data(web::Data::new(file_records_repo))
                    .configure(files::configure),
            )
            .service(
                web::scope("/configurations")
                    .app_data(web::Data::new(configuration_repo))
                    .configure(configuration::configure),
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

pub fn set_up_database_pool(
    database_url: &str,
    pool_size: u32,
) -> Pool<ConnectionManager<PgConnection>> {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::builder()
        .test_on_check_out(true)
        .max_size(pool_size)
        .build(manager)
        .expect("failed setting up database pool")
}
