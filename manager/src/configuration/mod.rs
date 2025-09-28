use actix_web::web;

pub mod controller;
pub mod dto;
pub mod health;
pub mod model;
pub mod repository;
pub mod service;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(health::health_check);
    cfg.service(controller::create_configuration);
    cfg.service(controller::get_configuration);
    cfg.service(controller::delete_configuration);
}
