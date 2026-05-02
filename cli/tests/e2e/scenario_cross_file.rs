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
