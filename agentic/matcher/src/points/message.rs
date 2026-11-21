//! Message edge -> address: RPC for gRPC, channel for brokers.

use agentic_models::Address;
use models::substitute::Substituter;
use models::{CommunicationProtocol, MessageEdge};

/// The destination gets constant substitution first.
pub fn address(edge: &MessageEdge, substituter: &Substituter) -> Address {
    let substituted = substituter.apply(edge.destination.trim());
    let raw = substituted.as_str();
    if raw.is_empty() {
        return Address::Unresolved {
            raw: edge.destination.clone(),
        };
    }
    match edge.protocol {
        CommunicationProtocol::Grpc => {
            let (service, method) = raw.split_once('/').unwrap_or((raw, ""));
            Address::Rpc {
                service: service.to_string(),
                method: method.to_string(),
            }
        }
        CommunicationProtocol::Kafka | CommunicationProtocol::RabbitMq => Address::Channel {
            name: raw.to_string(),
            broker: edge.exchange.clone(),
        },
    }
}
