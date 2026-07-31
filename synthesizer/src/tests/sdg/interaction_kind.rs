#[cfg(test)]
mod tests {
    use crate::sdg::interaction_kind::{
        InteractionKind, InteractionSignals, classify, host_of, is_reflexive,
    };

    #[test]
    fn default_is_business() {
        assert_eq!(InteractionKind::default(), InteractionKind::Business);
    }

    #[test]
    fn ordering_encodes_precedence() {
        // TestOrigin > Reflexive > HealthInfra as a *precedence*, which as an
        // Ord means TestOrigin sorts first. Business sorts before all of them.
        assert!(InteractionKind::Business < InteractionKind::TestOrigin);
        assert!(InteractionKind::TestOrigin < InteractionKind::Reflexive);
        assert!(InteractionKind::Reflexive < InteractionKind::HealthInfra);
    }

    #[test]
    fn rollup_business_wins_any_tie() {
        let kinds = [InteractionKind::HealthInfra, InteractionKind::Business];
        assert_eq!(
            kinds.iter().copied().min(),
            Some(InteractionKind::Business),
            "one business request must keep the whole edge in the business view"
        );
    }

    #[test]
    fn rollup_of_only_non_business_picks_highest_precedence() {
        // The case the 2026-07-21 spec left undefined.
        let kinds = [InteractionKind::HealthInfra, InteractionKind::TestOrigin];
        assert_eq!(
            kinds.iter().copied().min(),
            Some(InteractionKind::TestOrigin)
        );
    }

    #[test]
    fn serialises_as_a_bare_name_and_defaults_when_absent() {
        let json = serde_json::to_string(&InteractionKind::HealthInfra)
            .expect("InteractionKind must serialise");
        assert_eq!(json, "\"HealthInfra\"");

        // A legacy sdg.json has no kind field at all.
        #[derive(serde::Deserialize)]
        struct Legacy {
            #[serde(default)]
            kind: InteractionKind,
        }
        let legacy: Legacy = serde_json::from_str("{}").expect("legacy JSON must load");
        assert_eq!(legacy.kind, InteractionKind::Business);
    }

    /// Signals for a plain business call, so each test varies one field.
    fn business_signals() -> InteractionSignals<'static> {
        InteractionSignals {
            caller_file: "ts-order-service/src/main/java/OrderController.java",
            target_path: "/api/v1/travelservice/trips",
            target_uri: "http://ts-travel-service:12346/api/v1/travelservice/trips",
        }
    }

    fn own_urls() -> Vec<String> {
        vec!["http://ts-order-service:12031".to_string()]
    }

    /// Every service's configured URLs flattened -- the caller's own plus one
    /// other service, enough to exercise the "someone else owns it" rule.
    fn all_urls() -> Vec<String> {
        let mut urls = own_urls();
        urls.push("http://ts-travel-service:12346".to_string());
        urls
    }

    #[test]
    fn plain_call_is_business() {
        assert_eq!(
            classify(&business_signals(), &own_urls(), &all_urls()),
            InteractionKind::Business
        );
    }

    #[test]
    fn test_origin_from_java_test_path() {
        let s = InteractionSignals {
            caller_file: "ts-preserve-service/src/test/java/PreserveServiceImplTest.java",
            ..business_signals()
        };
        assert_eq!(
            classify(&s, &own_urls(), &all_urls()),
            InteractionKind::TestOrigin
        );

        // File-name convention without the /src/test/ segment.
        let s = InteractionSignals {
            caller_file: "ts-preserve-service/java/FooIT.java",
            ..business_signals()
        };
        assert_eq!(
            classify(&s, &own_urls(), &all_urls()),
            InteractionKind::TestOrigin
        );
    }

    #[test]
    fn test_origin_from_python_test_file() {
        for path in [
            "medical-data-service/tests/test_slides.py",
            "medical-data-service/app/conftest.py",
            "medical-data-service/app/slides_test.py",
        ] {
            let s = InteractionSignals {
                caller_file: path,
                ..business_signals()
            };
            assert_eq!(
                classify(&s, &own_urls(), &all_urls()),
                InteractionKind::TestOrigin,
                "{path} should be test-origin"
            );
        }
    }

    #[test]
    fn production_file_named_like_a_test_is_not_test_origin() {
        // 'latest' contains 'test'; only a whole path segment counts.
        let s = InteractionSignals {
            caller_file: "medical-data-service/app/latest/client.py",
            ..business_signals()
        };
        assert_eq!(
            classify(&s, &own_urls(), &all_urls()),
            InteractionKind::Business
        );
    }

    #[test]
    fn health_probe_typed() {
        for path in ["/alive", "/actuator/health", "/api/v1/healthz"] {
            let s = InteractionSignals {
                target_path: path,
                ..business_signals()
            };
            assert_eq!(
                classify(&s, &own_urls(), &all_urls()),
                InteractionKind::HealthInfra,
                "{path} should be health-infra"
            );
        }
    }

    #[test]
    fn health_rule_works_on_a_full_url_fallback() {
        // When the matched endpoint uri is empty the builder passes the raw
        // target_uri instead; the last-segment rule must still fire.
        let s = InteractionSignals {
            target_path: "http://auth-service/alive",
            ..business_signals()
        };
        assert_eq!(
            classify(&s, &own_urls(), &all_urls()),
            InteractionKind::HealthInfra
        );
    }

    #[test]
    fn actuator_must_be_a_whole_segment() {
        // Real Spring actuator paths stay health-infra.
        for path in [
            "/actuator/health",
            "/actuator/info",
            "http://svc:8080/actuator/health",
        ] {
            let s = InteractionSignals {
                target_path: path,
                ..business_signals()
            };
            assert_eq!(
                classify(&s, &own_urls(), &all_urls()),
                InteractionKind::HealthInfra,
                "{path} should be health-infra"
            );
        }
        // A business path that merely starts with the same letters must not be.
        for path in ["/actuator-config", "/api/v1/actuatorish-metrics"] {
            let s = InteractionSignals {
                target_path: path,
                ..business_signals()
            };
            assert_eq!(
                classify(&s, &own_urls(), &all_urls()),
                InteractionKind::Business,
                "{path} is not a health probe"
            );
        }
    }

    #[test]
    fn reflexive_localhost() {
        // Nobody in all_urls owns localhost (no port), so this falls through
        // to the loopback fallback (rule step 3).
        let s = InteractionSignals {
            target_uri: "http://localhost/api/v1/travelservice/trips",
            ..business_signals()
        };
        assert_eq!(
            classify(&s, &own_urls(), &all_urls()),
            InteractionKind::Reflexive
        );
    }

    #[test]
    fn reflexive_own_config_url() {
        // own_urls holds full URLs, not bare hosts -- is_reflexive parses them.
        // The authority (host:port) must match exactly, not just the host.
        let s = InteractionSignals {
            target_uri: "http://ts-order-service:12031/api/v1/travelservice/trips",
            ..business_signals()
        };
        assert_eq!(
            classify(&s, &own_urls(), &all_urls()),
            InteractionKind::Reflexive
        );
    }

    #[test]
    fn another_services_configured_url_is_never_reflexive_even_on_loopback() {
        // Ownership beats loopback: a different service configured on
        // localhost at its own port is a real cross-service target, not self.
        let all_urls = vec![
            "http://ts-order-service:12031".to_string(),
            "http://localhost:10005".to_string(),
        ];
        let s = InteractionSignals {
            target_uri: "http://localhost:10005/v3/files",
            ..business_signals()
        };
        assert_eq!(
            classify(&s, &own_urls(), &all_urls),
            InteractionKind::Business
        );
    }

    #[test]
    fn precedence_test_beats_reflexive_beats_health() {
        // A health probe to localhost from a test file is all three; TestOrigin wins.
        let s = InteractionSignals {
            caller_file: "svc/src/test/java/AliveTest.java",
            target_path: "/alive",
            target_uri: "http://localhost/alive",
        };
        assert_eq!(
            classify(&s, &own_urls(), &all_urls()),
            InteractionKind::TestOrigin
        );

        // Drop the test origin: Reflexive beats HealthInfra.
        let s = InteractionSignals {
            caller_file: "svc/src/main/java/Alive.java",
            target_path: "/alive",
            target_uri: "http://localhost/alive",
        };
        assert_eq!(
            classify(&s, &own_urls(), &all_urls()),
            InteractionKind::Reflexive
        );
    }

    #[test]
    fn host_of_handles_the_shapes_that_occur() {
        assert_eq!(
            host_of("http://ts-travel-service:12346/api/v1"),
            "ts-travel-service"
        );
        assert_eq!(host_of("http://localhost:8000/fhir/x"), "localhost");
        assert_eq!(host_of("https://mds/slides"), "mds");
        assert_eq!(host_of("http://user:pw@mds:8000/x"), "mds");
        assert_eq!(host_of("http://[::1]:8000/x"), "[::1]");
        assert_eq!(host_of("http://mds"), "mds");
        assert_eq!(
            host_of("/api/v1/relative"),
            "",
            "a relative URI has no host"
        );
        assert_eq!(host_of(""), "");
    }

    #[test]
    fn relative_target_is_never_reflexive() {
        // Guards against a regression: today's behaviour for relative URIs must
        // not change, or every relative call becomes a self-loop.
        assert!(!is_reflexive("/api/v1/trips", &own_urls(), &all_urls()));
    }
}
