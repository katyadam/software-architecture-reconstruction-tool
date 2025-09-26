use crate::files::{
    dto::PostFileRecord,
    model::NewFileRecord,
    repository::{FileRecordsRepository, PgFileRecordsRepository},
};

use actix_web::{HttpResponse, Responder, post, web};
use chrono::Utc;

use crate::errors::api::ApiError;

#[utoipa::path(
        post,
        path = "/file-records",
        responses(
            (status = 201, description = "File Record successfully saved"),
            (status = 400, description = "File Record failed to save", body = ApiError),
        ),
    )]
#[post("")]
pub async fn add_record(
    file_records_repo: web::Data<PgFileRecordsRepository>,
    dto: web::Json<PostFileRecord>,
) -> Result<impl Responder, ApiError> {
    let new_file_record = NewFileRecord {
        codebase_uuid: dto.codebase_uuid,
        file_path: dto.file_path.clone(),
        file_size: dto.file_size,
        uploaded_at: Utc::now(),
    };

    file_records_repo.save(new_file_record)?; // ? converts DatabaseError -> ApiError

    Ok(HttpResponse::Created())
}
