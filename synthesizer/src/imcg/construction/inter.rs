use std::collections::HashMap;

use models::{CallStatement, Callable, configuration::ServiceDescription};

use crate::{
    errors::builder::BuilderError,
    imcg::{
        construction::{intra::CallGraphBuilderImpl, map::get_callables_map},
        model::{Call, Imcg, ServiceCallable},
    },
    sdg::model::{MessageRequest, Request, Sdg},
    utils::assign_service_description_to_file,
};

pub trait ImcgBuilder {
    fn build(
        &self,
        callables: &[Callable],
        call_statements: &[CallStatement],
        service_descs: &[ServiceDescription],
        sdg: &Sdg,
    ) -> Result<Imcg, BuilderError>;
}

pub struct ImcgBuilderImpl {}

impl Default for ImcgBuilderImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl ImcgBuilderImpl {
    pub fn new() -> Self {
        Self {}
    }

    fn get_service_callables(
        &self,
        callables: &[Callable],
        service_descs: &[ServiceDescription],
    ) -> Vec<ServiceCallable> {
        callables
            .iter()
            .map(|callable| {
                let service_desc =
                    assign_service_description_to_file(&callable.file_path, service_descs);
                ServiceCallable::new(callable.to_owned(), service_desc.name)
            })
            .collect()
    }

    fn create_imcg_calls(
        &self,
        sdg: &Sdg,
        callables_map: &HashMap<String, ServiceCallable>,
    ) -> Result<Vec<Call>, BuilderError> {
        let mut calls: Vec<Call> = sdg
            .connections
            .iter()
            .flat_map(|connection| connection.requests.iter())
            .map(|request| self.create_call_from_request(request, callables_map))
            .collect::<Result<_, _>>()?;

        let mut message_calls: Vec<Call> = sdg
            .message_connections
            .iter()
            .flat_map(|connection| connection.messages.iter())
            .map(|message| self.create_call_from_message_request(message, callables_map))
            .collect::<Result<_, _>>()?;

        calls.append(&mut message_calls);
        Ok(calls)
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

        Ok(Call::from_request(
            restcall.callable.signature.clone(),
            endpoint.callable.signature.clone(),
            request.clone(),
        ))
    }

    fn create_call_from_message_request(
        &self,
        message: &MessageRequest,
        callables_map: &HashMap<String, ServiceCallable>,
    ) -> Result<Call, BuilderError> {
        let consumer = callables_map
            .get(&message.consumer.function_hash)
            .ok_or_else(|| {
                BuilderError::Error(format!(
                    "Missing consumer function: {:?}",
                    message.consumer
                ))
            })?;

        let producer = callables_map
            .get(&message.producer.function_hash)
            .ok_or_else(|| {
                BuilderError::Error(format!(
                    "Missing producer function: {:?}",
                    message.producer
                ))
            })?;

        Ok(Call::from_message_request(
            producer.callable.signature.clone(),
            consumer.callable.signature.clone(),
            message.clone(),
        ))
    }
}

impl ImcgBuilder for ImcgBuilderImpl {
    fn build(
        &self,
        callables: &[Callable],
        call_statements: &[CallStatement],
        service_descs: &[ServiceDescription],
        sdg: &Sdg,
    ) -> Result<Imcg, BuilderError> {
        let service_callables = self.get_service_callables(callables, service_descs);
        let callables_map = get_callables_map(&service_callables);
        let cg_builder = CallGraphBuilderImpl::new();
        let intra_cg = cg_builder.build(&service_callables, &callables_map, call_statements)?;

        let mut imcg_calls = self.create_imcg_calls(sdg, &callables_map)?;
        let mut merged_calls = intra_cg.calls;
        merged_calls.append(&mut imcg_calls);

        Ok(Imcg::new(intra_cg.callables, merged_calls))
    }
}
