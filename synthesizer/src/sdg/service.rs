use std::{
    collections::HashMap,
    i32::{self},
};

use models::{Endpoint, RestCall};
use strsim::levenshtein;

use crate::sdg::model::{Connection, Request, SDG, Service};

pub const DISSIMILARITY_PERCENT: f32 = 0.3;

pub fn build_sdg(endpoints: Vec<Endpoint>, restcalls: Vec<RestCall>) -> SDG {
    let services = create_service_map(&endpoints);
    let connections = create_connections(&endpoints, &restcalls);
    SDG {
        services,
        connections,
    }
}

fn create_service_map(endpoints: &[Endpoint]) -> Vec<Service> {
    let mut service_map: HashMap<String, Service> = HashMap::new();

    for endpoint in endpoints {
        service_map
            .entry(endpoint.service_name.clone())
            .or_insert_with(|| Service {
                name: endpoint.service_name.clone(),
                endpoints: Vec::new(),
            })
            .endpoints
            .push(endpoint.clone());
    }

    service_map.into_values().collect()
}

fn create_connections(endpoints: &[Endpoint], restcalls: &[RestCall]) -> Vec<Connection> {
    let restcall_endpoint: HashMap<RestCall, &Endpoint> =
        create_endpoint_restcall_pairs(endpoints, restcalls);

    let mut connections_map: HashMap<String, Connection> = HashMap::new();

    for (restcall, endpoint) in restcall_endpoint {
        connections_map
            .entry(format!(
                "{}__{}",
                restcall.service_name, endpoint.service_name
            ))
            .or_insert_with(|| Connection {
                source_id: restcall.service_name.clone(),
                target_id: endpoint.service_name.clone(),
                requests: Vec::new(),
            })
            .requests
            .push(Request {
                endpoint: endpoint.clone(),
                restcall,
            });
    }

    connections_map.into_values().collect()
}

fn create_endpoint_restcall_pairs<'a>(
    endpoints: &'a [Endpoint],
    restcalls: &[RestCall],
) -> HashMap<RestCall, &'a Endpoint> {
    // We are using HashMap instead of Pairs to prevent RestCall matching multiple possible Endpoints
    let mut restcall_endpoint: HashMap<RestCall, &Endpoint> = HashMap::new();
    for restcall in restcalls {
        let mut matched_endpoint: Option<&Endpoint> = None;
        let mut min_dist = i32::MAX;
        let mut length_of_longest_str = 0;

        for endpoint in endpoints {
            if endpoint.http_method != restcall.http_method
                || endpoint.service_name == restcall.service_name
            {
                continue;
            }

            let cur_dist = levenshtein(&endpoint.uri, &restcall.target_uri) as i32;
            if cur_dist < min_dist {
                min_dist = cur_dist;
                matched_endpoint = Some(endpoint);
                length_of_longest_str =
                    std::cmp::max(endpoint.uri.len(), restcall.target_uri.len());
            }
        }

        if let Some(endpoint) = matched_endpoint {
            let percent = length_of_longest_str as f32 * DISSIMILARITY_PERCENT;
            if percent > min_dist as f32 {
                restcall_endpoint.insert(restcall.clone(), endpoint);
            }
        }
    }
    restcall_endpoint
}
