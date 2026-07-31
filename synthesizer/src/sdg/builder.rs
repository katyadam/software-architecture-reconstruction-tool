use std::{
    borrow::Cow,
    cmp::{Reverse, min},
    collections::HashMap,
};

use models::{ConfigurationData, Endpoint, RestCall, configuration::ServiceDescription};
use regex::Regex;
use strsim::levenshtein;

use crate::{
    connectors::dto::Constant,
    errors::builder::BuilderError,
    sdg::interaction_kind::{InteractionKind, InteractionSignals, classify, is_reflexive},
    sdg::model::{AssignedEndpoint, AssignedRestCall, Connection, Request, Sdg, Service},
    utils::assign_service_description_to_file,
};

pub trait SdgBuilder {
    fn build(
        &self,
        endpoints: &[Endpoint],
        restcalls: &[RestCall],
        configuration: &ConfigurationData,
        constants: &[Constant],
    ) -> Result<Sdg, BuilderError>;
}

pub struct SdgBuilderImpl {}

impl SdgBuilder for SdgBuilderImpl {
    fn build(
        &self,
        endpoints: &[Endpoint],
        restcalls: &[RestCall],
        configuration: &ConfigurationData,
        constants: &[Constant],
    ) -> Result<Sdg, BuilderError> {
        let assigned_endpoints =
            self.get_assigned_endpoints(endpoints, &configuration.service_descriptions);
        let services = self
            .map_endpoints_to_services(&assigned_endpoints, &configuration.service_descriptions);

        let mut assigned_restcalls =
            self.get_assigned_restcalls(restcalls, &configuration.service_descriptions);
        self.substitute_constants_in_restcalls(&mut assigned_restcalls, constants)?;

        // Every URL of every configured service, flattened -- lets is_reflexive
        // tell "nobody claims this authority" (loopback fallback applies) apart
        // from "another service owns this authority" (never reflexive), even
        // when that other service happens to be configured on localhost.
        let all_urls: Vec<String> = configuration
            .service_descriptions
            .iter()
            .flat_map(|service| service.urls.iter().cloned())
            .collect();

        let connections =
            self.create_connections(assigned_endpoints, assigned_restcalls, &all_urls);
        Ok(Sdg {
            services,
            connections,
        })
    }
}

impl Default for SdgBuilderImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl SdgBuilderImpl {
    pub fn new() -> Self {
        Self {}
    }

    const DISSIMILARITY_PERCENT: f32 = 0.3;

    fn get_assigned_endpoints(
        &self,
        endpoints: &[Endpoint],
        service_descs: &[ServiceDescription],
    ) -> Vec<AssignedEndpoint> {
        endpoints
            .iter()
            .map(|endpoint| {
                let service_desc =
                    assign_service_description_to_file(&endpoint.file_path, service_descs);
                AssignedEndpoint::new(endpoint.clone(), service_desc)
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

    fn get_assigned_restcalls(
        &self,
        restcalls: &[RestCall],
        service_descs: &[ServiceDescription],
    ) -> Vec<AssignedRestCall> {
        restcalls
            .iter()
            .map(|restcall| {
                let service_desc =
                    assign_service_description_to_file(&restcall.file_path, service_descs);
                AssignedRestCall::new(restcall.clone(), service_desc)
            })
            .collect()
    }

    fn substitute_constants_in_restcalls(
        &self,
        restcalls: &mut [AssignedRestCall],
        constants: &[Constant],
    ) -> Result<(), BuilderError> {
        if constants.is_empty() {
            return Ok(());
        }

        let mut sorted_constants = constants.to_vec();
        sorted_constants.sort_by_key(|b| Reverse(b.name.len()));

        let lookup_table: HashMap<&str, &str> = sorted_constants
            .iter()
            .map(|constant| (constant.name.as_str(), constant.value.as_str()))
            .collect();

        let pattern = sorted_constants
            .iter()
            .map(|constant| regex::escape(&constant.name))
            .collect::<Vec<String>>()
            .join("|");
        let re = Regex::new(&pattern)
            .map_err(|_| BuilderError::Error("Failed to compile constants regex".to_owned()))?;

        for restcall in restcalls {
            let substituted =
                re.replace_all(&restcall.data.target_uri, |caps: &regex::Captures| {
                    if let Some(value) = lookup_table.get(&caps[0]) {
                        value.to_string()
                    } else {
                        caps[0].to_string()
                    }
                });
            if let Cow::Owned(changed_uri) = substituted {
                restcall.data.target_uri = changed_uri;
            }
        }

        Ok(())
    }

    fn create_connections(
        &self,
        endpoints: Vec<AssignedEndpoint>,
        restcalls: Vec<AssignedRestCall>,
        all_urls: &[String],
    ) -> Vec<Connection> {
        let restcall_endpoint: Vec<(AssignedRestCall, AssignedEndpoint)> =
            self.create_endpoint_restcall_pairs(endpoints, restcalls, all_urls);

        let mut connections_map: HashMap<String, Connection> = HashMap::new();

        for (restcall, endpoint) in restcall_endpoint {
            // Prefer the matched endpoint's path; fall back to the raw target
            // when matching produced none. The health rule reads the last
            // segment, so it works on a full URL too.
            let target_path = if endpoint.data.uri.is_empty() {
                restcall.data.target_uri.as_str()
            } else {
                endpoint.data.uri.as_str()
            };
            let kind = classify(
                &InteractionSignals {
                    caller_file: &restcall.data.file_path,
                    target_path,
                    target_uri: &restcall.data.target_uri,
                },
                &restcall.service.urls,
                all_urls,
            );

            connections_map
                .entry(format!(
                    "{}__{}",
                    restcall.service.name, endpoint.service.name
                ))
                .or_insert_with(|| Connection {
                    source_id: restcall.service.name.clone(),
                    target_id: endpoint.service.name.clone(),
                    requests: Vec::new(),
                    kind: InteractionKind::default(),
                })
                .requests
                .push(Request {
                    endpoint: endpoint.data.clone(),
                    restcall: restcall.data.clone(),
                    kind,
                });
        }

        let mut connections: Vec<Connection> = connections_map.into_values().collect();
        interaction_kind_rollup_connection(&mut connections);
        connections
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
        all_urls: &[String],
    ) -> Vec<(AssignedRestCall, AssignedEndpoint)> {
        let mut restcall_endpoint: Vec<(AssignedRestCall, AssignedEndpoint)> = Vec::new();
        for restcall in restcalls {
            // A reflexive call (localhost, or the caller's own configured host)
            // must resolve *inside* its own service. Without this the matcher
            // skips own-service endpoints and fuzzy-matches a peer that happens
            // to share a port -- the empaia mds -> app false positive.
            let want_self =
                is_reflexive(&restcall.data.target_uri, &restcall.service.urls, all_urls);

            let mut matched_endpoint: Option<&AssignedEndpoint> = None;
            let mut min_dist = usize::MAX;
            let mut length_of_longest_str = 0;
            let mut was_matched_exactly = false;
            for endpoint in &endpoints {
                if endpoint.data.http_method != restcall.data.http_method
                    || (endpoint.service.name == restcall.service.name) != want_self
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

fn interaction_kind_rollup_connection(connections: &mut Vec<Connection>) {
    for connection in connections {
        connection.kind = InteractionKind::rollup(connection.requests.iter().map(|r| r.kind));
    }
}
