pub mod connectors;
pub mod contextmap;
pub mod errors;
pub mod imcg;
pub mod sdg;
pub mod utils;

pub use contextmap::builder::{ContextMapBuilder, ContextMapBuilderImpl};
pub use imcg::construction::inter::{ImcgBuilder, ImcgBuilderImpl};
pub use sdg::builder::{SdgBuilder, SdgBuilderImpl};

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
    let sdg_builder = SdgBuilderImpl::new();
    sdg_builder
        .build(
            &all_code_elements.endpoints,
            &all_code_elements.restcalls,
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
