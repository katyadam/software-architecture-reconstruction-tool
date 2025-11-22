use std::env;

use actix_web::{App, HttpServer, middleware::Logger, web};
use diesel::{PgConnection, r2d2::ConnectionManager};
use r2d2::Pool;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    codebase::{
        dto::CodebaseResponse,
        repository::PgCodebaseRepository,
        service::{CodebaseService, CodebaseServiceImpl},
    },
    commit::{
        dto::CommitResponse,
        repository::PgCommitRepository,
        service::{CommitService, CommitServiceImpl},
    },
    configuration::{
        repository::PgConfigurationRepository,
        service::{ConfigurationService, ConfigurationServiceImpl},
    },
    errors::api::ApiError,
    files::{
        repository::PgFileRecordsRepository,
        service::{FileRecordsService, FileRecordsServiceImpl},
    },
    project::{
        dto::ProjectResponse,
        repository::PgProjectRepository,
        service::{ProjectService, ProjectServiceImpl},
    },
};

mod codebase;
mod commit;
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
        codebase::controller::get_codebase_configuration,
        files::controller::add_record,
        configuration::controller::create_configuration,
        configuration::controller::get_configuration,
        configuration::controller::delete_configuration,
        commit::controller::create_commit,
        commit::controller::get_commit,
        commit::controller::delete_commit,
    ),
    components(schemas(ProjectResponse, ApiError, CodebaseResponse, CommitResponse))
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
        let file_records_repo = PgFileRecordsRepository::new(pool.clone());
        let configuration_repo = PgConfigurationRepository::new(pool.clone());
        let codebase_repo = PgCodebaseRepository::new(pool.clone());
        let commit_repo = PgCommitRepository::new(pool.clone());

        let project_service: Box<dyn ProjectService> =
            Box::new(ProjectServiceImpl::new(Box::new(project_repo)));

        let file_records_service: Box<dyn FileRecordsService> =
            Box::new(FileRecordsServiceImpl::new(Box::new(file_records_repo)));

        let codebase_service: Box<dyn CodebaseService> =
            Box::new(CodebaseServiceImpl::new(Box::new(codebase_repo)));

        let configuration_service: Box<dyn ConfigurationService> =
            Box::new(ConfigurationServiceImpl::new(Box::new(configuration_repo)));

        let commit_service: Box<dyn CommitService> =
            Box::new(CommitServiceImpl::new(Box::new(commit_repo)));

        App::new()
            .wrap(Logger::default())
            .service(
                web::scope("/projects")
                    .app_data(web::Data::new(project_service))
                    .configure(project::configure),
            )
            .service(
                web::scope("/codebases")
                    .app_data(web::Data::new(codebase_service))
                    .configure(codebase::configure),
            )
            .service(
                web::scope("/file-records")
                    .app_data(web::Data::new(file_records_service))
                    .configure(files::configure),
            )
            .service(
                web::scope("/configurations")
                    .app_data(web::Data::new(configuration_service))
                    .configure(configuration::configure),
            )
            .service(
                web::scope("/commits")
                    .app_data(web::Data::new(commit_service))
                    .configure(commit::configure),
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
