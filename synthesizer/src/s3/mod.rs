use actix_web::web;

pub mod client;
pub mod controller;
pub mod dto;
pub mod health;
mod model;
pub mod service;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(health::health_check);
}
