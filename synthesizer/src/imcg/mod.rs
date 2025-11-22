use actix_web::web;

pub mod construction;
pub mod controller;
pub mod dto;
pub mod health;
pub mod model;
pub mod queries;
pub mod repository;
pub mod service;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(health::health_check);
    cfg.service(controller::create_imcg);
    cfg.service(controller::get_imcg);
    cfg.service(controller::delete_imcg);
}
