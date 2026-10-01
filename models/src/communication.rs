use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::Argument;

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Clone)]
pub enum CommunicationProtocol {
    RabbitMq,
    Kafka,
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Clone)]
pub enum MessageRole {
    Producer,
    Consumer,
    QueueDeclaration,
    TopicDeclaration,
    Binding,
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Clone)]
pub enum MessageDestinationKind {
    Queue,
    Topic,
    ExchangeRoutingKey,
    Unknown,
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Clone)]
pub struct MessageEdge {
    pub protocol: CommunicationProtocol,
    pub role: MessageRole,
    pub destination_kind: MessageDestinationKind,
    pub destination: String,
    pub exchange: Option<String>,
    pub routing_key: Option<String>,
    pub queue: Option<String>,
    pub topic: Option<String>,
    pub handler: Option<String>,
    pub function_name: String,
    pub function_hash: String,
    pub call_arguments: Vec<Argument>,
    pub file_path: String,
}

impl MessageEdge {
    pub fn clone_with_resolved_destination(
        &self,
        destination: String,
        topic: Option<String>,
        exchange: Option<String>,
        routing_key: Option<String>,
        queue: Option<String>,
        handler: Option<String>,
    ) -> Self {
        Self {
            protocol: self.protocol.clone(),
            role: self.role.clone(),
            destination_kind: self.destination_kind.clone(),
            destination,
            topic,
            exchange,
            routing_key,
            queue,
            handler,
            function_name: self.function_name.clone(),
            function_hash: self.function_hash.clone(),
            call_arguments: self.call_arguments.clone(),
            file_path: self.file_path.clone(),
        }
    }
}
