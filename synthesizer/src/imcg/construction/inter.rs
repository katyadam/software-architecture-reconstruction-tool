use std::collections::HashMap;

use models::{CallStatement, Callable, configuration::ServiceDescription};

use crate::{
    errors::builder::BuilderError,
    imcg::{
        construction::{intra::CallGraphBuilderImpl, map::get_callables_map},
        model::{Call, Imcg, ServiceCallable},
    },
    sdg::model::{Request, Sdg},
    utils::assign_service_description_to_file,
};

pub trait ImcgBuilder {
    fn build(
        &self,
        callables: Vec<Callable>,
        call_statements: Vec<CallStatement>,
        service_descs: Vec<ServiceDescription>,
        sdg: &Sdg,
    ) -> Result<Imcg, BuilderError>;
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

    fn create_imcg_calls(
        &self,
        sdg: &Sdg,
        callables_map: &HashMap<String, ServiceCallable>,
    ) -> Result<Vec<Call>, BuilderError> {
        sdg.connections
            .iter()
            .flat_map(|conn| conn.requests.iter())
            .map(|rq| self.create_call_from_request(rq, callables_map))
            .collect()
    }

    fn create_call_from_request(
        &self,
        request: &Request,
        callables_map: &HashMap<String, ServiceCallable>,
    ) -> Result<Call, BuilderError> {
        let endpoint = callables_map
            .get(&request.endpoint.function_hash)
            .ok_or_else(|| {
                BuilderError::Error(format!("Missing endpoint function: {:?}", request.endpoint))
            })?;

        let restcall = callables_map
            .get(&request.restcall.function_hash)
            .ok_or_else(|| {
                BuilderError::Error(format!("Missing restcall function: {:?}", request.restcall))
            })?;

        Ok(Call::new(
            restcall.callable.signature.clone(),
            endpoint.callable.signature.clone(),
            Some(request.clone()),
        ))
    }
}

impl ImcgBuilder for ImcgBuilderImpl {
    fn build(
        &self,
        callables: Vec<Callable>,
        call_statements: Vec<CallStatement>,
        service_descs: Vec<ServiceDescription>,
        sdg: &Sdg,
    ) -> Result<Imcg, BuilderError> {
        let service_callables = self.get_service_callables(callables, service_descs);
        let callables_map = get_callables_map(&service_callables);
        let cg_builder = CallGraphBuilderImpl::new();
        let intra_cg = cg_builder.build(service_callables, &callables_map, call_statements)?;

        let mut imcg_calls = self.create_imcg_calls(sdg, &callables_map)?;
        let mut merged_calls = intra_cg.calls;
        merged_calls.append(&mut imcg_calls);

        Ok(Imcg::new(intra_cg.callables, merged_calls))
    }
}
