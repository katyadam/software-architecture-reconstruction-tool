//! Request and response types for the scraper HTTP endpoint.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Summary counts broken down by source kind.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BySource {
    /// Constants scraped from `.env*` / `*.env` files.
    pub dotenv: usize,
    /// Constants scraped from `docker-compose` files.
    pub docker_compose: usize,
}

/// Response for `POST /constants/scrape/{commit_hash}`.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ScrapeResponse {
    pub scraped: usize,
    pub files_scanned: usize,
    pub collisions: usize,
    pub by_source: BySource,
}
