use models::Callable;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::sdg::model::{MessageRequest, Request};

mod bolt;

#[derive(ToSchema, Debug, Serialize, Deserialize)]
pub struct Imcg {
    pub callables: Vec<ServiceCallable>,
    pub calls: Vec<Call>,
}

impl Imcg {
    pub fn new(callables: Vec<ServiceCallable>, calls: Vec<Call>) -> Self {
        Self { callables, calls }
    }
}

#[derive(ToSchema, Debug, Serialize, Deserialize, Clone)]
pub struct Call {
    source_id: String,
    target_id: String,
    request: Option<Request>,
    message_request: Option<MessageRequest>,
}

impl Call {
    pub fn new(
        source_id: String,
        target_id: String,
        request: Option<Request>,
        message_request: Option<MessageRequest>,
    ) -> Self {
        Self {
            source_id,
            target_id,
            request,
            message_request,
        }
    }

    pub fn from_request(source_id: String, target_id: String, request: Request) -> Self {
        Self::new(source_id, target_id, Some(request), None)
    }

    pub fn from_message_request(
        source_id: String,
        target_id: String,
        message_request: MessageRequest,
    ) -> Self {
        Self::new(source_id, target_id, None, Some(message_request))
    }
}
#[derive(ToSchema, Debug, Serialize, Deserialize, Clone)]
pub struct ServiceCallable {
    pub callable: Callable,
    pub service_name: String,
}

impl ServiceCallable {
    pub fn new(callable: Callable, service_name: String) -> Self {
        Self {
            callable,
            service_name,
        }
    }
}
