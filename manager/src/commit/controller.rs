use actix_web::{HttpResponse, Responder, delete, get, patch, post, web};
use chrono::Utc;

use crate::{
    commit::{
        dto::{CommitResponse, PostCommit},
        model::NewCommit,
        service::CommitService,
    },
    configuration::{dto::ConfigurationResponse, model::DbConfiguration},
    errors::api::ApiError,
};

#[utoipa::path(
        post,
        path = "/commits",
        responses(
            (status = 201, description = "Commit successfully created & saved", body = CommitResponse),
            (status = 400, description = "Commit failed to create", body = ApiError),
        ),
    )]
#[post("")]
pub async fn create_commit(
    commit_service: web::Data<Box<dyn CommitService>>,
    dto: web::Json<PostCommit>,
) -> Result<impl Responder, ApiError> {
    let new_commit = NewCommit {
        commit_hash: dto.commit_hash.clone(),
        commit_message: dto.commit_message.clone(),
        codebase_uuid: dto.codebase_uuid,
        created_at: Utc::now(),
        processed: false,
        configuration_uuid: dto.configuration_uuid,
    };
    let commit = commit_service.create(new_commit)?; // ? converts DatabaseError -> ApiError

    Ok(HttpResponse::Created().json(commit.to_response()))
}

#[utoipa::path(
        get,
        path = "/commits/{commit_hash}",
        responses(
            (status = 200, description = "Commit found", body = CommitResponse),
            (status = 404, description = "Commit not found", body = ApiError),
        ),
    )]
#[get("/{commit_hash}")]
pub async fn get_commit(
    commit_service: web::Data<Box<dyn CommitService>>,
    commit_hash_path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let commit_hash = commit_hash_path.into_inner();
    let commit = commit_service.get_single(commit_hash)?;

    Ok(HttpResponse::Ok().json(commit.to_response()))
}

#[utoipa::path(
    patch,
    path = "/commits/{commit_hash}/processed",
    responses(
        (status = 204, description = "Commit successfully marked as processed"),
        (status = 404, description = "Commit not found", body = ApiError),
    ),
)]
#[patch("/{commit_hash}/processed")]
pub async fn patch_commit_processed(
    commit_service: web::Data<Box<dyn CommitService>>,
    commit_hash_path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let commit_hash = commit_hash_path.into_inner();
    commit_service.commit_processed(commit_hash)?;

    Ok(HttpResponse::NoContent())
}

#[utoipa::path(
        delete,
        path = "/commits/{commit_hash}",
        responses(
            (status = 204, description = "Commit successfully deleted"),
            (status = 404, description = "Commit not found", body = ApiError),
        ),
    )]
#[delete("/{commit_hash}")]
pub async fn delete_commit(
    commit_service: web::Data<Box<dyn CommitService>>,
    commit_hash_path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let commit_hash = commit_hash_path.into_inner();
    commit_service.delete(commit_hash)?;

    Ok(HttpResponse::NoContent())
}

#[utoipa::path(
        get,
        path = "/commits/{commit_hash}/conf",
        responses(
            (status = 200, description = "Commit Configuration found", body = ConfigurationResponse),
            (status = 404, description = "Commit Configuration not found", body = ApiError),
        ),
    )]
#[get("/{commit_hash}/conf")]
pub async fn get_commit_configuration(
    commit_service: web::Data<Box<dyn CommitService>>,
    commit_hash_path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let commit_hash = commit_hash_path.into_inner();
    let configuration: DbConfiguration = commit_service.get_commit_configuration(&commit_hash)?;

    Ok(HttpResponse::Ok().json(configuration.to_response().unwrap()))
}
