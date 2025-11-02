use models::{CallStatement, Callable, Import, configuration::ServiceDescription};

use crate::{errors::builder::BuilderError, imcg::model::IMCG};

pub trait ImcgBuilder {
    fn build(
        &self,
        callables: Vec<Callable>,
        call_statements: Vec<CallStatement>,
        imports: Vec<Import>,
        service_descriptions: Vec<ServiceDescription>,
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
        service_descriptions: Vec<ServiceDescription>,
    ) -> Result<IMCG, BuilderError> {
        todo!()
    }
}
