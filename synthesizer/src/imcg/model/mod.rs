use models::Callable;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::sdg::model::Request;

mod bolt;

#[derive(ToSchema, Debug, Serialize, Deserialize)]
pub struct IMCG {
    pub callables: Vec<Callable>,
    pub calls: Vec<Call>,
}

#[derive(ToSchema, Debug, Serialize, Deserialize, Clone)]
pub struct Call {
    source_id: String,
    target_id: String,
    request: Option<Request>,
}
