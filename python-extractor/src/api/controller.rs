use actix_multipart::Multipart;
use actix_web::{HttpResponse, Responder, Result, put, web};
use futures_util::StreamExt as _;
use log::info;
use python_extractor::extraction::parse::parse;
use uuid::Uuid;

use crate::{
    api::{
        connectors::{
            manager_connector::ManagerConnector, synthesizer_connector::SynthesizerConnector,
        },
        dto::{MultipleFileUploadSchema, PostFileRecord, ServiceNameQuery},
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
async fn process_files(
    manager_connector: web::Data<ManagerConnector>,
    synthesizer_connector: web::Data<SynthesizerConnector>,
    mut payload: Multipart,
    codebase_uuid_path: web::Path<Uuid>,
    query: web::Query<ServiceNameQuery>,
) -> Result<impl Responder, ApiError> {
    let codebase_uuid = codebase_uuid_path.into_inner();

    while let Some(field) = payload.next().await {
        let mut field = field.map_err(|_| ApiError::BadRequest)?;
        if let Some(content_disposition) = field.content_disposition() {
            if let Some(filename) = content_disposition.get_filename() {
                let filename = filename.to_string(); // Cloning to prevent having *field* borrowed mutable and also immutable
                info!("Uploaded file: {}", filename);

                // Collect file data into a single Vec<u8>
                let mut file_bytes = Vec::new();
                while let Some(chunk) = field.next().await {
                    let data = chunk.map_err(|_| ApiError::BadRequest)?;
                    file_bytes.extend_from_slice(&data);
                }
                info!("Uploaded file size: {} bytes", file_bytes.len());

                let file_size: i64 = file_bytes.len() as i64;
                // Convert to string (assuming UTF-8 text)
                let text = String::from_utf8(file_bytes).map_err(|_| ApiError::BadRequest)?;

                let code_elements_aggregate =
                    parse(text.as_str(), &filename, &query.service_name).await;

                synthesizer_connector
                    .send_code_elements(code_elements_aggregate, codebase_uuid)
                    .await
                    .map_err(|_| ApiError::OtherServerResponseError)?;

                manager_connector
                    .send_file_record(PostFileRecord::new(
                        codebase_uuid,
                        filename.clone(),
                        file_size,
                    ))
                    .await
                    .map_err(|_| ApiError::OtherServerResponseError)?;
                info!("Recorded File extraction in Manager.");

                return Ok(HttpResponse::Ok().body("File processed successfully"));
            }
        }
    }

    Ok(HttpResponse::BadRequest().body("No file found in request"))
}
