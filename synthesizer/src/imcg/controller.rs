use std::sync::Arc;

use crate::{
    errors::api::ApiError,
    imcg::{
        dto::{GetIMCGErrorReponse, PostIMCG, PostIMCGErrorResponse},
        model::Imcg,
        service::{ImcgService, ImcgServiceImpl},
    },
};
use actix_web::{HttpResponse, Responder, delete, get, post, web};
use models::api::ProcessFilesIdentifier;

#[utoipa::path(
        post,
        path = "/imcgs",
        responses(
            (status = 201, description = "IMCG successfully created & saved", body = Imcg),
            (status = 202, description = "IMCG successfully created, saving failed", body = PostIMCGErrorResponse),
        ),
    )]
#[post("")]
pub async fn create_imcg(
    service: web::Data<Arc<ImcgServiceImpl>>,
    dto: web::Json<PostIMCG>,
) -> Result<impl Responder, ApiError> {
    let dto = dto.into_inner();
    let imcg = service.save(dto).await?;

    Ok(HttpResponse::Created().json(imcg))
}

#[utoipa::path(
    get,
    path = "/imcgs/{codebase_uuid}/{commit_hash}",
    params(
        ("codebase_uuid", Path, description = "Codebase UUID of the IMCG to get", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
        ("commit_hash", Path, description = "Git commit hash of the IMCG to get", example = "abc123def4567890abc123def4567890abc123de")
    ),
    responses(
        (status = 200, description = "IMCG successfully retrieved", body = Imcg),
        (status = 400, description = "IMCG cannot be retrieved", body = GetIMCGErrorReponse),
    ),
)]
#[get("/{codebase_uuid}/{commit_hash}")]
pub async fn get_imcg(
    service: web::Data<Arc<ImcgServiceImpl>>,
    path: web::Path<ProcessFilesIdentifier>,
) -> Result<impl Responder, ApiError> {
    let identifier = path.into_inner();

    let imcg = service
        .get_single(identifier.codebase_uuid, &identifier.commit_hash)
        .await?;

    Ok(HttpResponse::Ok().json(imcg))
}

#[utoipa::path(
    delete,
    path = "/imcgs/{codebase_uuid}/{commit_hash}",
    params(
        ("codebase_uuid", Path, description = "Codebase UUID of the IMCG to delete", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
        ("commit_hash", Path, description = "Git commit hash of the IMCG to delete", example = "abc123def4567890abc123def4567890abc123de")
    ),
    responses(
        (status = 204, description = "IMCG successfully deleted"),
        (status = 400, description = "IMCG couldn't be deleted"),
    ),
)]
#[delete("/{codebase_uuid}/{commit_hash}")]
pub async fn delete_imcg(
    service: web::Data<Arc<ImcgServiceImpl>>,
    path: web::Path<ProcessFilesIdentifier>,
) -> Result<impl Responder, ApiError> {
    let identifier = path.into_inner();

    service
        .delete(identifier.codebase_uuid, &identifier.commit_hash)
        .await?;

    Ok(HttpResponse::NoContent())
}
