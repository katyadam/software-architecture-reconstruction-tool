//! `env-scraper` — pure business logic for discovering and parsing environment
//! variable definitions from `.env*` and `docker-compose*.{yml,yaml}` files.

pub mod discovery;
pub mod merge;
pub mod parsers;

use std::collections::HashMap;
use std::path::Path;

/// Scrape all env vars from `.env*` and `docker-compose*.{yml,yaml}` files
/// found under `root`. Returns a merged flat map of KEY -> value.
pub fn scrape(root: &Path) -> HashMap<String, String> {
    let discovered = discovery::discover(
        root,
        &[
            parsers::SourceKind::DotEnv,
            parsers::SourceKind::DockerCompose,
        ],
        discovery::DEFAULT_MAX_DEPTH,
    );
    let entries = parsers::parse_all(&discovered);
    let merged = merge::merge(entries);
    merged
        .entries
        .into_iter()
        .map(|e| (e.name, e.value))
        .collect()
}
