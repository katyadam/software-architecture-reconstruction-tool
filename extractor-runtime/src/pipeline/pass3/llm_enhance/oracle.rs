//! Auto-derived ground-truth oracle for scoring the service matcher (S0.2).
//!
//! Ground truth is NOT hand-supplied. It is derived by joining a curated
//! constants file (identifier -> URL) with the config file (URL -> service):
//!
//! ```text
//! constants: mds_url -> http://medical-data-service:5000
//! config:    http://medical-data-service:8000 -> medical-data-service
//! join on HOST (medical-data-service) -> oracle edge: mds_url -> medical-data-service
//! ```
//!
//! The join is on **host**, not the full URL, so a port mismatch between the
//! constants and config (expected) is tolerated. This is not circular: the
//! classifier under test resolves from names/classes/imports, never from the
//! constants' URL values. The oracle is deliberately partial -- identifiers
//! whose host maps to zero or multiple services are dropped.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;
use models::ConfigurationData;
use serde::Deserialize;

/// One curated constant: an identifier and the URL it resolves to.
#[derive(Debug, Deserialize)]
pub(super) struct OracleConstant {
    pub name: String,
    pub value: String,
}

/// The constants file shape (`{ commit_hash, constants: [...] }`).
#[derive(Debug, Deserialize)]
struct ConstantsFile {
    constants: Vec<OracleConstant>,
}

/// Maps a normalized identifier to the service it provably targets.
pub(super) struct ServiceOracle {
    edges: HashMap<String, String>,
    /// Count of constants dropped (host unknown / ambiguous in config).
    dropped: usize,
}

/// Extract the bare host from a URL: strip scheme, `:port`, and any path.
/// e.g. `http://medical-data-service:5000/v1` -> `medical-data-service`.
fn host_of(url: &str) -> Option<String> {
    let after_scheme = url.split("://").last().unwrap_or(url);
    let host_port = after_scheme.split('/').next().unwrap_or(after_scheme);
    let host = host_port.split(':').next().unwrap_or(host_port);
    let host = host.trim();
    (!host.is_empty()).then(|| host.to_string())
}

/// Map a (possibly rewritten) URL back to the config service it now targets:
/// return the name of the service any of whose `urls` shares the same host as
/// `url`. `None` when `url` has no host or no service matches. Used to turn a
/// final `target_uri` into the service it points at for scoring.
pub(super) fn service_for_url(url: &str, config: &ConfigurationData) -> Option<String> {
    let host = host_of(url)?;
    config
        .service_descriptions
        .iter()
        .find(|svc| svc.urls.iter().any(|u| host_of(u).as_deref() == Some(&host)))
        .map(|svc| svc.name.clone())
}

/// Normalize an identifier to its oracle/scorer join key: keep the last dotted
/// segment (drops `settings.` / `self.` prefixes), strip leading underscores,
/// lowercase. e.g. `settings.mps_url` -> `mps_url`, `_mds_url` -> `mds_url`.
pub(super) fn normalize(identifier: &str) -> String {
    identifier
        .rsplit('.')
        .next()
        .unwrap_or(identifier)
        .trim_start_matches('_')
        .to_lowercase()
}

impl ServiceOracle {
    /// Build the oracle from already-parsed parts. Used by both [`load`] and
    /// the unit tests (so tests need no files on disk).
    pub(super) fn from_parts(constants: &[OracleConstant], config: &ConfigurationData) -> Self {
        // host -> set of service names (a host should map to one service).
        let mut host_to_services: HashMap<String, Vec<String>> = HashMap::new();
        for svc in &config.service_descriptions {
            for url in &svc.urls {
                if let Some(host) = host_of(url) {
                    let services = host_to_services.entry(host).or_default();
                    if !services.contains(&svc.name) {
                        services.push(svc.name.clone());
                    }
                }
            }
        }

        let mut edges: HashMap<String, String> = HashMap::new();
        let mut dropped = 0;
        for c in constants {
            let Some(host) = host_of(&c.value) else {
                dropped += 1;
                continue;
            };
            match host_to_services.get(&host) {
                Some(services) if services.len() == 1 => {
                    edges.insert(normalize(&c.name), services[0].clone());
                }
                // host unknown in config, or ambiguous -> drop (partial oracle).
                _ => dropped += 1,
            }
        }

        ServiceOracle { edges, dropped }
    }

    /// Load the oracle from a constants file and a config file on disk.
    pub(super) fn load(
        constants_path: impl AsRef<Path>,
        config_path: impl AsRef<Path>,
    ) -> anyhow::Result<Self> {
        let constants_path = constants_path.as_ref();
        let config_path = config_path.as_ref();
        let constants_raw = std::fs::read_to_string(constants_path)
            .with_context(|| format!("reading constants {}", constants_path.display()))?;
        let config_raw = std::fs::read_to_string(config_path)
            .with_context(|| format!("reading config {}", config_path.display()))?;
        let constants: ConstantsFile = serde_json::from_str(&constants_raw)
            .with_context(|| format!("parsing constants {}", constants_path.display()))?;
        let config: ConfigurationData = serde_json::from_str(&config_raw)
            .with_context(|| format!("parsing config {}", config_path.display()))?;
        Ok(Self::from_parts(&constants.constants, &config))
    }

    /// Load the oracle from a constants file on disk plus an in-memory config.
    /// Unlike [`load`], the config is already held by the caller, so only the
    /// constants file is read and parsed here.
    pub(super) fn from_constants_file(
        constants_path: impl AsRef<Path>,
        config: &ConfigurationData,
    ) -> anyhow::Result<Self> {
        let constants_path = constants_path.as_ref();
        let constants_raw = std::fs::read_to_string(constants_path)
            .with_context(|| format!("reading constants {}", constants_path.display()))?;
        let file: ConstantsFile = serde_json::from_str(&constants_raw)
            .with_context(|| format!("parsing constants {}", constants_path.display()))?;
        Ok(Self::from_parts(&file.constants, config))
    }

    /// The expected service for a set of operand identifiers: normalize each,
    /// look each up, and return the service only if all that match agree on a
    /// single one. Returns `None` when none match or when they conflict
    /// (unscoreable).
    pub(super) fn expected_service(&self, identifiers: &[String]) -> Option<&str> {
        let mut found: Option<&str> = None;
        for ident in identifiers {
            if let Some(service) = self.edges.get(&normalize(ident)) {
                match found {
                    None => found = Some(service),
                    Some(prev) if prev == service => {}
                    Some(_) => return None, // conflicting -> unscoreable
                }
            }
        }
        found
    }

    /// Number of oracle edges (the scoreable universe).
    pub(super) fn len(&self) -> usize {
        self.edges.len()
    }

    /// Number of constants dropped during the join (host unknown / ambiguous).
    pub(super) fn dropped(&self) -> usize {
        self.dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::configuration::ServiceDescription;

    fn manifest_relative(rel: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
    }

    fn svc(name: &str, urls: &[&str]) -> ServiceDescription {
        ServiceDescription {
            name: name.to_string(),
            base_dir_path: format!("/proj/{name}"),
            urls: urls.iter().map(|u| u.to_string()).collect(),
        }
    }

    fn constant(name: &str, value: &str) -> OracleConstant {
        OracleConstant {
            name: name.to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn host_extraction() {
        assert_eq!(
            host_of("http://medical-data-service:5000/v1"),
            Some("medical-data-service".to_string())
        );
        assert_eq!(
            host_of("http://event-service"),
            Some("event-service".to_string())
        );
        assert_eq!(host_of(""), None);
    }

    #[test]
    fn normalize_strips_prefix_and_underscores() {
        assert_eq!(normalize("settings.mps_url"), "mps_url");
        assert_eq!(normalize("_mds_url"), "mds_url");
        assert_eq!(normalize("self._cds_url"), "cds_url");
        assert_eq!(normalize("ES_URL"), "es_url");
    }

    #[test]
    fn port_mismatch_tolerated() {
        // constants port 5000, config port 8000 -> still joins on host.
        let config = ConfigurationData {
            service_descriptions: vec![svc(
                "medical-data-service",
                &["http://medical-data-service:8000"],
            )],
        };
        let constants = vec![constant("mds_url", "http://medical-data-service:5000")];
        let oracle = ServiceOracle::from_parts(&constants, &config);
        assert_eq!(
            oracle.expected_service(&["mds_url".to_string()]),
            Some("medical-data-service")
        );
        assert_eq!(oracle.dropped(), 0);
    }

    #[test]
    fn ambiguous_host_dropped() {
        // Two services share a host -> any constant on that host is dropped.
        let config = ConfigurationData {
            service_descriptions: vec![
                svc("svc-a", &["http://shared-host:8000"]),
                svc("svc-b", &["http://shared-host:9000"]),
            ],
        };
        let constants = vec![constant("x_url", "http://shared-host:1000")];
        let oracle = ServiceOracle::from_parts(&constants, &config);
        assert_eq!(oracle.len(), 0);
        assert_eq!(oracle.dropped(), 1);
        assert_eq!(oracle.expected_service(&["x_url".to_string()]), None);
    }

    #[test]
    fn unknown_host_dropped() {
        let config = ConfigurationData {
            service_descriptions: vec![svc("svc-a", &["http://known:8000"])],
        };
        let constants = vec![constant("x_url", "http://unknown:8000")];
        let oracle = ServiceOracle::from_parts(&constants, &config);
        assert_eq!(oracle.len(), 0);
        assert_eq!(oracle.dropped(), 1);
    }

    #[test]
    fn conflicting_identifiers_yield_none() {
        let config = ConfigurationData {
            service_descriptions: vec![
                svc("svc-a", &["http://host-a:8000"]),
                svc("svc-b", &["http://host-b:8000"]),
            ],
        };
        let constants = vec![
            constant("a_url", "http://host-a:8000"),
            constant("b_url", "http://host-b:8000"),
        ];
        let oracle = ServiceOracle::from_parts(&constants, &config);
        assert_eq!(
            oracle.expected_service(&["a_url".to_string(), "b_url".to_string()]),
            None,
        );
    }

    #[test]
    fn service_for_url_matches_on_host() {
        let config = ConfigurationData {
            service_descriptions: vec![
                svc("medical-data-service", &["http://medical-data-service:8000"]),
                svc("clinical-data-service", &["http://clinical-data-service:8000"]),
            ],
        };
        // Port/path differ from config but host matches -> hit.
        assert_eq!(
            service_for_url("http://medical-data-service:5000/v1", &config),
            Some("medical-data-service".to_string())
        );
        // No configured service on this host -> miss.
        assert_eq!(service_for_url("http://unknown-service:8000", &config), None);
        // No host -> miss.
        assert_eq!(service_for_url("", &config), None);
    }

    #[test]
    fn from_constants_file_reads_only_constants() {
        // Config held in memory; only the constants file is read from disk.
        let config = ConfigurationData {
            service_descriptions: vec![svc(
                "medical-data-service",
                &["http://medical-data-service:8000"],
            )],
        };
        let oracle = ServiceOracle::from_constants_file(
            manifest_relative("../config/constants/empaia-constants.json"),
            &config,
        )
        .expect("empaia constants load");
        // Only mds_url's host is in the in-memory config, so it resolves;
        // constants for other hosts are dropped (partial oracle).
        assert_eq!(
            oracle.expected_service(&["mds_url".to_string()]),
            Some("medical-data-service")
        );
        assert!(oracle.len() >= 1);
    }

    #[test]
    fn empaia_known_edges_resolve() {
        let oracle = ServiceOracle::load(
            manifest_relative("../config/constants/empaia-constants.json"),
            manifest_relative("../config/configurations/empaia-config.json"),
        )
        .expect("empaia fixtures load");

        assert_eq!(
            oracle.expected_service(&["mds_url".to_string()]),
            Some("medical-data-service")
        );
        assert_eq!(
            oracle.expected_service(&["cds_url".to_string()]),
            Some("clinical-data-service")
        );
        assert_eq!(
            oracle.expected_service(&["es_url".to_string()]),
            Some("examination-service")
        );
        assert_eq!(
            oracle.expected_service(&["as_url".to_string()]),
            Some("annotation-service")
        );
        // prefixed / underscored identifiers normalize to the same edges.
        assert_eq!(
            oracle.expected_service(&["settings.mps_url".to_string()]),
            oracle.expected_service(&["mps_url".to_string()]),
        );
        assert_eq!(
            oracle.expected_service(&["self._mds_url".to_string()]),
            Some("medical-data-service")
        );
        assert!(oracle.len() > 0);
    }
}
