use models::{Callable, ConfigurationData};

use crate::{
    errors::builder::BuilderError,
    imcg::model::{Call, IMCG},
};

pub trait ImcgBuilder {
    fn build(
        &self,
        callables: Vec<Callable>,
        calls: Vec<Call>,
        configuration: ConfigurationData,
    ) -> Result<IMCG, BuilderError>;
}

pub struct ImcgBuilderImpl {}

impl ImcgBuilderImpl {
    pub fn new() -> Self {
        Self {}
    }
}
