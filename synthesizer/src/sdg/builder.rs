use std::{cmp::min, collections::HashMap};

use models::{ConfigurationData, Endpoint, RestCall, configuration::ServiceDescription};
use strsim::levenshtein;

use crate::{
    errors::builder::BuilderError,
    sdg::model::{AssignedEndpoint, AssignedRestCall, Connection, Request, Sdg, Service},
    utils::assign_service_description_to_file,
};

pub trait SdgBuilder {
    fn build(
        &self,
        endpoints: Vec<Endpoint>,
        restcalls: Vec<RestCall>,
        configuration: ConfigurationData,
    ) -> Result<Sdg, BuilderError>;
}

pub struct SdgBuilderImpl {}

impl SdgBuilder for SdgBuilderImpl {
    fn build(
        &self,
        endpoints: Vec<Endpoint>,
        restcalls: Vec<RestCall>,
        configuration: ConfigurationData,
    ) -> Result<Sdg, BuilderError> {
        let assigned_endpoints =
            self.get_assigned_endpoints(endpoints, &configuration.service_descriptions);
        let services = self
            .map_endpoints_to_services(&assigned_endpoints, &configuration.service_descriptions);

        let assigned_restcalls =
            self.get_assigned_restcalls(restcalls, &configuration.service_descriptions);
        let connections = self.create_connections(assigned_endpoints, assigned_restcalls);
        Ok(Sdg {
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

    fn get_assigned_endpoints(
        &self,
        endpoints: Vec<Endpoint>,
        service_descs: &[ServiceDescription],
    ) -> Vec<AssignedEndpoint> {
        endpoints
            .into_iter()
            .map(|endpoint| {
                let service_desc =
                    assign_service_description_to_file(&endpoint.file_path, service_descs);
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
                    assign_service_description_to_file(&restcall.file_path, service_descs);
                AssignedRestCall::new(restcall, service_desc)
            })
            .collect()
    }

    fn map_endpoints_to_services(
        &self,
        assigned_endpoints: &[AssignedEndpoint],
        service_descs: &[ServiceDescription],
    ) -> Vec<Service> {
        let mut service_map: HashMap<String, Service> = service_descs
            .iter()
            .map(|s_desc| {
                (
                    s_desc.name.clone(),
                    Service {
                        name: s_desc.name.to_string(),
                        endpoints: Vec::new(),
                        urls: s_desc.urls.clone(),
                    },
                )
            })
            .collect();

        for endpoint in assigned_endpoints {
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

    fn exact_match(&self, endpoint: &AssignedEndpoint, restcall: &AssignedRestCall) -> bool {
        if !endpoint.data.uri.starts_with("http") {
            for url in &endpoint.service.urls {
                let to_match = format!(
                    "{}/{}",
                    url.trim_end_matches("/"),
                    endpoint.data.uri.trim_start_matches("/")
                );
                if to_match == restcall.data.target_uri {
                    return true;
                }
            }
        } else {
            return endpoint.data.uri == restcall.data.target_uri;
        }
        false
    }

    fn get_levenshtein_distance(
        &self,
        endpoint: &AssignedEndpoint,
        restcall: &AssignedRestCall,
    ) -> usize {
        let mut lowest_score = usize::MAX;

        for prefix in endpoint
            .service
            .urls
            .iter()
            .chain(std::iter::once(&"".to_string()))
        {
            let endpoint_test_uri = format!(
                "{}/{}",
                prefix.trim_end_matches("/"),
                endpoint.data.uri.trim_start_matches("/")
            );
            let score = levenshtein(&endpoint_test_uri, &restcall.data.target_uri);
            lowest_score = min(lowest_score, score);
        }

        lowest_score
    }

    fn create_endpoint_restcall_pairs(
        &self,
        endpoints: Vec<AssignedEndpoint>,
        restcalls: Vec<AssignedRestCall>,
    ) -> Vec<(AssignedRestCall, AssignedEndpoint)> {
        let mut restcall_endpoint: Vec<(AssignedRestCall, AssignedEndpoint)> = Vec::new();
        for restcall in restcalls {
            let mut matched_endpoint: Option<&AssignedEndpoint> = None;
            let mut min_dist = usize::MAX;
            let mut length_of_longest_str = 0;
            let mut was_matched_exactly = false;
            for endpoint in &endpoints {
                if endpoint.data.http_method != restcall.data.http_method
                    || endpoint.service.name == restcall.service.name
                {
                    continue;
                }
                if self.exact_match(endpoint, &restcall) {
                    matched_endpoint = Some(endpoint);
                    was_matched_exactly = true;
                    break;
                }

                let cur_dist = self.get_levenshtein_distance(endpoint, &restcall);
                if cur_dist < min_dist {
                    min_dist = cur_dist;
                    matched_endpoint = Some(endpoint);
                    length_of_longest_str =
                        std::cmp::max(endpoint.data.uri.len(), restcall.data.target_uri.len());
                }
            }

            if let Some(endpoint) = matched_endpoint {
                let percent = length_of_longest_str as f32 * Self::DISSIMILARITY_PERCENT;
                if percent > min_dist as f32 || was_matched_exactly {
                    restcall_endpoint.push((restcall, endpoint.to_owned()));
                }
            }
        }
        restcall_endpoint
    }
}
