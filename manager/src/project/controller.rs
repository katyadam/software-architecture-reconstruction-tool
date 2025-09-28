use actix_web::{HttpResponse, Responder, delete, get, post, web};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    errors::api::ApiError,
    project::{
        dto::{PostProject, ProjectResponse},
        model::NewProject,
        repository::{PgProjectRepository, ProjectRepository},
        service::ProjectService,
    },
};

#[utoipa::path(
        post,
        path = "/projects",
        responses(
            (status = 201, description = "Project successfully created & saved", body = ProjectResponse),
            (status = 400, description = "Project failed to create", body = ApiError),
        ),
    )]
#[post("")]
pub async fn create_project(
    project_service: web::Data<Box<dyn ProjectService>>,
    dto: web::Json<PostProject>,
) -> Result<impl Responder, ApiError> {
    let new_project = NewProject {
        project_uuid: Uuid::new_v4(),
        name: dto.name.clone(),
        owner: dto.owner.clone(),
        created_at: Utc::now(),
    };

    let project = project_service.create(new_project)?; // ? converts ServiceError -> ApiError

    Ok(HttpResponse::Created().json(project.to_response()))
}

#[utoipa::path(
        get,
        path = "/projects/{project_uuid}",
        responses(
            (status = 200, description = "Project found", body = ProjectResponse),
            (status = 404, description = "Project not found", body = ApiError),
        ),
    )]
#[get("/{project_uuid}")]
pub async fn get_project(
    project_service: web::Data<Box<dyn ProjectService>>,
    project_uuid_path: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    let projet_uuid = project_uuid_path.into_inner();
    let project = project_service.get_single(projet_uuid)?;

    Ok(HttpResponse::Ok().json(project.to_response()))
}

#[utoipa::path(
        delete,
        path = "/projects/{project_uuid}",
        responses(
            (status = 204, description = "Project successfully deleted"),
            (status = 404, description = "Project not found", body = ApiError),
        ),
    )]
#[delete("/{project_uuid}")]
pub async fn delete_project(
    project_service: web::Data<Box<dyn ProjectService>>,
    project_uuid_path: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    let projet_uuid = project_uuid_path.into_inner();
    project_service.delete(projet_uuid)?;
    Ok(HttpResponse::NoContent())
}
