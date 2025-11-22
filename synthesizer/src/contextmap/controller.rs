use std::sync::Arc;

use crate::{
    contextmap::{
        dto::{GetContextMapErrorReponse, PostContextMap, PostContextMapErrorResponse},
        model::ContextMap,
        service::{ContextMapService, ContextMapServiceImpl},
    },
    errors::api::ApiError,
};
use actix_web::{HttpResponse, Responder, delete, get, post, web};
use models::api::ProcessFilesIdentifier;

#[utoipa::path(
        post,
        path = "/context-maps",
        responses(
            (status = 201, description = "Context Map successfully created & saved", body = ContextMap),
            (status = 202, description = "Context Map successfully created, saving failed", body = PostContextMapErrorResponse),
        ),
    )]
#[post("")]
pub async fn create_context_map(
    service: web::Data<Arc<ContextMapServiceImpl>>,
    dto: web::Json<PostContextMap>,
) -> Result<impl Responder, ApiError> {
    let dto = dto.into_inner();
    let cm = service.save(dto).await?;

    Ok(HttpResponse::Created().json(cm))
}

#[utoipa::path(
    get,
    path = "/context-maps/{codebase_uuid}/{commit_hash}",
    params(
        ("codebase_uuid", Path, description = "Codebase UUID of the context map to get", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
        ("commit_hash", Path, description = "Git commit hash of the context map to get", example = "abc123def4567890abc123def4567890abc123de")
    ),
    responses(
        (status = 200, description = "Context Map successfully retrieved", body = ContextMap),
        (status = 400, description = "Context Map cannot be retrieved", body = GetContextMapErrorReponse),
    ),
)]
#[get("/{codebase_uuid}/{commit_hash}")]
pub async fn get_context_map(
    service: web::Data<Arc<ContextMapServiceImpl>>,
    path: web::Path<ProcessFilesIdentifier>,
) -> Result<impl Responder, ApiError> {
    let identifier = path.into_inner();

    let cm = service
        .get_single(identifier.codebase_uuid, &identifier.commit_hash)
        .await?;

    Ok(HttpResponse::Ok().json(cm))
}

#[utoipa::path(
    delete,
    path = "/context-maps/{codebase_uuid}/{commit_hash}",
    params(
        ("codebase_uuid", Path, description = "Codebase UUID of the context map to delete", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
        ("commit_hash", Path, description = "Git commit hash of the context map to delete", example = "abc123def4567890abc123def4567890abc123de")
    ),
    responses(
        (status = 204, description = "Context Map successfully deleted"),
        (status = 400, description = "Context Map couldn't be deleted"),
    ),
)]
#[delete("/{codebase_uuid}/{commit_hash}")]
pub async fn delete_context_map(
    service: web::Data<Arc<ContextMapServiceImpl>>,
    path: web::Path<ProcessFilesIdentifier>,
) -> Result<impl Responder, ApiError> {
    let identifier = path.into_inner();

    service
        .delete(identifier.codebase_uuid, &identifier.commit_hash)
        .await?;

    Ok(HttpResponse::NoContent())
}
