use actix_web::{HttpResponse, Responder, delete, get, post, web};
use chrono::Utc;

use crate::{
    commit::{
        dto::{CommitResponse, PostCommit},
        model::NewCommit,
        service::CommitService,
    },
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
