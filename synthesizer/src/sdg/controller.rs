use std::sync::Arc;

use actix_web::{HttpResponse, Responder, delete, get, post, web};
use models::api::ProcessFilesIdentifier;

use crate::{
    errors::api::ApiError,
    sdg::{
        dto::{GetSDGErrorReponse, PostSDG, PostSDGErrorResponse},
        model::Sdg,
        service::{SdgService, SdgServiceImpl},
    },
};

#[utoipa::path(
        post,
        path = "/sdgs",
        responses(
            (status = 201, description = "SDG successfully created & saved", body = Sdg),
            (status = 202, description = "SDG successfully created, saving failed", body = PostSDGErrorResponse),
        ),
    )]
#[post("")]
pub async fn create_sdg(
    service: web::Data<Arc<SdgServiceImpl>>,
    dto: web::Json<PostSDG>,
) -> Result<impl Responder, ApiError> {
    let dto = dto.into_inner();
    let sdg = service.save(dto).await?;

    Ok(HttpResponse::Created().json(sdg))
}

#[utoipa::path(
    get,
    path = "/sdgs/{codebase_uuid}/{commit_hash}",
    params(
        ("codebase_uuid", Path, description = "Codebase UUID of the SDG to get", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
        ("commit_hash", Path, description = "Git commit hash of the SDG to get", example = "abc123def4567890abc123def4567890abc123de")
    ),
    responses(
        (status = 200, description = "SDG successfully retrieved", body = Sdg),
        (status = 400, description = "SDG cannot be retrieved", body = GetSDGErrorReponse),
    ),
)]
#[get("/{codebase_uuid}/{commit_hash}")]
pub async fn get_sdg(
    service: web::Data<Arc<SdgServiceImpl>>,
    path: web::Path<ProcessFilesIdentifier>,
) -> Result<impl Responder, ApiError> {
    let identifier = path.into_inner();

    let sdg = service
        .get_single(identifier.codebase_uuid, &identifier.commit_hash)
        .await?;

    Ok(HttpResponse::Ok().json(sdg))
}

#[utoipa::path(
    delete,
    path = "/sdgs/{codebase_uuid}/{commit_hash}",
    params(
        ("codebase_uuid", Path, description = "Codebase UUID of the SDG to delete", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
        ("commit_hash", Path, description = "Git commit hash of the SDG to delete", example = "abc123def4567890abc123def4567890abc123de")
    ),
    responses(
        (status = 204, description = "Service Dependency Graph successfully deleted"),
        (status = 400, description = "Service Dependency Graph couldn't be deleted"),
    ),
)]
#[delete("/{codebase_uuid}/{commit_hash}")]
pub async fn delete_sdg(
    service: web::Data<Arc<SdgServiceImpl>>,
    path: web::Path<ProcessFilesIdentifier>,
) -> Result<impl Responder, ApiError> {
    let identifier = path.into_inner();

    service
        .delete(identifier.codebase_uuid, &identifier.commit_hash)
        .await?;

    Ok(HttpResponse::NoContent())
}
