use std::collections::HashMap;
use std::collections::HashSet;

use models::{CallStatement, Callable, ResolvedCallEdge, configuration::ServiceDescription};

use crate::{
    errors::builder::BuilderError,
    imcg::{
        construction::{intra::CallGraphBuilderImpl, map::get_callables_map},
        model::{Call, Imcg, ServiceCallable},
    },
    sdg::model::{Request, Sdg},
    utils::assign_service_description_to_file,
};

/// Builds an inter-service method call graph from syntactic and resolved static edges.
pub trait ImcgBuilder {
    /// Produces an IMCG, merging external resolved edges with existing syntactic and service edges.
    fn build(
        &self,
        callables: &[Callable],
        call_statements: &[CallStatement],
        resolved_call_edges: &[ResolvedCallEdge],
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
    /// Creates the stateless default IMCG builder.
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
        sdg.connections
            .iter()
            .flat_map(|connection| connection.requests.iter())
            .map(|request| self.create_call_from_request(request, callables_map))
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
        callables: &[Callable],
        call_statements: &[CallStatement],
        resolved_call_edges: &[ResolvedCallEdge],
        service_descs: &[ServiceDescription],
        sdg: &Sdg,
    ) -> Result<Imcg, BuilderError> {
        let service_callables = self.get_service_callables(callables, service_descs);
        let callables_map = get_callables_map(&service_callables);
        let cg_builder = CallGraphBuilderImpl::new();
        let intra_cg = cg_builder.build(&service_callables, &callables_map, call_statements)?;

        let mut imcg_calls = self.create_imcg_calls(sdg, &callables_map)?;
        let mut merged_calls = intra_cg.calls;
        // Add only known and distinct provider edges; unresolved or duplicate data cannot
        // introduce speculative relationships into the reconstructed architecture.
        let mut seen_resolved = HashSet::new();
        for edge in resolved_call_edges {
            if service_callables
                .iter()
                .any(|c| c.callable.signature == edge.source_id)
                && service_callables
                    .iter()
                    .any(|c| c.callable.signature == edge.target_id)
                && seen_resolved.insert((edge.source_id.clone(), edge.target_id.clone()))
            {
                merged_calls.push(Call::new(
                    edge.source_id.clone(),
                    edge.target_id.clone(),
                    None,
                ));
            }
        }
        merged_calls.append(&mut imcg_calls);

        Ok(Imcg::new(intra_cg.callables, merged_calls))
    }
}
