mod interaction_kind;

#[cfg(test)]
mod tests {
    use models::{
        ConfigurationData, Endpoint, HttpMethod, RestCall, configuration::ServiceDescription,
    };
    use strsim::levenshtein;

    use crate::sdg::builder::{SdgBuilder, SdgBuilderImpl};
    use crate::sdg::interaction_kind::InteractionKind;

    #[test]
    fn should_create_simple_sdg() {
        let service = SdgBuilderImpl::new();
        let sample_data = sample_data();
        let configuration = sample_configuration();
        let sdg = service
            .build(&sample_data.0, &sample_data.1, &configuration, &vec![])
            .expect("This test doesn't produce error!");
        assert!(
            sdg.services.len() == 2,
            "Incorrect number of services, should be 2"
        );
        assert!(
            sdg.connections.len() == 1,
            "Incorrect number of connections, should be 1"
        );
        assert!(
            sdg.connections.iter().all(|conn| conn.requests.len() == 3),
            "Incorrect number of matched requests within the only connection, should be 3"
        );
        assert!(
            sdg.connections.iter().all(|conn| {
                conn.requests
                    .iter()
                    .all(|req| levenshtein(&req.endpoint.uri, &req.restcall.target_uri) == 24)
            }),
            "Not matching URIs between matched endpoint-restcall pair, should be matching or atleast similar"
        );
    }

    #[test]
    fn should_type_test_origin_and_health_requests() {
        let builder = SdgBuilderImpl::new();
        let endpoints = vec![
            Endpoint {
                function_name: "send_email".to_string(),
                http_method: HttpMethod::POST,
                uri: "/api/v1/notifyservice/notification".to_string(),
                file_path: "ts-notification-service/src/main/java/Controller.java".to_string(),
                ..Default::default()
            },
            Endpoint {
                function_name: "alive".to_string(),
                http_method: HttpMethod::GET,
                uri: "/alive".to_string(),
                file_path: "ts-notification-service/src/main/java/Health.java".to_string(),
                ..Default::default()
            },
        ];
        let restcalls = vec![
            // Call site in test code -> TestOrigin.
            RestCall {
                function_name: "testSendEmail".to_string(),
                http_method: HttpMethod::POST,
                target_uri:
                    "http://ts-notification-service:17853/api/v1/notifyservice/notification"
                        .to_string(),
                file_path: "ts-preserve-service/src/test/java/PreserveServiceImplTest.java"
                    .to_string(),
                ..Default::default()
            },
            // Production health probe -> HealthInfra.
            RestCall {
                function_name: "checkAlive".to_string(),
                http_method: HttpMethod::GET,
                target_uri: "http://ts-notification-service:17853/alive".to_string(),
                file_path: "ts-preserve-service/src/main/java/Monitor.java".to_string(),
                ..Default::default()
            },
        ];
        // ConfigurationData has exactly one field and derives no Default.
        let configuration = ConfigurationData {
            service_descriptions: vec![
                ServiceDescription {
                    name: "ts-preserve-service".to_string(),
                    base_dir_path: "ts-preserve-service".to_string(),
                    urls: vec!["http://ts-preserve-service:14568".to_string()],
                },
                ServiceDescription {
                    name: "ts-notification-service".to_string(),
                    base_dir_path: "ts-notification-service".to_string(),
                    urls: vec!["http://ts-notification-service:17853".to_string()],
                },
            ],
        };

        let sdg = builder
            .build(&endpoints, &restcalls, &configuration, &[])
            .expect("build must succeed");

        let conn = sdg
            .connections
            .iter()
            .find(|c| {
                c.source_id == "ts-preserve-service" && c.target_id == "ts-notification-service"
            })
            .expect("the preserve -> notification connection must exist");

        let kinds: Vec<InteractionKind> = conn.requests.iter().map(|r| r.kind).collect();
        assert!(
            kinds.contains(&InteractionKind::TestOrigin),
            "the test-code call site must be TestOrigin, got {kinds:?}"
        );
        assert!(
            kinds.contains(&InteractionKind::HealthInfra),
            "the /alive call must be HealthInfra, got {kinds:?}"
        );
        assert!(
            !kinds.contains(&InteractionKind::Business),
            "neither request is business, got {kinds:?}"
        );
        assert_eq!(
            conn.kind,
            InteractionKind::TestOrigin,
            "with no business request the rollup takes the highest precedence"
        );
    }

    #[test]
    fn connection_rolls_up_to_business_when_one_request_is_and_another_isnt() {
        // A single connection carrying both a real business request and a
        // non-business one (a health probe) must still roll up to Business --
        // one real business request keeps the whole edge in the business view.
        let builder = SdgBuilderImpl::new();
        let (mut endpoints, mut restcalls) = sample_data();
        let configuration = sample_configuration();

        // Same source/target service pair as the business traffic above, so
        // this lands in the *same* connection rather than a new one.
        endpoints.push(Endpoint {
            function_name: "health".to_string(),
            http_method: HttpMethod::GET,
            uri: "/health".to_string(),
            file_path: "crm/user-service/src/api/controller.py".to_string(),
            ..Default::default()
        });
        restcalls.push(RestCall {
            function_name: "check_health".to_string(),
            http_method: HttpMethod::GET,
            target_uri: "http://user-service:8000/health".to_string(),
            file_path: "crm/admin-user-service/src/api/user_connector.py".to_string(),
            ..Default::default()
        });

        let sdg = builder
            .build(&endpoints, &restcalls, &configuration, &[])
            .expect("build must succeed");

        let conn = sdg
            .connections
            .iter()
            .find(|c| c.source_id == "admin-user-service" && c.target_id == "user-service")
            .expect("the admin-user-service -> user-service connection must exist");

        let kinds: Vec<InteractionKind> = conn.requests.iter().map(|r| r.kind).collect();
        assert!(
            kinds.contains(&InteractionKind::Business),
            "must still carry the original business requests, got {kinds:?}"
        );
        assert!(
            kinds.contains(&InteractionKind::HealthInfra),
            "must carry the added health probe, got {kinds:?}"
        );
        assert_eq!(
            conn.kind,
            InteractionKind::Business,
            "one business request must keep the whole connection Business, got requests {kinds:?}"
        );
    }

    fn sample_data() -> (Vec<Endpoint>, Vec<RestCall>) {
        (
            vec![
                Endpoint {
                    function_name: "get_user".to_string(),
                    function_hash: "some-random-hash".to_string(),
                    parameters: vec![],
                    http_method: HttpMethod::GET,
                    uri: "/users/{id}".to_string(),
                    file_path: "crm/user-service/src/api/controller.py".to_string(),
                    ..Default::default()
                },
                Endpoint {
                    function_name: "create_user".to_string(),
                    function_hash: "some-random-hash".to_string(),
                    parameters: vec![],
                    http_method: HttpMethod::POST,
                    uri: "/users".to_string(),
                    file_path: "crm/user-service/src/api/controller.py".to_string(),
                    ..Default::default()
                },
                Endpoint {
                    function_name: "delete_user".to_string(),
                    function_hash: "some-random-hash".to_string(),
                    parameters: vec![],
                    http_method: HttpMethod::DELETE,
                    uri: "/users/{id}".to_string(),
                    file_path: "crm/user-service/src/api/controller.py".to_string(),
                    ..Default::default()
                },
            ],
            vec![
                RestCall {
                    function_name: "get_user".to_string(),
                    function_hash: "some-random-hash".to_string(),

                    call_arguments: vec![],
                    http_method: HttpMethod::GET,
                    target_uri: "http://user-service:8000/users/{id}".to_string(),
                    file_path: "crm/admin-user-service/src/api/user_connector.py".to_string(),
                    source_span: Default::default(),
                },
                RestCall {
                    function_name: "create_user".to_string(),
                    function_hash: "some-random-hash".to_string(),

                    call_arguments: vec![],
                    http_method: HttpMethod::POST,
                    target_uri: "http://user-service:8000/users".to_string(),
                    file_path: "crm/admin-user-service/src/api/user_connector.py".to_string(),
                    source_span: Default::default(),
                },
                RestCall {
                    function_name: "delete_user".to_string(),
                    function_hash: "some-random-hash".to_string(),

                    call_arguments: vec![],
                    http_method: HttpMethod::DELETE,
                    target_uri: "http://user-service:8000/users/{id}".to_string(),
                    file_path: "crm/admin-user-service/src/api/user_connector.py".to_string(),
                    source_span: Default::default(),
                },
            ],
        )
    }

    fn sample_configuration() -> ConfigurationData {
        ConfigurationData {
            service_descriptions: vec![
                ServiceDescription {
                    name: "user-service".to_string(),
                    base_dir_path: "crm/user-service".to_string(),
                    urls: vec![
                        "http://localhost:8000".to_string(),
                        "http://user-service:8000".to_string(),
                    ],
                },
                ServiceDescription {
                    name: "admin-user-service".to_string(),
                    base_dir_path: "crm/admin-user-service".to_string(),
                    urls: vec![
                        "http://localhost:7000".to_string(),
                        "http://admin-user-service:8000".to_string(),
                    ],
                },
            ],
        }
    }

    #[test]
    fn should_not_emit_cross_service_edge_for_localhost_self_call() {
        // The real empaia shape: mds is NOT configured on localhost anywhere,
        // and app-service happens to share the port the localhost call names.
        // This is the load-bearing assertion -- it must hold with no localhost
        // in mds's config, i.e. purely from the ownership-first rule falling
        // through to the loopback fallback (nobody owns localhost:8000).
        let builder = SdgBuilderImpl::new();
        let endpoints = vec![
            Endpoint {
                function_name: "get_fhir".to_string(),
                http_method: HttpMethod::GET,
                uri: "/fhir/Patient".to_string(),
                file_path: "medical-data-service/app/api.py".to_string(),
                ..Default::default()
            },
            Endpoint {
                function_name: "get_fhir".to_string(),
                http_method: HttpMethod::GET,
                uri: "/fhir/Patient".to_string(),
                file_path: "app-service/app/api.py".to_string(),
                ..Default::default()
            },
        ];
        let restcalls = vec![RestCall {
            function_name: "read_patient".to_string(),
            http_method: HttpMethod::GET,
            target_uri: "http://localhost:8000/fhir/Patient".to_string(),
            file_path: "medical-data-service/app/fhir_resources.py".to_string(),
            ..Default::default()
        }];
        let configuration = ConfigurationData {
            service_descriptions: vec![
                ServiceDescription {
                    name: "medical-data-service".to_string(),
                    base_dir_path: "medical-data-service".to_string(),
                    urls: vec!["http://medical-data-service:5000".to_string()],
                },
                ServiceDescription {
                    name: "app-service".to_string(),
                    base_dir_path: "app-service".to_string(),
                    urls: vec!["http://app-service:8000".to_string()],
                },
            ],
        };

        let sdg = builder
            .build(&endpoints, &restcalls, &configuration, &[])
            .expect("build must succeed");

        assert!(
            !sdg.connections
                .iter()
                .any(|c| c.source_id == "medical-data-service" && c.target_id == "app-service"),
            "a localhost call must not become a cross-service edge"
        );
    }

    #[test]
    fn should_emit_self_loop_when_own_url_matches() {
        // mds *is* configured on localhost:8000 here, so ownership resolves
        // directly (step 1) and the matcher can resolve inside its own service.
        let builder = SdgBuilderImpl::new();
        let endpoints = vec![
            Endpoint {
                function_name: "get_fhir".to_string(),
                http_method: HttpMethod::GET,
                uri: "/fhir/Patient".to_string(),
                file_path: "medical-data-service/app/api.py".to_string(),
                ..Default::default()
            },
            Endpoint {
                function_name: "get_fhir".to_string(),
                http_method: HttpMethod::GET,
                uri: "/fhir/Patient".to_string(),
                file_path: "app-service/app/api.py".to_string(),
                ..Default::default()
            },
        ];
        let restcalls = vec![RestCall {
            function_name: "read_patient".to_string(),
            http_method: HttpMethod::GET,
            target_uri: "http://localhost:8000/fhir/Patient".to_string(),
            file_path: "medical-data-service/app/fhir_resources.py".to_string(),
            ..Default::default()
        }];
        let configuration = ConfigurationData {
            service_descriptions: vec![
                ServiceDescription {
                    name: "medical-data-service".to_string(),
                    base_dir_path: "medical-data-service".to_string(),
                    urls: vec!["http://localhost:8000".to_string()],
                },
                ServiceDescription {
                    name: "app-service".to_string(),
                    base_dir_path: "app-service".to_string(),
                    urls: vec!["http://app-service:8000".to_string()],
                },
            ],
        };

        let sdg = builder
            .build(&endpoints, &restcalls, &configuration, &[])
            .expect("build must succeed");

        let self_loop = sdg
            .connections
            .iter()
            .find(|c| {
                c.source_id == "medical-data-service" && c.target_id == "medical-data-service"
            })
            .expect("the localhost call must become a self-loop");
        assert_eq!(self_loop.kind, InteractionKind::Reflexive);
        assert_eq!(
            self_loop.requests[0].endpoint.uri, "/fhir/Patient",
            "the self-loop must carry the real matched endpoint, not a placeholder"
        );
    }

    #[test]
    fn should_not_treat_another_services_localhost_address_as_self() {
        // The loadtus regression: a *different* service is configured on
        // localhost at its own port. The caller's call to that address must
        // resolve as a real cross-service edge, not get swallowed as self.
        let builder = SdgBuilderImpl::new();
        let endpoints = vec![Endpoint {
            function_name: "list_files".to_string(),
            http_method: HttpMethod::GET,
            uri: "/v3/files".to_string(),
            file_path: "loadtus-service/app/api.py".to_string(),
            ..Default::default()
        }];
        let restcalls = vec![RestCall {
            function_name: "list_uploaded_files".to_string(),
            http_method: HttpMethod::GET,
            target_uri: "http://localhost:10005/v3/files".to_string(),
            file_path: "caller-service/app/client.py".to_string(),
            ..Default::default()
        }];
        let configuration = ConfigurationData {
            service_descriptions: vec![
                ServiceDescription {
                    name: "caller-service".to_string(),
                    base_dir_path: "caller-service".to_string(),
                    urls: vec!["http://caller-service:9000".to_string()],
                },
                ServiceDescription {
                    name: "loadtus-service".to_string(),
                    base_dir_path: "loadtus-service".to_string(),
                    urls: vec!["http://localhost:10005".to_string()],
                },
            ],
        };

        let sdg = builder
            .build(&endpoints, &restcalls, &configuration, &[])
            .expect("build must succeed");

        let conn = sdg
            .connections
            .iter()
            .find(|c| c.source_id == "caller-service" && c.target_id == "loadtus-service")
            .expect("the loadtus edge must survive, not resolve as a self-loop");
        assert_eq!(conn.kind, InteractionKind::Business);
        assert!(
            !sdg.connections
                .iter()
                .any(|c| c.source_id == "caller-service" && c.target_id == "caller-service"),
            "another service's localhost address must never read as self"
        );
    }

    #[test]
    fn non_reflexive_calls_still_never_match_their_own_service() {
        // Guards the other half of the flip: a normal cross-service call must
        // not start matching endpoints inside its own service.
        let builder = SdgBuilderImpl::new();
        let sample_data = sample_data();
        let configuration = sample_configuration();
        let sdg = builder
            .build(&sample_data.0, &sample_data.1, &configuration, &vec![])
            .expect("build must succeed");

        assert!(
            sdg.connections.iter().all(|c| c.source_id != c.target_id),
            "no self-loops should appear in the plain cross-service sample"
        );
    }
}
