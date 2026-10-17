use std::{borrow::Cow, cmp::Reverse, collections::HashMap};

use models::{
    ConfigurationData, Endpoint, MessageDestinationKind, MessageEdge, MessageRole, RestCall,
    configuration::ServiceDescription,
};
use regex::Regex;

use crate::{
    connectors::dto::Constant,
    errors::builder::BuilderError,
    sdg::interaction_kind::{InteractionKind, InteractionSignals, classify},
    sdg::matching::{MatchTag, candidates},
    sdg::model::{
        AssignedEndpoint, AssignedMessageEdge, AssignedRestCall, Connection, MessageConnection,
        MessageRequest, Request, Sdg, Service,
    },
    utils::assign_service_description_to_file,
};

pub trait SdgBuilder {
    fn build(
        &self,
        endpoints: &[Endpoint],
        restcalls: &[RestCall],
        message_edges: &[MessageEdge],
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
        message_edges: &[MessageEdge],
        configuration: &ConfigurationData,
        constants: &[Constant],
    ) -> Result<Sdg, BuilderError> {
        self.build_with_unmatched(
            endpoints,
            restcalls,
            message_edges,
            configuration,
            constants,
        )
        .map(|(sdg, _)| sdg)
    }
}

/// REST call that matched no endpoint, after constant substitution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmatchedCall {
    /// Calling service name.
    pub source: String,
    pub restcall: RestCall,
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

    /// Like [`SdgBuilder::build`], plus the calls with no candidate endpoint.
    pub fn build_with_unmatched(
        &self,
        endpoints: &[Endpoint],
        restcalls: &[RestCall],
        message_edges: &[MessageEdge],
        configuration: &ConfigurationData,
        constants: &[Constant],
    ) -> Result<(Sdg, Vec<UnmatchedCall>), BuilderError> {
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

        let (connections, unmatched) =
            self.create_connections(assigned_endpoints, assigned_restcalls, &all_urls);
        let assigned_message_edges =
            self.get_assigned_message_edges(message_edges, &configuration.service_descriptions);
        let message_connections = self.create_message_connections(assigned_message_edges);
        let sdg = Sdg {
            services,
            connections,
            message_connections,
        };
        Ok((sdg, unmatched))
    }

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

    fn get_assigned_message_edges(
        &self,
        message_edges: &[MessageEdge],
        service_descs: &[ServiceDescription],
    ) -> Vec<AssignedMessageEdge> {
        message_edges
            .iter()
            .map(|edge| {
                let service_desc =
                    assign_service_description_to_file(&edge.file_path, service_descs);
                AssignedMessageEdge::new(edge.clone(), service_desc)
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

        // Java emits unresolved constants as holes `{HOST}`; bare names need
        // word boundaries so `HOST` never matches inside `HOSTNAME`.
        let names = sorted_constants
            .iter()
            .map(|constant| regex::escape(&constant.name))
            .collect::<Vec<String>>()
            .join("|");
        let pattern = format!(r"\{{(?:{names})\}}|\b(?:{names})\b");
        let re = Regex::new(&pattern)
            .map_err(|_| BuilderError::Error("Failed to compile constants regex".to_owned()))?;

        for restcall in restcalls {
            let substituted =
                re.replace_all(&restcall.data.target_uri, |caps: &regex::Captures| {
                    let key = caps[0].trim_start_matches('{').trim_end_matches('}');
                    if let Some(value) = lookup_table.get(key) {
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
    ) -> (Vec<Connection>, Vec<UnmatchedCall>) {
        let (restcall_endpoint, unmatched) =
            self.create_endpoint_restcall_pairs(endpoints, restcalls, all_urls);

        let mut connections_map: HashMap<String, Connection> = HashMap::new();

        for (restcall, endpoint, match_tag) in restcall_endpoint {
            let kind = request_kind(
                &restcall.data,
                &endpoint.data.uri,
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
                    match_tag,
                });
        }

        let mut connections: Vec<Connection> = connections_map.into_values().collect();
        interaction_kind_rollup_connection(&mut connections);
        (connections, unmatched)
    }

    fn create_message_connections(
        &self,
        message_edges: Vec<AssignedMessageEdge>,
    ) -> Vec<MessageConnection> {
        let producers = message_edges
            .iter()
            .filter(|edge| matches!(edge.data.role, MessageRole::Producer));
        let consumers = message_edges
            .iter()
            .filter(|edge| matches!(edge.data.role, MessageRole::Consumer))
            .collect::<Vec<_>>();

        let mut connections_map: HashMap<String, MessageConnection> = HashMap::new();

        for producer in producers {
            for consumer in &consumers {
                if producer.service.name == consumer.service.name {
                    continue;
                }
                if !message_destinations_match(&producer.data, &consumer.data) {
                    continue;
                }

                connections_map
                    .entry(format!(
                        "{}__{}",
                        producer.service.name, consumer.service.name
                    ))
                    .or_insert_with(|| MessageConnection {
                        source_id: producer.service.name.clone(),
                        target_id: consumer.service.name.clone(),
                        messages: Vec::new(),
                    })
                    .messages
                    .push(MessageRequest {
                        producer: producer.data.clone(),
                        consumer: consumer.data.clone(),
                    });
            }
        }

        connections_map.into_values().collect()
    }

    fn create_endpoint_restcall_pairs(
        &self,
        endpoints: Vec<AssignedEndpoint>,
        restcalls: Vec<AssignedRestCall>,
        all_urls: &[String],
    ) -> (
        Vec<(AssignedRestCall, AssignedEndpoint, MatchTag)>,
        Vec<UnmatchedCall>,
    ) {
        let mut pairs = Vec::new();
        let mut unmatched = Vec::new();
        for restcall in restcalls {
            let found = candidates(&restcall, &endpoints, all_urls);
            if found.is_empty() {
                unmatched.push(UnmatchedCall {
                    source: restcall.service.name.clone(),
                    restcall: restcall.data.clone(),
                });
            }
            for (endpoint, tag) in found {
                pairs.push((restcall.clone(), endpoint.clone(), tag));
            }
        }
        (pairs, unmatched)
    }
}

/// Kind of one request; `own_urls` are the caller's urls.
pub(crate) fn request_kind(
    restcall: &RestCall,
    endpoint_uri: &str,
    own_urls: &[String],
    all_urls: &[String],
) -> InteractionKind {
    // Prefer the matched endpoint's path; fall back to the raw target
    // when matching produced none. The health rule reads the last
    // segment, so it works on a full URL too.
    let target_path = if endpoint_uri.is_empty() {
        restcall.target_uri.as_str()
    } else {
        endpoint_uri
    };
    classify(
        &InteractionSignals {
            caller_file: &restcall.file_path,
            target_path,
            target_uri: &restcall.target_uri,
        },
        own_urls,
        all_urls,
    )
}

pub(crate) fn interaction_kind_rollup_connection(connections: &mut Vec<Connection>) {
    for connection in connections {
        connection.kind = InteractionKind::rollup(connection.requests.iter().map(|r| r.kind));
    }
}

fn message_destinations_match(producer: &MessageEdge, consumer: &MessageEdge) -> bool {
    match producer.destination_kind {
        MessageDestinationKind::Topic => producer.topic.as_ref().is_some_and(|topic| {
            consumer
                .topic
                .as_ref()
                .is_some_and(|consumer_topic| consumer_topic == topic)
        }),
        MessageDestinationKind::Queue => producer
            .queue
            .as_ref()
            .or(producer.routing_key.as_ref())
            .is_some_and(|queue| {
                consumer
                    .queue
                    .as_ref()
                    .is_some_and(|consumer_queue| consumer_queue == queue)
            }),
        MessageDestinationKind::ExchangeRoutingKey => false,
        MessageDestinationKind::Unknown => false,
    }
}
