use std::sync::Arc;

use log::warn;
use models::Entity;
use neo4rs::{BoltType, Graph, query};
use uuid::Uuid;

use crate::{
    contextmap::{
        model::{ContextMap, Dependency},
        queries::{CREATE_CONTEXT_MAP, DELETE_CONTEXT_MAP, GET_CONTEXT_MAP},
    },
    errors::database::DatabaseError,
};

pub trait ContextMapRepository {
    fn get_single(
        &self,
        codebase_uuid: Uuid,
        commit_hash: &str,
    ) -> impl std::future::Future<Output = Result<ContextMap, DatabaseError>> + Send;
    fn save(
        &self,
        context_map: &ContextMap,
        codebase_uuid: Uuid,
        commit_hash: &str,
    ) -> impl std::future::Future<Output = Result<(), DatabaseError>> + Send;
    fn delete(
        &self,
        codebase_uuid: Uuid,
        commit_hash: &str,
    ) -> impl std::future::Future<Output = Result<(), DatabaseError>> + Send;
}

pub struct ContextMapRepositoryImpl {
    graph_handle: Arc<Graph>,
}

impl ContextMapRepositoryImpl {
    pub fn new(graph_handle: Arc<Graph>) -> Self {
        Self { graph_handle }
    }
}

impl ContextMapRepository for ContextMapRepositoryImpl {
    async fn get_single(
        &self,
        codebase_uuid: Uuid,
        commit_hash: &str,
    ) -> Result<ContextMap, DatabaseError> {
        let mut result = self
            .graph_handle
            .execute(
                query(GET_CONTEXT_MAP)
                    .param("codebase_uuid", codebase_uuid.to_string())
                    .param("commit_hash", commit_hash.to_string()),
            )
            .await?;
        let mut context_map = ContextMap {
            entities: Vec::new(),
            dependencies: Vec::new(),
        };

        if let Some(record) = result.next().await? {
            let entities_bolt_type: Vec<neo4rs::BoltType> = record.get("all_entities")?;
            let dependencies_bolt_type: Vec<neo4rs::BoltType> = record.get("all_dependencies")?;

            for bolt_entity in entities_bolt_type {
                if let BoltType::Node(node) = bolt_entity {
                    match Entity::try_from(node) {
                        Ok(entity) => context_map.entities.push(entity),
                        Err(e) => warn!("Failed to deserialize Entity: {e:?}"),
                    }
                } else {
                    warn!("Unexpected BoltType for entity: {bolt_entity:?}");
                }
            }

            for bolt_dep in dependencies_bolt_type {
                if let BoltType::Map(map) = bolt_dep {
                    match Dependency::try_from(map) {
                        Ok(dep) => context_map.dependencies.push(dep),
                        Err(e) => warn!("Failed to deserialize Dependency: {e:?}"),
                    }
                } else {
                    warn!("Unexpected BoltType for dependency: {bolt_dep:?}");
                }
            }
        }
        Ok(context_map)
    }

    async fn save(
        &self,
        context_map: &ContextMap,
        codebase_uuid: Uuid,
        commit_hash: &str,
    ) -> Result<(), DatabaseError> {
        self.graph_handle
            .run(
                query(CREATE_CONTEXT_MAP)
                    .param("entities", context_map.entities.clone())
                    .param("dependencies", context_map.dependencies.clone())
                    .param("codebase_uuid", codebase_uuid.to_string())
                    .param("commit_hash", commit_hash.to_string()),
            )
            .await?;
        Ok(())
    }

    async fn delete(&self, codebase_uuid: Uuid, commit_hash: &str) -> Result<(), DatabaseError> {
        self.graph_handle
            .run(
                query(DELETE_CONTEXT_MAP)
                    .param("codebase_uuid", codebase_uuid.to_string())
                    .param("commit_hash", commit_hash.to_string()),
            )
            .await?;
        Ok(())
    }
}
