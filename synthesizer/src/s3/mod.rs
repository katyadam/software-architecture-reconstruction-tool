use actix_web::web;

pub mod client;
pub mod controller;
pub mod health;
pub mod service;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(health::health_check);
}
