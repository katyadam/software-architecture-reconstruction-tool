use actix_web::web;

pub mod builder;
pub mod controller;
pub mod dto;
pub mod health;
pub mod model;
pub mod queries;
pub(crate) mod repository;
pub(crate) mod service;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(health::health_check);
    cfg.service(controller::create_context_map);
    cfg.service(controller::get_context_map);
    cfg.service(controller::delete_context_map);
}
