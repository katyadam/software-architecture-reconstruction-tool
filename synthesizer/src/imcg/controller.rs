use std::sync::Arc;

use crate::{
    errors::api::ApiError,
    imcg::{
        dto::{GetIMCGErrorReponse, PostIMCG, PostIMCGErrorResponse},
        model::IMCG,
        service::{ImcgService, ImcgServiceImpl},
    },
};
use actix_web::{HttpResponse, Responder, delete, get, post, web};
use uuid::Uuid;

#[utoipa::path(
        post,
        path = "/imcgs",
        responses(
            (status = 201, description = "IMCG successfully created & saved", body = IMCG),
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
        path = "/imcgs/{codebase_uuid}",
        params(
            ("codebase_uuid", Path, description = "Codebase UUID of the IMCG to get", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")
        ),
        responses(
            (status = 200, description = "IMCG successfully retrieved", body = IMCG),
            (status = 400, description = "IMCG  cannot be retrieved", body = GetIMCGErrorReponse),
        ),
    )]
#[get("/{codebase_uuid}")]
pub async fn get_imcg(
    service: web::Data<Arc<ImcgServiceImpl>>,
    codebase_uuid_path: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    let codebase_uuid = codebase_uuid_path.into_inner();
    let cm = service.get_single(codebase_uuid).await?;

    Ok(HttpResponse::Ok().json(cm))
}

#[utoipa::path(
        delete,
        path = "/imcgs/{codebase_uuid}",
        params(
            ("codebase_uuid", Path, description = "Codebase UUID of the IMCG to delete", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")
        ),
        responses(
            (status = 204, description = "IMCG successfully deleted"),
            (status = 400, description = "IMCG couldn't be deleted"),
        ),
    )]
#[delete("/{codebase_uuid}")]
pub async fn delete_imcg(
    service: web::Data<Arc<ImcgServiceImpl>>,
    codebase_uuid_path: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    let codebase_uuid = codebase_uuid_path.into_inner();
    service.delete(codebase_uuid).await?;

    Ok(HttpResponse::NoContent())
}
