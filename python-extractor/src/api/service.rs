use actix_multipart::Multipart;
use actix_web::{HttpResponse, Responder, ResponseError, Result};
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
        dto::PostFileRecord,
    },
    error::ApiError,
};

pub trait ExtractorService {
    async fn process_files(
        &self,
        payload: &mut Multipart,
        codebase_uuid: Uuid,
    ) -> Result<ServiceResponse, ApiError>;
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
            .map_err(|_| ApiError::OtherServerResponseError)?;
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

                    let code_elements_aggregate = parse(
                        text.as_str(),
                        &filename,
                        &"TODO: Service Name from Codebase Configs".to_string(),
                    )
                    .await;

                    self.synthesizer_connector
                        .send_code_elements(code_elements_aggregate, codebase_uuid)
                        .await
                        .map_err(|_| ApiError::OtherServerResponseError)?;

                    self.manager_connector
                        .send_file_record(PostFileRecord::new(
                            codebase_uuid,
                            filename.clone(),
                            file_size,
                        ))
                        .await
                        .map_err(|_| ApiError::OtherServerResponseError)?;
                    info!("Recorded File extraction in Manager.");

                    return Ok(ServiceResponse::FileProcessed);
                }
            }
        }

        Ok(ServiceResponse::NoFileFoundInRequest)
    }
}
