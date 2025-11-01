use models::{CallStatement, Callable, ConfigurationData, Import};

use crate::{
    errors::builder::BuilderError,
    imcg::model::{Call, IMCG},
};

pub trait ImcgBuilder {
    fn build(
        &self,
        callables: Vec<Callable>,
        call_statements: Vec<CallStatement>,
        imports: Vec<Import>,
    ) -> Result<IMCG, BuilderError>;
}

pub struct ImcgBuilderImpl {}

impl ImcgBuilderImpl {
    pub fn new() -> Self {
        Self {}
    }
}

impl ImcgBuilder for ImcgBuilderImpl {
    fn build(
        &self,
        callables: Vec<Callable>,
        call_statements: Vec<CallStatement>,
        imports: Vec<Import>,
    ) -> Result<IMCG, BuilderError> {
        todo!()
    }
}
