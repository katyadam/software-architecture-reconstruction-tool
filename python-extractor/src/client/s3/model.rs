use models::{CallStatement, Callable, Endpoint, Entity, RestCall};
use serde::Serialize;

#[derive(Serialize)]
pub struct S3ContextMap<'a> {
    entities: &'a Vec<Entity>,
}

impl<'a> S3ContextMap<'a> {
    pub fn new(entities: &'a Vec<Entity>) -> Self {
        Self { entities }
    }
}

#[derive(Serialize)]
pub struct S3Sdg<'a> {
    endpoints: &'a Vec<Endpoint>,
    restcalls: &'a Vec<RestCall>,
}

impl<'a> S3Sdg<'a> {
    pub fn new(endpoints: &'a Vec<Endpoint>, restcalls: &'a Vec<RestCall>) -> Self {
        Self {
            endpoints,
            restcalls,
        }
    }
}

#[derive(Serialize)]
pub struct S3Imcg<'a> {
    callables: &'a Vec<Callable>,
    calls: &'a Vec<CallStatement>,
}

impl<'a> S3Imcg<'a> {
    pub fn new(callables: &'a Vec<Callable>, calls: &'a Vec<CallStatement>) -> Self {
        Self { callables, calls }
    }
}
