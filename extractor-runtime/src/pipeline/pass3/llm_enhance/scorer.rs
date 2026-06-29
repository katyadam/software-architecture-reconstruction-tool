//! Precision/recall scorer for the service matcher (S0.2).
//!
//! Pure function over produced edges and the auto-derived [`ServiceOracle`].
//! Reused verbatim in the Phase 3 final evaluation.

use crate::pipeline::pass3::llm_enhance::oracle::ServiceOracle;

/// One resolution attempt: the residual's operand identifiers (used to look up
/// the expected service via the oracle) and the service the matcher chose
/// (`None` when it abstained).
pub(super) struct ProducedEdge {
    pub identifiers: Vec<String>,
    pub chosen_service: Option<String>,
}

/// Scoring result over the scoreable population.
pub(super) struct Score {
    pub precision: f64,
    pub recall: f64,
    pub correct: usize,
    pub produced: usize,
    pub scoreable: usize,
}

/// Score `produced` edges against `oracle`.
///
/// A residual is **scoreable** iff the oracle has an expected service for its
/// identifiers. Only scoreable residuals count toward recall's denominator.
/// - **produced** = scoreable residuals where the matcher chose a service.
/// - **correct** = scoreable residuals where the choice equals the expected.
/// - **precision** = correct / produced (0.0 when produced == 0).
/// - **recall** = correct / scoreable (0.0 when scoreable == 0).
///
/// All divisions are guarded against zero (no NaN).
pub(super) fn score(produced: &[ProducedEdge], oracle: &ServiceOracle) -> Score {
    let mut scoreable = 0;
    let mut produced_count = 0;
    let mut correct = 0;

    for edge in produced {
        let Some(expected) = oracle.expected_service(&edge.identifiers) else {
            continue; // unscoreable -> ignored entirely
        };
        scoreable += 1;
        if let Some(chosen) = &edge.chosen_service {
            produced_count += 1;
            if chosen == expected {
                correct += 1;
            }
        }
    }

    let precision = if produced_count == 0 {
        0.0
    } else {
        correct as f64 / produced_count as f64
    };
    let recall = if scoreable == 0 {
        0.0
    } else {
        correct as f64 / scoreable as f64
    };

    Score {
        precision,
        recall,
        correct,
        produced: produced_count,
        scoreable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::ConfigurationData;
    use models::configuration::ServiceDescription;

    // The oracle's constants/`from_parts` types are private to its module, so
    // build the oracle through the public-to-super loader is not possible with
    // inline data; instead reuse `from_parts` via the oracle module's test
    // helper path. We construct a tiny config + constants here.
    use crate::pipeline::pass3::llm_enhance::oracle::OracleConstant;

    fn svc(name: &str, urls: &[&str]) -> ServiceDescription {
        ServiceDescription {
            name: name.to_string(),
            base_dir_path: format!("/proj/{name}"),
            urls: urls.iter().map(|u| u.to_string()).collect(),
        }
    }

    fn oracle() -> ServiceOracle {
        let config = ConfigurationData {
            service_descriptions: vec![
                svc("medical-data-service", &["http://medical-data-service:8000"]),
                svc("clinical-data-service", &["http://clinical-data-service:8000"]),
            ],
        };
        let constants = vec![
            OracleConstant {
                name: "mds_url".to_string(),
                value: "http://medical-data-service:5000".to_string(),
            },
            OracleConstant {
                name: "cds_url".to_string(),
                value: "http://clinical-data-service:8000".to_string(),
            },
        ];
        ServiceOracle::from_parts(&constants, &config)
    }

    fn edge(ident: &str, chosen: Option<&str>) -> ProducedEdge {
        ProducedEdge {
            identifiers: vec![ident.to_string()],
            chosen_service: chosen.map(|s| s.to_string()),
        }
    }

    #[test]
    fn perfect() {
        let s = score(
            &[
                edge("mds_url", Some("medical-data-service")),
                edge("cds_url", Some("clinical-data-service")),
            ],
            &oracle(),
        );
        assert_eq!((s.scoreable, s.produced, s.correct), (2, 2, 2));
        assert_eq!(s.precision, 1.0);
        assert_eq!(s.recall, 1.0);
    }

    #[test]
    fn partial_one_abstained() {
        let s = score(
            &[
                edge("mds_url", Some("medical-data-service")),
                edge("cds_url", None),
            ],
            &oracle(),
        );
        assert_eq!((s.scoreable, s.produced, s.correct), (2, 1, 1));
        assert_eq!(s.precision, 1.0); // 1/1
        assert_eq!(s.recall, 0.5); // 1/2
    }

    #[test]
    fn all_wrong() {
        let s = score(
            &[
                edge("mds_url", Some("clinical-data-service")),
                edge("cds_url", Some("medical-data-service")),
            ],
            &oracle(),
        );
        assert_eq!((s.scoreable, s.produced, s.correct), (2, 2, 0));
        assert_eq!(s.precision, 0.0);
        assert_eq!(s.recall, 0.0);
    }

    #[test]
    fn all_abstained() {
        let s = score(&[edge("mds_url", None), edge("cds_url", None)], &oracle());
        assert_eq!((s.scoreable, s.produced, s.correct), (2, 0, 0));
        assert_eq!(s.precision, 0.0); // guarded
        assert_eq!(s.recall, 0.0);
    }

    #[test]
    fn empty_no_nan() {
        let s = score(&[], &oracle());
        assert_eq!((s.scoreable, s.produced, s.correct), (0, 0, 0));
        assert_eq!(s.precision, 0.0);
        assert_eq!(s.recall, 0.0);
    }

    #[test]
    fn unscoreable_ignored() {
        // An identifier with no oracle edge is not scoreable: it must not count.
        let s = score(
            &[
                edge("mds_url", Some("medical-data-service")),
                edge("unknown_url", Some("medical-data-service")),
            ],
            &oracle(),
        );
        assert_eq!((s.scoreable, s.produced, s.correct), (1, 1, 1));
        assert_eq!(s.precision, 1.0);
        assert_eq!(s.recall, 1.0);
    }
}
