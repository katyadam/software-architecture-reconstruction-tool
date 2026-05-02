//! End-to-end tests against the fixture tree in `tests/fixtures/`.
//!
//! Fixture layout:
//! ```
//! tests/fixtures/
//!   medical-data-service/sample.env   # MDS_ES_URL=http://examination-service:8000, ...
//!   docker-compose.yml                 # services.mds.environment: { MDS_ES_URL: http://override }
//! ```
//!
//! Expected: sample.env value (priority 60) beats docker-compose (priority 40).

use std::path::Path;

use env_scraper::discovery::{self, DEFAULT_MAX_DEPTH};
use env_scraper::merge;
use env_scraper::parsers::docker_compose::DockerComposeParser;
use env_scraper::parsers::dotenv::DotEnvParser;
use env_scraper::parsers::{Parser, SourceKind};

fn fixture_root() -> std::path::PathBuf {
    // Resolve relative to CARGO_MANIFEST_DIR so tests work from any cwd.
    let manifest = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest).join("src/scraper/tests/fixtures")
}

#[test]
fn discovered_files_include_sample_env_and_compose() {
    let root = fixture_root();
    let files = discovery::discover(&root, &[], DEFAULT_MAX_DEPTH);

    let kinds: Vec<SourceKind> = files.iter().map(|f| f.kind).collect();
    assert!(
        kinds.contains(&SourceKind::DotEnv),
        "should find sample.env"
    );
    assert!(
        kinds.contains(&SourceKind::DockerCompose),
        "should find docker-compose.yml"
    );
}

#[test]
fn sample_env_value_beats_docker_compose() {
    let root = fixture_root();
    let files = discovery::discover(&root, &[], DEFAULT_MAX_DEPTH);

    let mut all_entries = Vec::new();
    for file in &files {
        let content = std::fs::read_to_string(&file.path).expect("fixture readable");
        let entries = match file.kind {
            SourceKind::DotEnv => {
                let prio = discovery::dotenv_priority(&file.path);
                DotEnvParser::new(prio).parse(&content, &file.path)
            }
            SourceKind::DockerCompose => DockerComposeParser.parse(&content, &file.path),
        };
        all_entries.extend(entries);
    }

    let merged = merge::merge(all_entries);

    let es_url = merged
        .entries
        .iter()
        .find(|e| e.name == "MDS_ES_URL")
        .expect("MDS_ES_URL present");

    assert_eq!(
        es_url.value, "http://examination-service:8000",
        "sample.env value should win over docker-compose override"
    );
}

#[test]
fn placeholder_var_dropped() {
    let root = fixture_root();
    let files = discovery::discover(&root, &[], DEFAULT_MAX_DEPTH);

    let mut all_entries = Vec::new();
    for file in &files {
        let content = std::fs::read_to_string(&file.path).expect("fixture readable");
        let entries = match file.kind {
            SourceKind::DotEnv => {
                let prio = discovery::dotenv_priority(&file.path);
                DotEnvParser::new(prio).parse(&content, &file.path)
            }
            SourceKind::DockerCompose => DockerComposeParser.parse(&content, &file.path),
        };
        all_entries.extend(entries);
    }

    let merged = merge::merge(all_entries);

    let placeholder = merged.entries.iter().find(|e| e.name == "PLACEHOLDER_VAR");
    assert!(
        placeholder.is_none(),
        "PLACEHOLDER_VAR (${{}}-style) should be dropped by merge"
    );
}

#[test]
fn docker_compose_only_key_is_present() {
    // MDS_DB_URL is only in docker-compose, not in sample.env -> must appear.
    let root = fixture_root();
    let files = discovery::discover(&root, &[], DEFAULT_MAX_DEPTH);

    let mut all_entries = Vec::new();
    for file in &files {
        let content = std::fs::read_to_string(&file.path).expect("fixture readable");
        let entries = match file.kind {
            SourceKind::DotEnv => {
                let prio = discovery::dotenv_priority(&file.path);
                DotEnvParser::new(prio).parse(&content, &file.path)
            }
            SourceKind::DockerCompose => DockerComposeParser.parse(&content, &file.path),
        };
        all_entries.extend(entries);
    }

    let merged = merge::merge(all_entries);

    let db_url = merged.entries.iter().find(|e| e.name == "MDS_DB_URL");
    assert!(
        db_url.is_some(),
        "MDS_DB_URL should be present from docker-compose"
    );
}
