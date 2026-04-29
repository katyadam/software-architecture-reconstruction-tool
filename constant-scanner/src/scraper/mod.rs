use actix_web::web;

pub mod controller;
pub mod dto;
pub mod service;

#[cfg(test)]
mod tests;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(controller::scrape_constants);
}
