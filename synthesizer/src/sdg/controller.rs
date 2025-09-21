use std::sync::Arc;

use actix_web::{HttpResponse, Responder, delete, get, post, web};
use neo4rs::Graph;

use crate::{
    contextmap::dto::GetContextMapErrorReponse,
    sdg::{
        dto::{GetSDGErrorReponse, PostSDG, PostSDGErrorResponse},
        model::SDG,
        repository,
        service::build_sdg,
    },
};

#[utoipa::path(
        post,
        path = "/sdgs",
        responses(
            (status = 201, description = "SDG successfully created & saved", body = SDG),
            (status = 202, description = "SDG successfully created, saving failed", body = PostSDGErrorResponse),
        ),
    )]
#[post("")]
pub async fn create_sdg(graph: web::Data<Arc<Graph>>, dto: web::Json<PostSDG>) -> impl Responder {
    let dto = dto.into_inner();
    let sdg = build_sdg(dto.endpoints, dto.restcalls);
    match repository::save_sdg(graph, &sdg, dto.codebase_uuid.clone()).await {
        Ok(_) => HttpResponse::Created().json(sdg),
        Err(e) => HttpResponse::Accepted().json(PostSDGErrorResponse {
            sdg,
            error: format!("{}", e),
            warning: "SDG created but not saved.".to_string(),
        }),
    }
}

#[utoipa::path(
        get,
        path = "/sdgs/{codebase_uuid}",
        params(
            ("codebase_uuid", Path, description = "Codebase UUID of the SDG to get", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")
        ),
        responses(
            (status = 200, description = "SDG successfully retrieved", body = SDG),
            (status = 400, description = "SDG cannot be retrieved", body = GetSDGErrorReponse),
        ),
    )]
#[get("/{codebase_uuid}")]
pub async fn get_sdg(
    graph: web::Data<Arc<Graph>>,
    codebase_uuid_path: web::Path<String>,
) -> impl Responder {
    let codebase_uuid = codebase_uuid_path.into_inner();

    match repository::get_sdg(graph, codebase_uuid).await {
        Ok(context_map) => HttpResponse::Ok().json(context_map),
        Err(e) => HttpResponse::BadRequest().json(GetContextMapErrorReponse {
            error: e.to_string(),
        }),
    }
}

#[utoipa::path(
        delete,
        path = "/sdgs/{codebase_uuid}",
        params(
            ("codebase_uuid", Path, description = "Codebase UUID of the SDG to delete", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")
        ),
        responses(
            (status = 204, description = "Service Dependency Graph successfully deleted"),
            (status = 400, description = "Service Dependency Graph couldn't be deleted"),
        ),
    )]
#[delete("/{codebase_uuid}")]
pub async fn delete_sdg(
    graph: web::Data<Arc<Graph>>,
    codebase_uuid_path: web::Path<String>,
) -> impl Responder {
    let codebase_uuid = codebase_uuid_path.into_inner();

    match repository::delete_sdg(graph, codebase_uuid).await {
        Ok(_) => HttpResponse::NoContent(),
        Err(_) => HttpResponse::BadRequest(),
    }
}
