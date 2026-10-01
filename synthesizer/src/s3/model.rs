use models::{CallStatement, Callable, Endpoint, Entity, MessageEdge, RestCall};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct S3ContextMapCodeElements {
    pub entities: Vec<Entity>,
}

impl S3ContextMapCodeElements {
    pub fn new(entities: Vec<Entity>) -> Self {
        Self { entities }
    }
}

#[derive(Deserialize)]
pub struct S3SdgCodeElements {
    pub endpoints: Vec<Endpoint>,
    pub restcalls: Vec<RestCall>,
    #[serde(default)]
    pub message_edges: Vec<MessageEdge>,
}

impl S3SdgCodeElements {
    pub fn new(
        endpoints: Vec<Endpoint>,
        restcalls: Vec<RestCall>,
        message_edges: Vec<MessageEdge>,
    ) -> Self {
        Self {
            endpoints,
            restcalls,
            message_edges,
        }
    }
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct S3ImcgCodeElements {
    pub callables: Vec<Callable>,
    pub calls: Vec<CallStatement>,
}

#[allow(dead_code)]
impl S3ImcgCodeElements {
    pub fn new(callables: Vec<Callable>, calls: Vec<CallStatement>) -> Self {
        Self { callables, calls }
    }
}
