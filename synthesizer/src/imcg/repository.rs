use std::sync::Arc;

use log::warn;
use neo4rs::{BoltType, Graph, query};
use uuid::Uuid;

use crate::{
    errors::database::DatabaseError,
    imcg::{
        model::{Call, IMCG, ServiceCallable},
        queries::{CREATE_IMCG, DELETE_IMCG, GET_IMCG},
    },
};

pub trait ImcgRepository {
    async fn get_single(&self, codebase_uuid: Uuid) -> Result<IMCG, DatabaseError>;
    async fn save(&self, imcg: &IMCG, codebase_uuid: Uuid) -> Result<(), DatabaseError>;
    async fn delete(&self, codebase_uuid: Uuid) -> Result<(), DatabaseError>;
}

pub struct ImcgRepositoryImpl {
    graph_handle: Arc<Graph>,
}

impl ImcgRepositoryImpl {
    pub fn new(graph_handle: Arc<Graph>) -> Self {
        Self { graph_handle }
    }
}

impl ImcgRepository for ImcgRepositoryImpl {
    async fn get_single(&self, codebase_uuid: Uuid) -> Result<IMCG, DatabaseError> {
        let mut result = self
            .graph_handle
            .execute(query(GET_IMCG).param("codebase_uuid", codebase_uuid.to_string()))
            .await?;

        let mut imcg = IMCG {
            callables: Vec::new(),
            calls: Vec::new(),
        };

        if let Some(record) = result.next().await? {
            let callables_bolt_type: Vec<neo4rs::BoltType> = record.get("all_callables")?;
            let calls_bolt_type: Vec<neo4rs::BoltType> = record.get("all_calls")?;

            for bolt_service in callables_bolt_type {
                if let BoltType::Node(node) = bolt_service {
                    match ServiceCallable::try_from(node) {
                        Ok(callable) => imcg.callables.push(callable),
                        Err(e) => warn!("Failed to deserialize Callable: {:?}", e),
                    }
                } else {
                    warn!("Unexpected BoltType for Callable: {:?}", bolt_service);
                }
            }

            for bolt_dep in calls_bolt_type {
                if let BoltType::Map(map) = bolt_dep {
                    match Call::try_from(map) {
                        Ok(call) => imcg.calls.push(call),
                        Err(e) => warn!("Failed to deserialize Call: {:?}", e),
                    }
                } else {
                    warn!("Unexpected BoltType for Call: {:?}", bolt_dep);
                }
            }
        }
        Ok(imcg)
    }

    async fn save(&self, imcg: &IMCG, codebase_uuid: Uuid) -> Result<(), DatabaseError> {
        self.graph_handle
            .run(
                query(CREATE_IMCG)
                    .param("callables", imcg.callables.clone())
                    .param("connections", imcg.calls.clone())
                    .param("codebase_uuid", codebase_uuid.to_string()),
            )
            .await?;
        Ok(())
    }

    async fn delete(&self, codebase_uuid: Uuid) -> Result<(), DatabaseError> {
        self.graph_handle
            .run(query(DELETE_IMCG).param("codebase_uuid", codebase_uuid.to_string()))
            .await?;
        Ok(())
    }
}
