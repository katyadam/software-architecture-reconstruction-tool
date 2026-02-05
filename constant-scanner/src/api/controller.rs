use actix_web::{HttpResponse, Responder, post, web};

use crate::{
    api::{
        dto::{ConstantBatchInput, ConstantResponse},
        service::ConstantService,
    },
    error::ApiError,
    model::Constant,
};

#[utoipa::path(
        post,
        path = "/constants/batch",
        responses(
            (status = 201, description = "Constants successfully saved", body = Vec<ConstantResponse>),
            (status = 400, description = "Constants failed to save", body = ApiError),
        ),
    )]
#[post("batch")]
pub async fn save_constants(
    constant_service: web::Data<Box<dyn ConstantService>>,
    dto: web::Json<ConstantBatchInput>,
) -> Result<impl Responder, ApiError> {
    let constants = constant_service
        .create_batch_from_keyvalues(dto.0)?
        .iter()
        .map(Constant::to_response)
        .collect::<Vec<ConstantResponse>>();

    Ok(HttpResponse::Created().json(constants))
}
