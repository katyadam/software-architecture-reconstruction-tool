use std::collections::HashMap;

use actix_multipart::{Field, Multipart};
use actix_web::{HttpResponse, Responder, Result};
use awc::body::BoxBody;
use futures_util::StreamExt as _;
use log::info;
use models::{api::ProcessFilesIdentifier, ir::evaluted::EvaluatedIR};
use uuid::Uuid;

use crate::{
    api::{
        connectors::{
            manager_connector::ManagerConnector, s3_connector::S3Connector,
            synthesizer_connector::SynthesizerConnector,
        },
        dto::PostFileRecord,
    },
    error::ApiError,
    pipeline,
};

pub trait ExtractorRuntimeService {
    async fn process_files(
        &self,
        payload: &mut Multipart,
        process_files_identifier: ProcessFilesIdentifier,
    ) -> Result<ServiceResponse, ApiError>;
}

pub struct ExtractorRuntimeServiceImpl {
    pub manager_connector: ManagerConnector,
    pub synthesizer_connector: SynthesizerConnector,
    pub s3_connector: S3Connector,
}

impl ExtractorRuntimeServiceImpl {
    pub fn new(
        manager_connector: ManagerConnector,
        synthesizer_connector: SynthesizerConnector,
        s3_connector: S3Connector,
    ) -> Self {
        Self {
            manager_connector,
            synthesizer_connector,
            s3_connector,
        }
    }
}

pub enum ServiceResponse {
    FileProcessed,
    NoFileFoundInRequest,
}

impl Responder for ServiceResponse {
    type Body = BoxBody;

    fn respond_to(self, _req: &actix_web::HttpRequest) -> HttpResponse<Self::Body> {
        match self {
            ServiceResponse::FileProcessed => {
                HttpResponse::Ok().body("file processed successfully")
            }
            ServiceResponse::NoFileFoundInRequest => {
                HttpResponse::BadRequest().body("no file found in request")
            }
        }
    }
}

impl ExtractorRuntimeService for ExtractorRuntimeServiceImpl {
    async fn process_files(
        &self,
        payload: &mut Multipart,
        identifier: ProcessFilesIdentifier,
    ) -> Result<ServiceResponse, ApiError> {
        let run_id = Uuid::new_v4();
        let base_dir_path = format!("{}/{run_id}", identifier.codebase_uuid);

        let collected = collect_uploaded_files(payload).await?;
        if collected.is_empty() {
            return Ok(ServiceResponse::NoFileFoundInRequest);
        }

        let evaluated_ir = run_extraction_pipeline(&collected);

        self.s3_connector
            .store_evaluated_ir(evaluated_ir, &base_dir_path)
            .await?;

        self.notify_services(identifier, &collected, &base_dir_path)
            .await?;

        Ok(ServiceResponse::FileProcessed)
    }
}

/// Drain a multipart payload into an in-memory list of `(name, text, size)` tuples.
/// Binary files (non-UTF-8) are skipped with a log message.
async fn collect_uploaded_files(
    payload: &mut Multipart,
) -> Result<Vec<(String, String, i64)>, ApiError> {
    let mut collected: Vec<(String, String, i64)> = Vec::new();

    while let Some(field) = payload.next().await {
        let mut field: Field =
            field.map_err(|_| ApiError::BadRequest("Could not get Payload Field".to_string()))?;

        let Some(file_name) = field
            .content_disposition()
            .and_then(|cd| cd.get_filename().map(|s| s.to_owned()))
        else {
            continue;
        };

        let mut file_bytes: Vec<u8> = Vec::new();
        while let Some(chunk) = field.next().await {
            let data = chunk
                .map_err(|_| ApiError::BadRequest("Cannot get file data from chunk".to_string()))?;
            file_bytes.extend_from_slice(&data);
        }
        info!("Uploaded file: {file_name} ({} bytes)", file_bytes.len());

        match std::str::from_utf8(&file_bytes) {
            Ok(text) => collected.push((file_name, text.to_string(), file_bytes.len() as i64)),
            Err(_) => info!("Skipping non-UTF-8 file: {file_name}"),
        }
    }

    Ok(collected)
}

/// Run the 3-pass extraction pipeline over the collected files.
/// Pass 1: syntactic extraction per file.
/// Pass 2: cross-file type resolution.
/// Pass 3: symbolic evaluation and URI resolution.
fn run_extraction_pipeline(collected: &[(String, String, i64)]) -> EvaluatedIR {
    let file_records: Vec<_> = collected
        .iter()
        .filter_map(
            |(name, text, _)| match pipeline::dispatch_syntactic(text, name) {
                Ok(Some(record)) => Some(record),
                Ok(None) => None,
                Err(e) => {
                    info!("Pass 1 error for {name}: {e}");
                    None
                }
            },
        )
        .collect();

    let project_ir = pipeline::build_project_ir(file_records);
    // TODO: Take external constants that are saved in db or supplied as JSON and put them here
    pipeline::evaluate(project_ir, &HashMap::new())
}

impl ExtractorRuntimeServiceImpl {
    /// Notify the manager (file records) and synthesizer (load trigger) after S3 storage.
    async fn notify_services(
        &self,
        identifier: ProcessFilesIdentifier,
        collected: &[(String, String, i64)],
        base_dir_path: &str,
    ) -> Result<(), ApiError> {
        for (file_name, _, file_size) in collected {
            if let Err(e) = self
                .manager_connector
                .send_file_record(PostFileRecord::new(
                    identifier.codebase_uuid,
                    file_name.clone(),
                    *file_size,
                ))
                .await
            {
                info!("Manager file record error for {file_name}: {e}");
            }
        }

        self.synthesizer_connector
            .send_load_info(identifier, base_dir_path)
            .await?;

        Ok(())
    }
}
