use actix_web::web;

pub mod controller;
pub mod dto;
pub mod health;
pub mod model;
pub mod repository;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(health::health_check);
    cfg.service(controller::add_record);
}
