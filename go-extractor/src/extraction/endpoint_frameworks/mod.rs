mod chi;
mod gorilla;
mod serve_mux;
mod shared;
mod web;

use std::collections::HashMap;

use models::{Assignment, AssignmentKey, HttpMethod, Import};
use tree_sitter::Node;

use self::{
    chi::ChiStrategy,
    gorilla::GorillaStrategy,
    serve_mux::{ServeMuxHandleFuncStrategy, ServeMuxHandleStrategy},
    web::WebStrategy,
};

pub(super) struct EndpointMatch {
    pub method: HttpMethod,
    pub path: String,
    pub handler: String,
}

pub(super) struct ExtractParams<'a> {
    pub node: Node<'a>,
    pub code: &'a str,
    pub globals: &'a HashMap<String, String>,
    pub assignments: &'a HashMap<AssignmentKey, Assignment>,
    pub imports: &'a [Import],
}

pub(super) trait EndpointIdentificationStrategy: Sync {
    fn identify(&self, params: &ExtractParams<'_>) -> Option<EndpointMatch>;
}

static GORILLA: GorillaStrategy = GorillaStrategy;
static CHI: ChiStrategy = ChiStrategy;
static SERVE_MUX_HANDLE: ServeMuxHandleStrategy = ServeMuxHandleStrategy;
static SERVE_MUX_HANDLE_FUNC: ServeMuxHandleFuncStrategy = ServeMuxHandleFuncStrategy;
static WEB: WebStrategy = WebStrategy;
static STRATEGIES: &[&dyn EndpointIdentificationStrategy] = &[
    &GORILLA,
    &CHI,
    &SERVE_MUX_HANDLE,
    &SERVE_MUX_HANDLE_FUNC,
    &WEB,
];

pub(super) fn strategies() -> &'static [&'static dyn EndpointIdentificationStrategy] {
    STRATEGIES
}
