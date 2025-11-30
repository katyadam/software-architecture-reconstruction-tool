use actix_multipart::Multipart;
use actix_web::{Responder, Result, put, web};
use models::api::ProcessFilesIdentifier;

use crate::{
    api::{
        dto::MultipleFileUploadSchema,
        service::{ExtractorRuntimeService, ExtractorRuntimeServiceImpl},
    },
    error::ApiError,
};

#[utoipa::path(
    put,
    path = "/process-files/{codebase_uuid}/{commit_hash}",
    request_body(
        content = MultipleFileUploadSchema,
        content_type = "multipart/form-data"
    ),
    responses(
        (status = 200, description = "Files processed successfully", body = String),
        (status = 400, description = "Invalid file or request")
    )
)]
#[put("/{codebase_uuid}/{commit_hash}")]
pub async fn process_files(
    extractor_service: web::Data<ExtractorRuntimeServiceImpl>,
    mut payload: Multipart,
    process_files_identifier_path: web::Path<ProcessFilesIdentifier>,
) -> Result<impl Responder, ApiError> {
    let process_files_identifier = process_files_identifier_path.into_inner();

    extractor_service
        .process_files(&mut payload, process_files_identifier)
        .await
}
