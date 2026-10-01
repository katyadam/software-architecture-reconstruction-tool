//! Lexical call-site -> service matcher. Abstains on ambiguity.

use std::collections::BTreeSet;

use models::{ConfigurationData, configuration::ServiceDescription};

use crate::pipeline::pass3::llm_enhance::signals::CallSiteSignals;
use crate::pipeline::pass3::llm_enhance::tokens::{RECEIVER_KEYWORDS, split_camel, split_snake};

const GENERIC_TOKENS: [&str; 6] = ["service", "client", "url", "uri", "api", "http"];

pub(super) struct IndexedService<'a> {
    desc: &'a ServiceDescription,
    /// e.g. {medical, data}
    tokens: BTreeSet<String>,
    /// e.g. `mds`
    acronym: String,
}

/// Build once, reuse per call site. No-URL entries (shared packages) are
/// skipped: never a target, only cause false ambiguity.
pub(super) fn build_index(config: &ConfigurationData) -> Vec<IndexedService<'_>> {
    config
        .service_descriptions
        .iter()
        .filter(|desc| !desc.urls.is_empty())
        .map(|desc| {
            let full = tokenize(&desc.name);
            let acronym: String = full
                .iter()
                .filter_map(|t| t.chars().next())
                .collect::<String>()
                .to_lowercase();
            let tokens = strip_generics(full);
            IndexedService {
                desc,
                tokens,
                acronym,
            }
        })
        .collect()
}

fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .flat_map(split_snake)
        .flat_map(|p| split_camel(&p))
        .map(|p| p.to_lowercase())
        .filter(|p| !p.is_empty())
        .collect()
}

fn strip_generics(tokens: Vec<String>) -> BTreeSet<String> {
    tokens
        .into_iter()
        .filter(|t| !GENERIC_TOKENS.contains(&t.as_str()))
        .collect()
}

fn signal_tokens(s: &str) -> BTreeSet<String> {
    strip_generics(tokenize(s))
}

/// `_mds_url` -> `mds`, `annotation_url` -> `annotation`, `self` -> ``.
fn operand_key(ident: &str) -> String {
    let s = ident.trim();
    if RECEIVER_KEYWORDS.contains(&s) {
        return String::new();
    }
    let s = s.trim_start_matches('_').to_lowercase();
    let s = s
        .strip_suffix("_url")
        .or_else(|| s.strip_suffix("_uri"))
        .unwrap_or(&s);
    s.to_string()
}

fn service_tokens_subset(svc: &IndexedService, signal: &BTreeSet<String>) -> bool {
    !svc.tokens.is_empty() && !signal.is_empty() && svc.tokens.is_subset(signal)
}

fn match_client_class(index: &[IndexedService], class: &str) -> Vec<usize> {
    let signal = signal_tokens(class);
    index
        .iter()
        .enumerate()
        .filter(|(_, svc)| service_tokens_subset(svc, &signal))
        .map(|(i, _)| i)
        .collect()
}

fn match_imports(index: &[IndexedService], imports: &[String]) -> Vec<usize> {
    let mut hits = BTreeSet::new();
    for imp in imports {
        let signal = signal_tokens(imp);
        for (i, svc) in index.iter().enumerate() {
            if service_tokens_subset(svc, &signal) {
                hits.insert(i);
            }
        }
    }
    hits.into_iter().collect()
}

/// Acronym equality or token subset.
fn match_operands(index: &[IndexedService], identifiers: &[String]) -> Vec<usize> {
    let mut hits = BTreeSet::new();
    for ident in identifiers {
        let key = operand_key(ident);
        if key.is_empty() {
            continue;
        }
        let word = signal_tokens(&key);
        for (i, svc) in index.iter().enumerate() {
            if svc.acronym == key || service_tokens_subset(svc, &word) {
                hits.insert(i);
            }
        }
    }
    hits.into_iter().collect()
}

/// Groups strongest-first: class, imports, operands. Origin excluded.
/// One hit wins, many abstain, none falls through.
pub(super) fn deterministic_match(
    signals: &CallSiteSignals,
    index: &[IndexedService],
) -> Option<ServiceDescription> {
    let groups: [Vec<usize>; 3] = [
        signals
            .client_class
            .as_deref()
            .map(|c| match_client_class(index, c))
            .unwrap_or_default(),
        match_imports(index, &signals.imports),
        match_operands(index, &signals.operand_identifiers),
    ];

    for group in groups {
        let hits: Vec<usize> = group
            .into_iter()
            .filter(|&i| index[i].desc.name != signals.origin_service)
            .collect();
        match hits.len() {
            1 => return Some(index[hits[0]].desc.clone()),
            0 => continue,
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svc(name: &str) -> ServiceDescription {
        ServiceDescription {
            name: name.to_string(),
            base_dir_path: format!("/proj/{name}"),
            urls: vec![format!("http://{name}:8000")],
        }
    }

    fn config(names: &[&str]) -> ConfigurationData {
        ConfigurationData {
            service_descriptions: names.iter().map(|n| svc(n)).collect(),
        }
    }

    fn resolve(s: &CallSiteSignals, cfg: &ConfigurationData) -> Option<ServiceDescription> {
        deterministic_match(s, &build_index(cfg))
    }

    fn signals(
        origin: &str,
        client_class: Option<&str>,
        imports: &[&str],
        operands: &[&str],
    ) -> CallSiteSignals {
        CallSiteSignals {
            origin_service: origin.to_string(),
            client_class: client_class.map(|c| c.to_string()),
            imports: imports.iter().map(|s| s.to_string()).collect(),
            operand_identifiers: operands.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn class_unique_accept() {
        let cfg = config(&[
            "medical-data-service",
            "clinical-data-service",
            "app-service",
        ]);
        let s = signals("app-service", Some("MedicalDataServiceClient"), &[], &[]);
        let hit = resolve(&s, &cfg).expect("should match");
        assert_eq!(hit.name, "medical-data-service");
    }

    #[test]
    fn operand_acronym_unique_accept() {
        let cfg = config(&[
            "clinical-data-service",
            "medical-data-service",
            "app-service",
        ]);
        let s = signals("app-service", None, &[], &["cds_url"]);
        let hit = resolve(&s, &cfg).expect("should match");
        assert_eq!(hit.name, "clinical-data-service");
    }

    #[test]
    fn operand_acronym_ambiguous_abstains() {
        let cfg = config(&["event-service", "examination-service", "app-service"]);
        let s = signals("app-service", None, &[], &["es_url"]);
        // `es` acronym matches BOTH event-service and examination-service.
        assert!(resolve(&s, &cfg).is_none());
    }

    #[test]
    fn class_fires_before_ambiguous_operand() {
        let cfg = config(&["event-service", "examination-service", "app-service"]);
        let s = signals(
            "app-service",
            Some("ExaminationServiceClient"),
            &[],
            &["es_url"],
        );
        // class group resolves uniquely before the ambiguous operand acronym.
        let hit = resolve(&s, &cfg).expect("class should fire first");
        assert_eq!(hit.name, "examination-service");
    }

    #[test]
    fn untelling_operand_no_match() {
        let cfg = config(&[
            "medical-data-service",
            "clinical-data-service",
            "app-service",
        ]);
        let s = signals("app-service", None, &[], &["base_url"]);
        assert!(resolve(&s, &cfg).is_none());
    }

    #[test]
    fn origin_service_hit_is_excluded() {
        // Only match is origin -> no self-loop.
        let cfg = config(&["medical-data-service", "app-service"]);
        let s = signals(
            "medical-data-service",
            Some("MedicalDataServiceClient"),
            &[],
            &[],
        );
        assert!(resolve(&s, &cfg).is_none());
    }

    #[test]
    fn import_token_subset_accept() {
        let cfg = config(&[
            "medical-data-service",
            "clinical-data-service",
            "app-service",
        ]);
        let s = signals(
            "app-service",
            None,
            &["custom_clients.medical_data_service.MedicalDataServiceClient"],
            &[],
        );
        let hit = resolve(&s, &cfg).expect("import should match");
        assert_eq!(hit.name, "medical-data-service");
    }

    #[test]
    fn no_url_service_excluded_from_index() {
        let cfg = ConfigurationData {
            service_descriptions: vec![
                svc("annotation-service"),
                ServiceDescription {
                    name: "annotation-models".to_string(),
                    base_dir_path: "/proj/models".to_string(),
                    urls: vec![],
                },
                svc("app-service"),
            ],
        };
        // Else `annotation` matches both -> ambiguous.
        let s = signals("app-service", None, &[], &["annotation_url"]);
        let hit = resolve(&s, &cfg).expect("no-URL entry excluded -> unique match");
        assert_eq!(hit.name, "annotation-service");
    }

    #[test]
    fn operand_full_word_token_subset_accept() {
        let cfg = config(&["annotation-service", "app-service"]);
        let s = signals("app-service", None, &[], &["annotation_url"]);
        let hit = resolve(&s, &cfg).expect("word should match");
        assert_eq!(hit.name, "annotation-service");
    }
}
