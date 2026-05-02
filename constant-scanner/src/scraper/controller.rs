//! HTTP handler for `POST /constants/scrape/{commit_hash}`.

use actix_multipart::{Field, Multipart};
use actix_web::{HttpResponse, post, web};
use futures_util::StreamExt as _;
use log::info;
use models::api::MultipleFileUploadSchema;

use crate::{
    constant::service::ConstantService,
    error::ApiError,
    scraper::{dto::ScrapeResponse, service::scrape_from_files},
};

#[utoipa::path(
    post,
    path = "/constants/scrape/{commit_hash}",
    request_body(
        content = MultipleFileUploadSchema,
        content_type = "multipart/form-data"
    ),
    responses(
        (status = 200, description = "Scrape completed successfully", body = ScrapeResponse),
        (status = 400, description = "Invalid request or no files provided"),
        (status = 500, description = "Internal error during scrape"),
    ),
    params(
        ("commit_hash" = String, Path, description = "Commit hash to associate scraped constants with"),
    )
)]
#[post("/scrape/{commit_hash}")]
pub async fn scrape_constants(
    constant_service: web::Data<Box<dyn ConstantService>>,
    path: web::Path<String>,
    mut payload: Multipart,
) -> Result<HttpResponse, ApiError> {
    let commit_hash = path.into_inner();

    if commit_hash.is_empty() {
        return Err(ApiError::BadRequest);
    }

    let files = collect_uploaded_files(&mut payload).await?;

    if files.is_empty() {
        return Ok(HttpResponse::BadRequest().body("no files found in request"));
    }

    let svc = constant_service.clone();
    let response: ScrapeResponse =
        web::block(move || scrape_from_files(commit_hash, files, svc.as_ref().as_ref()))
            .await
            .map_err(|_| ApiError::InternalServerError)??;

    Ok(HttpResponse::Ok().json(response))
}

/// Drain a multipart stream into `(filename, content)` pairs.
/// Non-UTF-8 files and fields without a filename are skipped.
async fn collect_uploaded_files(
    payload: &mut Multipart,
) -> Result<Vec<(String, String)>, ApiError> {
    let mut collected = Vec::new();

    while let Some(field) = payload.next().await {
        let mut field: Field = field.map_err(|_| ApiError::BadRequest)?;

        let Some(file_name) = field
            .content_disposition()
            .and_then(|cd| cd.get_filename().map(|s| s.to_owned()))
        else {
            continue;
        };

        let mut bytes: Vec<u8> = Vec::new();
        while let Some(chunk) = field.next().await {
            let data = chunk.map_err(|_| ApiError::BadRequest)?;
            bytes.extend_from_slice(&data);
        }

        match std::str::from_utf8(&bytes) {
            Ok(text) => {
                info!(
                    "scraper: received file '{}' ({} bytes)",
                    file_name,
                    bytes.len()
                );
                collected.push((file_name, text.to_owned()));
            }
            Err(_) => info!("scraper: skipping non-UTF-8 file '{}'", file_name),
        }
    }

    Ok(collected)
}
