use models::Callable;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::sdg::model::Request;

mod bolt;

#[derive(ToSchema, Debug, Serialize, Deserialize)]
pub struct IMCG {
    callables: Vec<Callable>,
    calls: Vec<Call>,
}

#[derive(ToSchema, Debug, Serialize, Deserialize)]
pub struct Call {
    source_id: String,
    target_id: String,
    request: Option<Request>,
}
