use actix_web::{HttpResponse, Responder, get};

#[get("/health")]
async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("Configurations responding...")
}
