use std::env;

use crate::{
    constant::{
        repository::PgConstantRepository,
        service::{ConstantService, ConstantServiceImpl},
    },
    error::ApiError,
};
use actix_web::{App, HttpServer, middleware::Logger, web};
use diesel::{PgConnection, r2d2::ConnectionManager};
use r2d2::Pool;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod constant;
mod error;
mod scanner;
mod schema;

#[derive(OpenApi)]
#[openapi(
    paths(
        constant::controller::save_constants,
        constant::controller::get_constants_by_commit_hash,
        constant::controller::delete_constants_by_commit_hash
    ),
    components(schemas(ApiError))
)]
struct ApiDoc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    log::info!("Logger initialized!");

    dotenvy::dotenv().ok();

    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8084".to_string())
        .parse()
        .expect("PORT must be a valid u16 number");

    let url: String = env::var("EXPOSE_URL").unwrap_or_else(|_| "127.0.0.1".to_string());

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = set_up_database_pool(&database_url, 4);
    HttpServer::new(move || {
        let constant_repo = PgConstantRepository::new(pool.clone());
        let constant_service: Box<dyn ConstantService> =
            Box::new(ConstantServiceImpl::new(Box::new(constant_repo)));
        App::new()
            .wrap(Logger::default())
            .service(
                web::scope("/constants")
                    .app_data(web::Data::new(constant_service))
                    .configure(constant::configure),
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
