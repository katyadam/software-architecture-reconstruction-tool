//! Scraper service: parse -> merge -> persist.

use log::info;

use env_scraper::merge::MergeResult;
use env_scraper::parsers::{EnvEntry, SourceKind, parse_from_memory};

use crate::constant::service::ConstantService;
use crate::error::ServiceError;

use super::dto::{BySource, ScrapeResponse};

pub fn scrape_from_files(
    commit_hash: String,
    files: Vec<(String, String)>,
    constant_service: &dyn ConstantService,
) -> Result<ScrapeResponse, ServiceError> {
    if commit_hash.is_empty() {
        return Err(ServiceError::ValidationError(
            "commit_hash must not be empty".into(),
        ));
    }

    let files_scanned = files.len();
    let entries = parse(&files);
    let merged = env_scraper::merge::merge(entries);
    let response = persist(commit_hash, files_scanned, merged, constant_service)?;
    Ok(response)
}

fn parse(files: &[(String, String)]) -> Vec<EnvEntry> {
    let pairs: Vec<(&str, &str)> = files
        .iter()
        .map(|(n, c)| (n.as_str(), c.as_str()))
        .collect();
    parse_from_memory(&pairs)
}

fn persist(
    commit_hash: String,
    files_scanned: usize,
    merged: MergeResult,
    constant_service: &dyn ConstantService,
) -> Result<ScrapeResponse, ServiceError> {
    let collisions = merged.collisions;
    let by_source = constants_by_source(&merged.entries);
    let scraped = merged.entries.len();

    let persist_entries: Vec<(String, String, String)> = merged
        .entries
        .into_iter()
        .map(|e| {
            let tag = format!("scraper:{}:{}", e.kind.label(), e.source_path);
            (e.name, e.value, tag)
        })
        .collect();

    if !persist_entries.is_empty() {
        constant_service.create_batch_with_source(commit_hash.clone(), persist_entries)?;
    }

    info!(
        "scraper: persisted {} constants for commit '{}' ({} collisions, {} files)",
        scraped, commit_hash, collisions, files_scanned
    );

    Ok(ScrapeResponse {
        scraped,
        files_scanned,
        collisions,
        by_source,
    })
}

fn constants_by_source(entries: &[EnvEntry]) -> BySource {
    let mut dotenv = 0usize;
    let mut docker_compose = 0usize;
    for e in entries {
        match e.kind {
            SourceKind::DotEnv => dotenv += 1,
            SourceKind::DockerCompose => docker_compose += 1,
        }
    }
    BySource {
        dotenv,
        docker_compose,
    }
}
