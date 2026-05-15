use models::ConfigurationData;
use std::{collections::HashMap, path::PathBuf};

use crate::e2e::helpers::fixture_base;

#[tokio::test]
async fn transitive_import_resolution() {
    // Verifies: data.py imports `settings` from api/v3/singletons.py, which
    // re-exports it from singletons.py (where `settings = Settings()` lives).
    // Settings.medical_data_service_url defaults to "http://mds", so the
    // PUT call's target URI must contain "http://mds" -- not the raw "{mds_url}".
    let fixture_dir = PathBuf::from(format!("{}/trans-import", fixture_base()));
    let mut external_constants = HashMap::new();
    let scraped = env_scraper::scrape(&fixture_dir);
    for (k, v) in scraped {
        external_constants.entry(k).or_insert(v);
    }
    let config = ConfigurationData {
        service_descriptions: vec![],
    };

    let result = cli::get_all_code_elements(&fixture_dir, &external_constants, &config, None)
        .await
        .expect("get_all_code_elements failed on trans-import fixture");

    assert!(
        !result.restcalls.is_empty(),
        "expected at least one REST call from data.py, got none"
    );

    let uris: Vec<_> = result.restcalls.iter().map(|rc| &rc.target_uri).collect();

    assert!(
        result
            .restcalls
            .iter()
            .any(|rc| rc.target_uri.contains("http://mds")),
        "transitive settings.medical_data_service_url -> mds_url not resolved; URIs: {uris:?}"
    );

    assert!(
        result
            .restcalls
            .iter()
            .all(|rc| !rc.target_uri.contains("{mds_url}")),
        "mds_url variable was not substituted in at least one URI; URIs: {uris:?}"
    );
}
