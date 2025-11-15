use models::{CallStatement, Callable, Import, configuration::ServiceDescription};

use crate::{
    errors::builder::BuilderError,
    imcg::{
        construction::intra::CallGraphBuilderImpl,
        model::{IMCG, ServiceCallable},
    },
    utils::assign_service_description_to_file,
};

pub trait ImcgBuilder {
    fn build(
        &self,
        callables: Vec<Callable>,
        call_statements: Vec<CallStatement>,
        imports: Vec<Import>,
        service_descs: Vec<ServiceDescription>,
    ) -> Result<IMCG, BuilderError>;
}

pub struct ImcgBuilderImpl {}

impl ImcgBuilderImpl {
    pub fn new() -> Self {
        Self {}
    }

    fn get_service_callables(
        &self,
        callables: Vec<Callable>,
        service_descs: Vec<ServiceDescription>,
    ) -> Vec<ServiceCallable> {
        callables
            .iter()
            .map(|callable| {
                let service_desc =
                    assign_service_description_to_file(&callable.file_path, &service_descs);
                ServiceCallable::new(callable.to_owned(), service_desc.name)
            })
            .collect()
    }
}

impl ImcgBuilder for ImcgBuilderImpl {
    fn build(
        &self,
        callables: Vec<Callable>,
        call_statements: Vec<CallStatement>,
        imports: Vec<Import>,
        service_descs: Vec<ServiceDescription>,
    ) -> Result<IMCG, BuilderError> {
        let service_callables = self.get_service_callables(callables, service_descs);
        let cg_builder = CallGraphBuilderImpl::new();
        let intra_cg = cg_builder.build(service_callables, call_statements, imports)?;
        Ok(IMCG::new(intra_cg.callables, intra_cg.calls))
    }
}
