use actix_web::web;

pub mod controller;
pub mod dto;
pub mod health;
pub mod model;
pub mod repository;
pub mod service;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(health::health_check);
    cfg.service(controller::save_constants);
    cfg.service(controller::get_constants_by_commit_hash);
    cfg.service(controller::delete_constants_by_commit_hash);
}
