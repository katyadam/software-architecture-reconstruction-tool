use actix_multipart::{Field, Multipart};
use actix_web::{HttpResponse, Responder, Result};
use awc::body::BoxBody;
use futures_util::StreamExt as _;
use log::info;
use python_extractor::extraction::parse::parse;
use uuid::Uuid;

use crate::{
    api::{
        connectors::{
            manager_connector::ManagerConnector, synthesizer_connector::SynthesizerConnector,
        },
        dto::{PostFileRecord, ServiceDto},
    },
    error::ApiError,
};

pub trait ExtractorService {
    async fn process_files(
        &self,
        payload: &mut Multipart,
        codebase_uuid: Uuid,
    ) -> Result<ServiceResponse, ApiError>;

    async fn process_file(
        &self,
        file_name: &str,
        field: Field,
        configuration_services: &Vec<ServiceDto>,
        codebase_uuid: Uuid,
    ) -> Result<(), ApiError>;
}

pub struct ExtractorServiceImpl {
    pub manager_connector: ManagerConnector,
    pub synthesizer_connector: SynthesizerConnector,
}

impl ExtractorServiceImpl {
    pub fn new(
        manager_connector: ManagerConnector,
        synthesizer_connector: SynthesizerConnector,
    ) -> Self {
        Self {
            manager_connector,
            synthesizer_connector,
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

impl ExtractorService for ExtractorServiceImpl {
    async fn process_files(
        &self,
        payload: &mut Multipart,
        codebase_uuid: Uuid,
    ) -> Result<ServiceResponse, ApiError> {
        let configuration = self
            .manager_connector
            .get_codebase_configuration(codebase_uuid)
            .await
            .map_err(|e| ApiError::OtherServerResponseError(e.to_string()))?;
        let mut any_file_processed: bool = false;
        while let Some(field) = payload.next().await {
            let field = field.map_err(|_| ApiError::BadRequest)?;

            let file_name_opt = field
                .content_disposition()
                .and_then(|cd| cd.get_filename().map(|s| s.to_owned()));

            if let Some(file_name) = file_name_opt {
                self.process_file(
                    &file_name,
                    field,
                    &configuration.configuration_data.services,
                    codebase_uuid,
                )
                .await?;
            }
            any_file_processed = true;
        }
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
        configuration_services: &Vec<ServiceDto>,
        codebase_uuid: Uuid,
    ) -> Result<(), ApiError> {
        info!("Uploaded file: {}", file_name);

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

        let assigned_service = assign_service_name_for_file(&file_name, configuration_services);
        let code_elements_aggregate = parse(
            text,
            file_name,
            &assigned_service.unwrap_or("Can't categorize this file to a service!".to_string()),
        )
        .await;

        self.synthesizer_connector
            .send_code_elements(code_elements_aggregate, codebase_uuid)
            .await
            .map_err(|e| ApiError::OtherServerResponseError(e.to_string()))?;

        self.manager_connector
            .send_file_record(PostFileRecord::new(
                codebase_uuid,
                file_name.to_string(),
                file_size,
            ))
            .await
            .map_err(|e| ApiError::OtherServerResponseError(e.to_string()))?;
        info!("Recorded File extraction in Manager.");

        return Ok(());
    }
}

fn assign_service_name_for_file(file_name: &str, services: &Vec<ServiceDto>) -> Option<String> {
    for service in services {
        if file_name.starts_with(&service.path) {
            return Some(service.name.clone());
        }
    }
    None
}
