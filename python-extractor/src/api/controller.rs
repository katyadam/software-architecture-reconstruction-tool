use actix_multipart::Multipart;
use actix_web::{Responder, Result, put, web};
use uuid::Uuid;

use crate::{
    api::{
        dto::MultipleFileUploadSchema,
        service::{ExtractorService, ExtractorServiceImpl},
    },
    error::ApiError,
};

#[utoipa::path(
    put,
    path = "/process-files/{codebase_uuid}",
    request_body(
        content = MultipleFileUploadSchema,
        content_type = "multipart/form-data"
    ),
    responses(
        (status = 200, description = "Files processed successfully", body = String),
        (status = 400, description = "Invalid file or request")
    )
)]
#[put("/{codebase_uuid}")]
pub async fn process_files(
    extractor_service: web::Data<ExtractorServiceImpl>,
    mut payload: Multipart,
    codebase_uuid_path: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    let codebase_uuid = codebase_uuid_path.into_inner();

    Ok(extractor_service
        .process_files(&mut payload, codebase_uuid)
        .await?)
}
