use actix_web::web;

pub mod connectors;
pub mod controller;
pub mod dto;
mod health;
pub mod service;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(health::health_check);
    cfg.service(controller::process_files);
}
