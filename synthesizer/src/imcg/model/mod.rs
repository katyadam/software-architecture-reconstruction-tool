use models::{Callable, configuration::ServiceDescription};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::sdg::model::Request;

mod bolt;

#[derive(ToSchema, Debug, Serialize, Deserialize)]
pub struct IMCG {
    pub callables: Vec<ServiceCallable>,
    pub calls: Vec<Call>,
}

#[derive(ToSchema, Debug, Serialize, Deserialize, Clone)]
pub struct Call {
    source_id: String,
    target_id: String,
    request: Option<Request>,
}

impl Call {
    pub fn new(source_id: String, target_id: String, request: Option<Request>) -> Self {
        Self {
            source_id,
            target_id,
            request,
        }
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
