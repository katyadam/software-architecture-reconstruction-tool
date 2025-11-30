use actix_multipart::{Field, Multipart};
use actix_web::{HttpResponse, Responder, Result};
use awc::body::BoxBody;
use futures_util::StreamExt as _;
use log::info;
use models::api::ProcessFilesIdentifier;
use python_extractor::extraction::parse::parse;
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
};

pub trait ExtractorRuntimeService {
    async fn process_files(
        &self,
        payload: &mut Multipart,
        process_files_identifier: ProcessFilesIdentifier,
    ) -> Result<ServiceResponse, ApiError>;

    async fn process_file(
        &self,
        file_name: &str,
        field: Field,
        codebase_uuid: Uuid,
        base_dir_path: &str,
    ) -> Result<(), ApiError>;
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
        let run_id = uuid::Uuid::new_v4();
        let base_dir_path = format!("{}/{run_id}", identifier.codebase_uuid);
        let mut any_file_processed: bool = false;

        while let Some(field) = payload.next().await {
            let field = field.map_err(|_| ApiError::BadRequest)?;

            let file_name_opt = field
                .content_disposition()
                .and_then(|cd| cd.get_filename().map(|s| s.to_owned()));

            if let Some(file_name) = file_name_opt {
                self.process_file(&file_name, field, identifier.codebase_uuid, &base_dir_path)
                    .await?;
            }
            any_file_processed = true;
        }

        self.synthesizer_connector
            .send_load_info(identifier, &base_dir_path)
            .await?;

        if any_file_processed {
            Ok(ServiceResponse::FileProcessed)
        } else {
            Ok(ServiceResponse::NoFileFoundInRequest)
        }
    }

    async fn process_file(
        &self,
        file_name: &str,
        mut field: Field,
        codebase_uuid: Uuid,
        base_dir_path: &str,
    ) -> Result<(), ApiError> {
        info!("Uploaded file: {file_name}");

        // Collect file data into a single Vec<u8>
        let mut file_bytes = Vec::new();
        while let Some(chunk) = field.next().await {
            let data = chunk.map_err(|_| ApiError::BadRequest)?;
            file_bytes.extend_from_slice(&data);
        }
        info!("Uploaded file size: {} bytes", file_bytes.len());

        let file_size: i64 = file_bytes.len() as i64;
        // Convert to string (assuming UTF-8 text)
        let text = std::str::from_utf8(&file_bytes)
            .map_err(|_| ApiError::InternalServerError)
            .unwrap();

        let code_elements_aggregate = parse(text, file_name).await;
        self.s3_connector
            .store_code_elements(code_elements_aggregate, base_dir_path)
            .await?;

        self.manager_connector
            .send_file_record(PostFileRecord::new(
                codebase_uuid,
                file_name.to_string(),
                file_size,
            ))
            .await?;
        info!("Recorded File extraction in Manager.");

        Ok(())
    }
}

// fn assign_service_description_to_file(
//     file_name: &str,
//     service_descs: Vec<ServiceDescription>,
// ) -> ServiceDescription {
//     service_descs
//         .into_iter()
//         .find(|sd| file_name.starts_with(&sd.base_dir_path))
//         .unwrap_or_default()
// }
