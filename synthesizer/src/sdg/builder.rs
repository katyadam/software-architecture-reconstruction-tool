use std::{
    collections::HashMap,
    i32::{self},
};

use models::{ConfigurationData, Endpoint, RestCall, configuration::ServiceDescription};
use strsim::levenshtein;

use crate::{
    errors::builder::BuilderError,
    sdg::model::types::{AssignedEndpoint, AssignedRestCall, Connection, Request, SDG, Service},
};

pub trait SdgBuilder {
    fn build(
        &self,
        endpoints: Vec<Endpoint>,
        restcalls: Vec<RestCall>,
        configuration: ConfigurationData,
    ) -> Result<SDG, BuilderError>;
}

pub struct SdgBuilderImpl {}

impl SdgBuilder for SdgBuilderImpl {
    fn build(
        &self,
        endpoints: Vec<Endpoint>,
        restcalls: Vec<RestCall>,
        configuration: ConfigurationData,
    ) -> Result<SDG, BuilderError> {
        let assigned_endpoints =
            self.get_assigned_endpoints(endpoints, &configuration.service_descriptions);
        let services = self.map_endpoints_to_services(&assigned_endpoints);

        let assigned_restcalls =
            self.get_assigned_restcalls(restcalls, &configuration.service_descriptions);
        let connections = self.create_connections(assigned_endpoints, assigned_restcalls);
        Ok(SDG {
            services,
            connections,
        })
    }
}

impl SdgBuilderImpl {
    pub fn new() -> Self {
        Self {}
    }

    const DISSIMILARITY_PERCENT: f32 = 0.3;

    fn assign_service_description_to_file(
        &self,
        file_name: &str,
        service_descs: &[ServiceDescription],
    ) -> ServiceDescription {
        service_descs
            .iter()
            .find(|sd| file_name.starts_with(&sd.base_dir_path))
            .cloned()
            .unwrap_or_default()
    }

    fn get_assigned_endpoints(
        &self,
        endpoints: Vec<Endpoint>,
        service_descs: &[ServiceDescription],
    ) -> Vec<AssignedEndpoint> {
        endpoints
            .into_iter()
            .map(|endpoint| {
                let service_desc =
                    self.assign_service_description_to_file(&endpoint.file_path, service_descs);
                AssignedEndpoint::new(endpoint, service_desc)
            })
            .collect()
    }

    fn get_assigned_restcalls(
        &self,
        restcalls: Vec<RestCall>,
        service_descs: &[ServiceDescription],
    ) -> Vec<AssignedRestCall> {
        restcalls
            .into_iter()
            .map(|restcall| {
                let service_desc =
                    self.assign_service_description_to_file(&restcall.file_path, service_descs);
                AssignedRestCall::new(restcall, service_desc)
            })
            .collect()
    }

    fn map_endpoints_to_services(&self, endpoints: &[AssignedEndpoint]) -> Vec<Service> {
        let mut service_map: HashMap<String, Service> = HashMap::new();

        for endpoint in endpoints {
            service_map
                .entry(endpoint.service.name.clone())
                .or_insert_with(|| Service {
                    name: endpoint.service.name.clone(),
                    endpoints: Vec::new(),
                    urls: endpoint.service.urls.clone(),
                })
                .endpoints
                .push(endpoint.data.clone());
        }

        service_map.into_values().collect()
    }

    fn create_connections(
        &self,
        endpoints: Vec<AssignedEndpoint>,
        restcalls: Vec<AssignedRestCall>,
    ) -> Vec<Connection> {
        let restcall_endpoint: Vec<(AssignedRestCall, AssignedEndpoint)> =
            self.create_endpoint_restcall_pairs(endpoints, restcalls);

        let mut connections_map: HashMap<String, Connection> = HashMap::new();

        for (restcall, endpoint) in restcall_endpoint {
            connections_map
                .entry(format!(
                    "{}__{}",
                    restcall.service.name, endpoint.service.name
                ))
                .or_insert_with(|| Connection {
                    source_id: restcall.service.name.clone(),
                    target_id: endpoint.service.name.clone(),
                    requests: Vec::new(),
                })
                .requests
                .push(Request {
                    endpoint: endpoint.data.clone(),
                    restcall: restcall.data.clone(),
                });
        }

        connections_map.into_values().collect()
    }

    fn create_endpoint_restcall_pairs(
        &self,
        endpoints: Vec<AssignedEndpoint>,
        restcalls: Vec<AssignedRestCall>,
    ) -> Vec<(AssignedRestCall, AssignedEndpoint)> {
        let mut restcall_endpoint: Vec<(AssignedRestCall, AssignedEndpoint)> = Vec::new();
        for restcall in restcalls {
            let mut matched_endpoint: Option<&AssignedEndpoint> = None;
            let mut min_dist = i32::MAX;
            let mut length_of_longest_str = 0;

            for endpoint in &endpoints {
                if endpoint.data.http_method != restcall.data.http_method
                    || endpoint.service.name == restcall.service.name
                {
                    continue;
                }
                // TODO: Should be introduced compare of domains where endpoints lives and from restcall calls
                let cur_dist = levenshtein(&endpoint.data.uri, &restcall.data.target_uri) as i32;
                if cur_dist < min_dist {
                    min_dist = cur_dist;
                    matched_endpoint = Some(&endpoint);
                    length_of_longest_str =
                        std::cmp::max(endpoint.data.uri.len(), restcall.data.target_uri.len());
                }
            }

            if let Some(endpoint) = matched_endpoint {
                let percent = length_of_longest_str as f32 * Self::DISSIMILARITY_PERCENT;
                if percent > min_dist as f32 {
                    restcall_endpoint.push((restcall, endpoint.to_owned()));
                }
            }
        }
        restcall_endpoint
    }
}
