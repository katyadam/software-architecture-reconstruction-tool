use actix_web::{HttpResponse, Responder, delete, get, post, web};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    codebase::{
        dto::{CodebaseResponse, PostCodebase},
        model::NewCodebase,
        service::CodebaseService,
    },
    errors::api::ApiError,
};

#[utoipa::path(
        post,
        path = "/codebases",
        responses(
            (status = 201, description = "Codebase successfully created & saved", body = CodebaseResponse),
            (status = 400, description = "Codebase failed to create", body = ApiError),
        ),
    )]
#[post("")]
pub async fn create_codebase(
    codebase_service: web::Data<Box<dyn CodebaseService>>,
    dto: web::Json<PostCodebase>,
) -> Result<impl Responder, ApiError> {
    let new_codebase = NewCodebase {
        codebase_uuid: Uuid::new_v4(),
        branch: dto.branch.clone(),
        project_uuid: dto.project_uuid,
        created_at: Utc::now(),
    };
    let codebase = codebase_service.create(new_codebase)?; // ? converts DatabaseError -> ApiError

    Ok(HttpResponse::Created().json(codebase.to_response()))
}

#[utoipa::path(
        get,
        path = "/codebases/{codebase_uuid}",
        responses(
            (status = 200, description = "Codebase found", body = CodebaseResponse),
            (status = 404, description = "Codebase not found", body = ApiError),
        ),
    )]
#[get("/{codebase_uuid}")]
pub async fn get_codebase(
    codebase_service: web::Data<Box<dyn CodebaseService>>,
    codebase_uuid_path: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    let codebase_uuid = codebase_uuid_path.into_inner();
    let codebase = codebase_service.get_single(codebase_uuid)?;

    Ok(HttpResponse::Ok().json(codebase.to_response()))
}

#[utoipa::path(
        delete,
        path = "/codebases/{codebase_uuid}",
        responses(
            (status = 204, description = "Codebase successfully deleted"),
            (status = 404, description = "Codebase not found", body = ApiError),
        ),
    )]
#[delete("/{codebase_uuid}")]
pub async fn delete_codebase(
    codebase_service: web::Data<Box<dyn CodebaseService>>,
    codebase_uuid_path: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    let codebase_uuid = codebase_uuid_path.into_inner();
    codebase_service.delete(codebase_uuid)?;

    Ok(HttpResponse::NoContent())
}
