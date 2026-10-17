pub mod connectors;
pub mod contextmap;
pub mod errors;
pub mod imcg;
// Lib-only (CLI): main.rs mounts `sdg` too and would flag it unused.
#[path = "sdg/jev_match.rs"]
pub mod jev_match;
pub mod sdg;
pub mod utils;

pub use contextmap::builder::{ContextMapBuilder, ContextMapBuilderImpl};
pub use imcg::construction::inter::{ImcgBuilder, ImcgBuilderImpl};
pub use sdg::builder::{SdgBuilder, SdgBuilderImpl, UnmatchedCall};
pub use jev_match::{MatchStats, tie_break};

use crate::connectors::dto::Constant;
use crate::contextmap::model::ContextMap;
use crate::imcg::model::Imcg;
use crate::sdg::model::Sdg;
use models::{CodeElementsAggregate, ConfigurationData};

pub fn direct_cm_build(
    all_code_elements: &CodeElementsAggregate,
    configuration: &ConfigurationData,
) -> ContextMap {
    let cm_builder = ContextMapBuilderImpl::new();
    cm_builder
        .build(
            &all_code_elements.entities,
            &configuration.service_descriptions,
        )
        .unwrap_or_else(|e| panic!("Context Map build failed: {e}"))
}

pub fn direct_sdg_build(
    all_code_elements: &CodeElementsAggregate,
    configuration: &ConfigurationData,
    constants: &[Constant],
) -> Sdg {
    direct_sdg_build_with_unmatched(all_code_elements, configuration, constants).0
}

/// [`direct_sdg_build`] plus the REST calls that matched no endpoint.
pub fn direct_sdg_build_with_unmatched(
    all_code_elements: &CodeElementsAggregate,
    configuration: &ConfigurationData,
    constants: &[Constant],
) -> (Sdg, Vec<UnmatchedCall>) {
    SdgBuilderImpl::new()
        .build_with_unmatched(
            &all_code_elements.endpoints,
            &all_code_elements.restcalls,
            &all_code_elements.message_edges,
            configuration,
            constants,
        )
        .unwrap_or_else(|e| panic!("SDG build failed: {e}"))
}

pub fn direct_imcg_build(
    all_code_elements: &CodeElementsAggregate,
    configuration: &ConfigurationData,
    sdg: &Sdg,
) -> Imcg {
    let imcg_builder = ImcgBuilderImpl::new();
    imcg_builder
        .build(
            &all_code_elements.callables,
            &all_code_elements.call_statements,
            &configuration.service_descriptions,
            sdg,
        )
        .unwrap_or_else(|e| panic!("IMCG build failed: {e}"))
}
