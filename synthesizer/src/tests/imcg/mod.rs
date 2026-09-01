#[cfg(test)]
mod tests {
    use models::{
        Argument, CallStatement, Callable, ConfigurationData, Endpoint, HttpMethod, Namespace,
        Parameter, RestCall, configuration::ServiceDescription,
    };

    use crate::{
        imcg::construction::inter::{ImcgBuilder, ImcgBuilderImpl},
        sdg::builder::{SdgBuilder, SdgBuilderImpl},
    };

    #[test]
    fn should_connect_restcall_endpoint_pairs_as_calls() {
        let imcg_builder = ImcgBuilderImpl::new();
        let sdg_builder = SdgBuilderImpl::new();
        let callables = sample_callables();
        let call_statements = sample_call_statements();
        let service_descs = sample_service_descriptions();
        let endpoints = sample_endpoints();
        let restcalls = sample_restcalls();

        let sdg = sdg_builder
            .build(
                &endpoints,
                &restcalls,
                &[],
                &ConfigurationData {
                    service_descriptions: service_descs.clone(),
                },
                &[],
            )
            .expect("SDG in IMCG tests should be valid and buildable!");

        let imcg = imcg_builder
            .build(&callables, &call_statements, &service_descs, &sdg)
            .expect("IMCG building should pass!");

        assert_eq!(imcg.calls.len(), 1, "There should be exactly 1 IMCG call");
    }

    fn sample_service_descriptions() -> Vec<ServiceDescription> {
        vec![
            ServiceDescription {
                name: "ServiceA".to_string(),
                base_dir_path: "serviceA/".to_string(),
                urls: vec!["http://localhost:8000".to_string()],
            },
            ServiceDescription {
                name: "ServiceB".to_string(),
                base_dir_path: "serviceB/".to_string(),
                urls: vec!["http://localhost:7000".to_string()],
            },
        ]
    }

    fn sample_callables() -> Vec<Callable> {
        vec![
            Callable {
                name: "create_item(item:ItemCreate)".to_string(),
                signature: "module:serviceA/main.py/create_item(item:ItemCreate)".to_string(),
                namespace: Namespace::Module("serviceA/main.py".to_string()),
                parameters: vec![Parameter {
                    name: "item".to_string(),
                    datatype: Some("ItemCreate".to_string()),
                    initial_value: None,
                }],
                return_type: None,
                is_async: true,
                is_constructor: false,
                hash: "912172a93ead73ec76396b25cca3afc21232de1fd4bf9611f0cbedca05089c17"
                    .to_string(),
                file_path: "serviceA/main.py".to_string(),
            },
            Callable {
                name: "proxy_create_item(data:ProxyItemCreate)".to_string(),
                signature: "module:serviceB/main.py/proxy_create_item(data:ProxyItemCreate)"
                    .to_string(),
                namespace: Namespace::Module("serviceB/main.py".to_string()),
                parameters: vec![Parameter {
                    name: "data".to_string(),
                    datatype: Some("ProxyItemCreate".to_string()),
                    initial_value: None,
                }],
                return_type: None,
                is_async: true,
                is_constructor: false,
                hash: "326415f3a09d55a0c9bf890ffdaeb12ae7b41cb3653e974f4296e110f343d2b2"
                    .to_string(),
                file_path: "serviceB/main.py".to_string(),
            },
        ]
    }

    fn sample_call_statements() -> Vec<CallStatement> {
        vec![CallStatement {
            function_name: "client.post".to_string(),
            arguments: vec![
                Argument {
                    assigned_variable: "".to_string(),
                    value: "f\"{BASE_URL}/items/\"".to_string(),
                    datatype: None,
                },
                Argument {
                    assigned_variable: "json".to_string(),
                    value: "data.dict".to_string(),
                    datatype: None,
                },
            ],
            enclosing_function_name: Some("proxy_create_item(data: ProxyItemCreate)".to_string()),
            enclosing_class_name: None,
            enclosing_function_hash: Some(
                "326415f3a09d55a0c9bf890ffdaeb12ae7b41cb3653e974f4296e110f343d2b2".to_string(),
            ),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        }]
    }

    fn sample_restcalls() -> Vec<RestCall> {
        vec![RestCall {
            function_name: "proxy_create_item(data: ProxyItemCreate)".to_string(),
            function_hash: "326415f3a09d55a0c9bf890ffdaeb12ae7b41cb3653e974f4296e110f343d2b2"
                .to_string(),
            call_arguments: vec![Argument {
                assigned_variable: "json".to_string(),
                value: "data.dict()".to_string(),
                datatype: None,
            }],
            http_method: HttpMethod::POST,
            target_uri: "http://localhost:8000/items/".to_string(),
            file_path: "serviceB/main.py".to_string(),
        }]
    }

    fn sample_endpoints() -> Vec<Endpoint> {
        vec![
            Endpoint {
                function_name: "create_item".to_string(),
                function_hash: "912172a93ead73ec76396b25cca3afc21232de1fd4bf9611f0cbedca05089c17"
                    .to_string(),
                http_method: HttpMethod::POST,
                parameters: vec![Parameter {
                    name: "item".to_string(),
                    datatype: Some("ItemCreate".to_string()),
                    initial_value: None,
                }],
                uri: "/items/".to_string(),
                file_path: "serviceA/main.py".to_string(),
                ..Default::default()
            },
            Endpoint {
                function_name: "proxy_create_item".to_string(),
                function_hash: "326415f3a09d55a0c9bf890ffdaeb12ae7b41cb3653e974f4296e110f343d2b2"
                    .to_string(),
                http_method: HttpMethod::POST,
                parameters: vec![Parameter {
                    name: "data".to_string(),
                    datatype: Some("ProxyItemCreate".to_string()),
                    initial_value: None,
                }],
                uri: "/proxy-items/".to_string(),
                file_path: "serviceB/main.py".to_string(),
                ..Default::default()
            },
        ]
    }
}
