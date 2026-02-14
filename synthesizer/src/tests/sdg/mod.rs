#[cfg(test)]
mod tests {
    use models::{
        ConfigurationData, Endpoint, HttpMethod, RestCall, configuration::ServiceDescription,
    };
    use strsim::levenshtein;

    use crate::sdg::builder::{SdgBuilder, SdgBuilderImpl};

    #[test]
    fn should_create_simple_sdg() {
        let service = SdgBuilderImpl::new();
        let sample_data = sample_data();
        let configuration = sample_configuration();
        let sdg = service
            .build(sample_data.0, sample_data.1, configuration, &vec![])
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
                },
                Endpoint {
                    function_name: "create_user".to_string(),
                    function_hash: "some-random-hash".to_string(),

                    parameters: vec![],
                    http_method: HttpMethod::POST,
                    uri: "/users".to_string(),
                    file_path: "crm/user-service/src/api/controller.py".to_string(),
                },
                Endpoint {
                    function_name: "delete_user".to_string(),
                    function_hash: "some-random-hash".to_string(),

                    parameters: vec![],
                    http_method: HttpMethod::DELETE,
                    uri: "/users/{id}".to_string(),
                    file_path: "crm/user-service/src/api/controller.py".to_string(),
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
                },
                RestCall {
                    function_name: "create_user".to_string(),
                    function_hash: "some-random-hash".to_string(),

                    call_arguments: vec![],
                    http_method: HttpMethod::POST,
                    target_uri: "http://user-service:8000/users".to_string(),
                    file_path: "crm/admin-user-service/src/api/user_connector.py".to_string(),
                },
                RestCall {
                    function_name: "delete_user".to_string(),
                    function_hash: "some-random-hash".to_string(),

                    call_arguments: vec![],
                    http_method: HttpMethod::DELETE,
                    target_uri: "http://user-service:8000/users/{id}".to_string(),
                    file_path: "crm/admin-user-service/src/api/user_connector.py".to_string(),
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
}
