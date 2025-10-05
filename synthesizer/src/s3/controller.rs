use actix_web::{HttpResponse, Responder, post, web};

use crate::errors::api::ApiError;

#[utoipa::path(
        post,
        path = "/views",
        responses(
            (status = 200, description = "Chunks successfully loaded, views created.", body = String),
        ),
    )]
#[post("")]
pub async fn create_views(base_dir_path: web::Json<String>) -> Result<impl Responder, ApiError> {
    let base_dir_path = base_dir_path.into_inner();

    Ok(HttpResponse::Created().json(base_dir_path))
}
