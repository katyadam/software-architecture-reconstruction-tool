use models::{CallStatement, Callable, Endpoint, Entity, RestCall};
use serde::Serialize;

#[derive(Serialize)]
pub struct S3ContextMapCodeElements<'a> {
    entities: &'a Vec<Entity>,
}

impl<'a> S3ContextMapCodeElements<'a> {
    pub fn new(entities: &'a Vec<Entity>) -> Self {
        Self { entities }
    }
}

#[derive(Serialize)]
pub struct S3SdgCodeElements<'a> {
    endpoints: &'a Vec<Endpoint>,
    restcalls: &'a Vec<RestCall>,
}

impl<'a> S3SdgCodeElements<'a> {
    pub fn new(endpoints: &'a Vec<Endpoint>, restcalls: &'a Vec<RestCall>) -> Self {
        Self {
            endpoints,
            restcalls,
        }
    }
}

#[derive(Serialize)]
pub struct S3ImcgCodeElements<'a> {
    callables: &'a Vec<Callable>,
    calls: &'a Vec<CallStatement>,
}

impl<'a> S3ImcgCodeElements<'a> {
    pub fn new(callables: &'a Vec<Callable>, calls: &'a Vec<CallStatement>) -> Self {
        Self { callables, calls }
    }
}
