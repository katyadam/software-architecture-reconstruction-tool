#[cfg(test)]
mod tests {
    use models::{
        CommunicationProtocol, ConfigurationData, Endpoint, HttpMethod, MessageDestinationKind,
        MessageEdge, MessageRole, RestCall, configuration::ServiceDescription,
    };
    use strsim::levenshtein;

    use crate::sdg::builder::{SdgBuilder, SdgBuilderImpl};

    #[test]
    fn should_create_simple_sdg() {
        let service = SdgBuilderImpl::new();
        let sample_data = sample_data();
        let configuration = sample_configuration();
        let sdg = service
            .build(&sample_data.0, &sample_data.1, &[], &configuration, &vec![])
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
    fn should_match_rabbitmq_bindings_to_producers() {
        let builder = SdgBuilderImpl::new();
        let configuration = sample_configuration();
        let producer = MessageEdge {
            protocol: CommunicationProtocol::RabbitMq,
            role: MessageRole::Producer,
            destination_kind: MessageDestinationKind::ExchangeRoutingKey,
            destination: "orders:created".to_string(),
            exchange: Some("orders".to_string()),
            routing_key: Some("created".to_string()),
            queue: None,
            topic: None,
            handler: None,
            function_name: "publish".to_string(),
            function_hash: "producer-hash".to_string(),
            call_arguments: vec![],
            file_path: "crm/admin-user-service/src/api/publisher.py".to_string(),
        };
        let binding = MessageEdge {
            protocol: CommunicationProtocol::RabbitMq,
            role: MessageRole::Binding,
            destination_kind: MessageDestinationKind::ExchangeRoutingKey,
            destination: "orders:created".to_string(),
            exchange: Some("orders".to_string()),
            routing_key: Some("created".to_string()),
            queue: Some("tmp-queue".to_string()),
            topic: None,
            handler: None,
            function_name: "consume".to_string(),
            function_hash: "binding-hash".to_string(),
            call_arguments: vec![],
            file_path: "crm/user-service/src/api/consumer.py".to_string(),
        };

        let sdg = builder
            .build(&[], &[], &[producer, binding], &configuration, &[])
            .expect("message-only SDG should build");

        assert_eq!(sdg.message_connections.len(), 1);
        assert_eq!(sdg.message_connections[0].source_id, "admin-user-service");
        assert_eq!(sdg.message_connections[0].target_id, "user-service");
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
