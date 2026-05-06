use std::collections::HashMap;
use std::path::PathBuf;

use super::helpers::fixture_base;

#[test]
fn cross_file_constant_resolution() {
    let fixture_dir = PathBuf::from(format!("{}/cross-file", fixture_base()));
    let mut external_constants = HashMap::new();
    let scraped = env_scraper::scrape(&fixture_dir);
    for (k, v) in scraped {
        external_constants.entry(k).or_insert(v);
    }

    let result = cli::get_all_code_elements(&fixture_dir, &external_constants)
        .expect("get_all_code_elements failed on cross-file fixture");

    // At least one REST call must be extracted from the fixture.
    assert!(
        !result.restcalls.is_empty(),
        "expected non-empty restcalls but got none"
    );

    // At least one REST call must have a resolved base URL. The fixture sets
    // `as_url = "http://annotation-service:8000"` in Settings, so the chain
    // `settings.as_url -> base_url -> target_uri` must produce "http://annotation-service:8000" in at least
    // one URI. This assertion fails if the propagation loop is dead code.
    assert!(
        result
            .restcalls
            .iter()
            .any(|rc| rc.target_uri.contains("http://annotation-service:8000")),
        "no REST call resolved to 'http://annotation-service:8000'; entity-import attr propagation may be broken. URIs: {:?}",
        result
            .restcalls
            .iter()
            .map(|rc| &rc.target_uri)
            .collect::<Vec<_>>()
    );

    // Every URI must be either resolved (contains "http://annotation-service:8000") or still-templated (contains "{").
    for rc in &result.restcalls {
        assert!(
            rc.target_uri.contains("http://annotation-service:8000") || rc.target_uri.contains('{'),
            "unexpected target_uri that is neither resolved nor a template: {}",
            rc.target_uri
        );
    }
}

#[test]
fn cross_file_relative_dot_import_resolution() {
    // Mirrors the empaia layout: jobs.py reaches `singletons.py` via a 4-dot
    // relative import (`from ....singletons import settings`). The
    // `settings.as_url` field is empty in source but must be filled from the
    // scraped `MDS_AS_URL` env var so that `base_url` resolves and the
    // closure's `target_uri` becomes a concrete URL.
    let fixture_dir = PathBuf::from(format!("{}/cross-file-nested", fixture_base()));
    let mut external_constants = HashMap::new();
    let scraped = env_scraper::scrape(&fixture_dir);
    for (k, v) in scraped {
        external_constants.entry(k).or_insert(v);
    }

    let result = cli::get_all_code_elements(&fixture_dir, &external_constants)
        .expect("get_all_code_elements failed on cross-file-nested fixture");

    assert!(
        !result.restcalls.is_empty(),
        "expected non-empty restcalls but got none"
    );

    assert!(
        result
            .restcalls
            .iter()
            .any(|rc| rc.target_uri.contains("http://annotation-service:8000")),
        "relative-dot import did not propagate settings.as_url -> base_url. URIs: {:?}",
        result
            .restcalls
            .iter()
            .map(|rc| &rc.target_uri)
            .collect::<Vec<_>>()
    );
}
