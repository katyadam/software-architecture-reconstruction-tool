use actix_web::{HttpResponse, Responder, delete, get, post, web};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    configuration::{
        dto::{ConfigurationResponse, PostConfiguration},
        model::NewDbConfiguration,
        service::ConfigurationService,
    },
    errors::api::ApiError,
};

#[utoipa::path(
        post,
        path = "/configurations",
        responses(
            (status = 201, description = "Configuration successfully created & saved", body = ConfigurationResponse),
            (status = 400, description = "Configuration failed to create", body = ApiError),
        ),
    )]
#[post("")]
pub async fn create_configuration(
    configuration_service: web::Data<Box<dyn ConfigurationService>>,
    dto: web::Json<PostConfiguration>,
) -> Result<impl Responder, ApiError> {
    let new_conf = NewDbConfiguration {
        configuration_uuid: Uuid::new_v4(),
        configuration_data: serde_json::json!(dto.configuration_data),
        created_at: Utc::now(),
    };

    let configuration = configuration_service.create(new_conf)?;

    Ok(HttpResponse::Created().json(configuration.to_response()))
}

#[utoipa::path(
        get,
        path = "/configurations/{configuration_uuid}",
        responses(
            (status = 200, description = "Configuration found", body = ConfigurationResponse),
            (status = 404, description = "Configuration not found", body = ApiError),
        ),
    )]
#[get("/{configuration_uuid}")]
pub async fn get_configuration(
    configuration_service: web::Data<Box<dyn ConfigurationService>>,
    configuration_uuid_path: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    let configuration_uuid = configuration_uuid_path.into_inner();
    let configuration = configuration_service.get_single(configuration_uuid)?;

    Ok(HttpResponse::Ok().json(configuration.to_response()))
}

#[utoipa::path(
        delete,
        path = "/configurations/{configuration_uuid}",
        responses(
            (status = 204, description = "Configuration successfully deleted"),
            (status = 404, description = "Configuration not found", body = ApiError),
        ),
    )]
#[delete("/{configuration_uuid}")]
pub async fn delete_configuration(
    configuration_service: web::Data<Box<dyn ConfigurationService>>,
    configuration_uuid_path: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    let configuration_uuid = configuration_uuid_path.into_inner();
    configuration_service.delete(configuration_uuid)?;
    Ok(HttpResponse::NoContent())
}
