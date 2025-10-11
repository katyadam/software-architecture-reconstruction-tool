use std::sync::Arc;

use actix_web::{HttpResponse, Responder, post, web};
use log::debug;

use crate::{
    errors::api::ApiError,
    s3::{dto::PostViews, service::S3Service},
};

#[utoipa::path(
        post,
        path = "/views",
        responses(
            (status = 200, description = "Chunks successfully loaded, views created.", body = String),
        ),
    )]
#[post("")]
pub async fn create_views(
    service: web::Data<Arc<S3Service>>,
    dto: web::Json<PostViews>,
) -> Result<impl Responder, ApiError> {
    let dto = dto.into_inner();
    service.save_views(dto).await?;

    Ok(HttpResponse::Created().json("code elements loaded, views created and stored"))
}
