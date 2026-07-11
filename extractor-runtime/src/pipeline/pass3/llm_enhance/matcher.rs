//! Deterministic identifier -> service matcher (Phase 2a).
//!
//! Lexically matches a residual call site's signals
//! ([`super::signals::CallSiteSignals`]) against the configured service set.
//! No LLM. High precision, deliberately partial recall: accepts only an
//! unambiguous match at the strongest available signal and abstains (returns
//! `None`) on ambiguity or no match, deferring to the LLM. See plan §5.

use std::collections::BTreeSet;

use models::{ConfigurationData, configuration::ServiceDescription};

use crate::pipeline::pass3::llm_enhance::signals::CallSiteSignals;
use crate::pipeline::pass3::llm_enhance::tokens::{RECEIVER_KEYWORDS, split_camel, split_snake};

/// Tokens stripped before matching so they cannot cause spurious hits.
const GENERIC_TOKENS: [&str; 6] = ["service", "client", "url", "uri", "api", "http"];

/// A configured service reduced to its match keys.
pub(super) struct IndexedService<'a> {
    desc: &'a ServiceDescription,
    /// Stripped, lowercased token set (generics removed). e.g. {medical,data}.
    tokens: BTreeSet<String>,
    /// First char of every full-name token (before stripping). e.g. `mds`.
    acronym: String,
}

/// Build the per-service match index over the configured service set. Build it
/// ONCE and reuse it across call sites -- [`deterministic_match`] takes the index
/// rather than the config so the per-residual loop does not rebuild it.
///
/// Config entries with no URL (e.g. empaia's `models` -- a shared data-models
/// package, `urls: []`, not a microservice) are excluded: they can never be a
/// resolution target (the service->URL rewrite abstains on no-URL), and keeping
/// them in the index only lets a generic token (`models`) shadow a real
/// second-place match into a spurious `>1` ambiguous abstention. Dropping them
/// can therefore only unmask real matches -- never lose one.
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

/// Tokenize an identifier: split on non-alphanumerics, then camel/snake split
/// each piece, lowercased. Generics are NOT stripped here.
fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .flat_map(split_snake)
        .flat_map(|p| split_camel(&p))
        .map(|p| p.to_lowercase())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Lowercase token set with generic tokens removed.
fn strip_generics(tokens: Vec<String>) -> BTreeSet<String> {
    tokens
        .into_iter()
        .filter(|t| !GENERIC_TOKENS.contains(&t.as_str()))
        .collect()
}

/// Reduce a signal string to its stripped token set.
fn signal_tokens(s: &str) -> BTreeSet<String> {
    strip_generics(tokenize(s))
}

/// An operand identifier -> acronym/word match key: drop bare receiver
/// keywords, strip leading underscores, and strip a trailing `_url`/`_uri`
/// suffix. e.g. `_mds_url` -> `mds`, `cds_url` -> `cds`, `annotation_url` ->
/// `annotation`, `self` -> `` (skipped by the caller). The identifier tokenizer
/// splits on `.`, so a receiver arrives as its own token, not a `self.` prefix.
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

/// A service's stripped tokens are contained in (subset-or-equal) the signal's
/// tokens. Empty token sets never match.
fn service_tokens_subset(svc: &IndexedService, signal: &BTreeSet<String>) -> bool {
    !svc.tokens.is_empty() && !signal.is_empty() && svc.tokens.is_subset(signal)
}

/// Indices of services matched by any key in the `client_class` signal group.
fn match_client_class(index: &[IndexedService], class: &str) -> Vec<usize> {
    let signal = signal_tokens(class);
    index
        .iter()
        .enumerate()
        .filter(|(_, svc)| service_tokens_subset(svc, &signal))
        .map(|(i, _)| i)
        .collect()
}

/// Indices matched by any import string (token-subset).
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

/// Indices matched by any operand identifier: acronym equality OR token-subset
/// (so a full-word identifier like `annotation` also matches).
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

/// Deterministically resolve the target service for a residual call site.
///
/// Tries signal groups strongest-first (`client_class`, `imports`,
/// `operand_identifiers`), always excluding the origin (caller) service. The
/// first group that yields exactly one non-origin hit wins; a group with
/// multiple hits is ambiguous and abstains; an empty group falls through to the
/// next. Returns the matched service, or `None` to defer to the LLM.
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

    /// Build the index from a config and match in one step (mirrors how callers
    /// build the index once, then match per call site).
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
        // The only matching service is the origin -> excluded -> falls through
        // -> None (no self-loop).
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
        // A no-URL config entry (a shared data-models package, not a service)
        // whose token would otherwise match is excluded, so it neither resolves
        // nor shadows a real match into an ambiguous abstention.
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
        // Without the no-URL exclusion, `annotation` would match BOTH
        // annotation-service and annotation-models -> ambiguous -> None.
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
