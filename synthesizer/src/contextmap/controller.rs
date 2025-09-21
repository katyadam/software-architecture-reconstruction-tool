use std::sync::Arc;

use crate::contextmap::{
    dto::{GetContextMapErrorReponse, PostContextMap, PostContextMapErrorResponse},
    model::ContextMap,
    repository::{self},
    service::build_context_map,
};
use actix_web::{HttpResponse, Responder, delete, get, post, web};
use neo4rs::Graph;

#[utoipa::path(
        post,
        path = "/context-maps",
        responses(
            (status = 201, description = "Context Map successfully created & saved", body = ContextMap),
            (status = 202, description = "Context Map successfully created, saving failed", body = PostContextMapErrorResponse),
        ),
    )]
#[post("")]
pub async fn create_context_map(
    graph: web::Data<Arc<Graph>>,
    dto: web::Json<PostContextMap>,
) -> impl Responder {
    let context_map = build_context_map(&dto.entities);
    match repository::save_context_map(graph, &context_map, dto.codebase_uuid.clone()).await {
        Ok(_) => HttpResponse::Created().json(context_map),
        Err(e) => HttpResponse::Accepted().json(PostContextMapErrorResponse {
            context_map: context_map,
            error: format!("{}", e),
            warning: "Context Map created but not saved.".to_string(),
        }),
    }
}

#[utoipa::path(
        get,
        path = "/context-maps/{codebase_uuid}",
        params(
            ("codebase_uuid", Path, description = "Codebase UUID of the context map to get", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")
        ),
        responses(
            (status = 200, description = "Context Map successfully retrieved", body = ContextMap),
            (status = 400, description = "Context Map cannot be retrieved", body = GetContextMapErrorReponse),
        ),
    )]
#[get("/{codebase_uuid}")]
pub async fn get_context_map(
    graph: web::Data<Arc<Graph>>,
    codebase_uuid_path: web::Path<String>,
) -> impl Responder {
    let codebase_uuid = codebase_uuid_path.into_inner();

    match repository::get_context_map(graph, codebase_uuid).await {
        Ok(context_map) => HttpResponse::Ok().json(context_map),
        Err(e) => HttpResponse::BadRequest().json(GetContextMapErrorReponse {
            error: e.to_string(),
        }),
    }
}

#[utoipa::path(
        delete,
        path = "/context-maps/{codebase_uuid}",
        params(
            ("codebase_uuid", Path, description = "Codebase UUID of the context map to delete", example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")
        ),
        responses(
            (status = 204, description = "Context Map successfully deleted"),
            (status = 400, description = "Context Map couldn't be deleted"),
        ),
    )]
#[delete("/{codebase_uuid}")]
pub async fn delete_context_map(
    graph: web::Data<Arc<Graph>>,
    codebase_uuid_path: web::Path<String>,
) -> impl Responder {
    let codebase_uuid = codebase_uuid_path.into_inner();

    match repository::delete_context_map(graph, codebase_uuid).await {
        Ok(_) => HttpResponse::NoContent(),
        Err(_) => HttpResponse::BadRequest(),
    }
}
