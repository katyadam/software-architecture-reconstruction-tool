use actix_web::{HttpResponse, Responder, delete, get, post, web};

use crate::{
    constant::{
        dto::{ConstantBatchInput, ConstantResponse},
        model::Constant,
        service::ConstantService,
    },
    error::ApiError,
};

#[utoipa::path(
        get,
        path = "/constants/{commit_hash}",
        responses(
            (status = 201, description = "Constants successfully loaded", body = Vec<ConstantResponse>),
            (status = 400, description = "Constants failed to load", body = ApiError),
        ),
    )]
#[get("/{commit_hash}")]
pub async fn get_constants_by_commit_hash(
    constant_service: web::Data<Box<dyn ConstantService>>,
    commit_hash_path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let constants = constant_service
        .get_by_commit_hash(&commit_hash_path.into_inner())?
        .iter()
        .map(Constant::to_response)
        .collect::<Vec<ConstantResponse>>();

    Ok(HttpResponse::Created().json(constants))
}

#[utoipa::path(
        post,
        path = "/constants/batch",
        responses(
            (status = 201, description = "Constants successfully saved", body = Vec<ConstantResponse>),
            (status = 400, description = "Constants failed to save", body = ApiError),
        ),
    )]
#[post("/batch")]
pub async fn save_constants(
    constant_service: web::Data<Box<dyn ConstantService>>,
    dto: web::Json<ConstantBatchInput>,
) -> Result<impl Responder, ApiError> {
    let constants = constant_service
        .create_batch_from_keyvalues(dto.0)?
        .iter()
        .map(Constant::to_response)
        .collect::<Vec<ConstantResponse>>();

    Ok(HttpResponse::Created().json(constants))
}

#[utoipa::path(
        post,
        path = "/constants/{commit_hash}",
        responses(
            (status = 200, description = "Constants successfully deleted"),
            (status = 400, description = "Constants failed to delete", body = ApiError),
        ),
    )]
#[delete("/{commit_hash}")]
pub async fn delete_constants_by_commit_hash(
    constant_service: web::Data<Box<dyn ConstantService>>,
    commit_hash_path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    constant_service.delete_by_commit_hash(&commit_hash_path.into_inner())?;

    Ok(HttpResponse::Ok())
}
