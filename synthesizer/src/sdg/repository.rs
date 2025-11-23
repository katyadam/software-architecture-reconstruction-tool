use std::sync::Arc;

use log::warn;
use neo4rs::{BoltType, Graph, query};
use uuid::Uuid;

use crate::{
    errors::database::DatabaseError,
    sdg::{
        model::{Connection, Sdg, Service},
        queries::{CREATE_SDG, DELETE_SDG, GET_SDG},
    },
};

pub trait SdgRepository {
    async fn get_single(
        &self,
        codebase_uuid: Uuid,
        commit_hash: &str,
    ) -> Result<Sdg, DatabaseError>;
    async fn save(
        &self,
        sdg: &Sdg,
        codebase_uuid: Uuid,
        commit_hash: &str,
    ) -> Result<(), DatabaseError>;
    async fn delete(&self, codebase_uuid: Uuid, commit_hash: &str) -> Result<(), DatabaseError>;
}

pub struct SdgRepositoryImpl {
    graph_handle: Arc<Graph>,
}

impl SdgRepositoryImpl {
    pub fn new(graph_handle: Arc<Graph>) -> Self {
        Self { graph_handle }
    }
}

impl SdgRepository for SdgRepositoryImpl {
    async fn get_single(
        &self,
        codebase_uuid: Uuid,
        commit_hash: &str,
    ) -> Result<Sdg, DatabaseError> {
        let mut result = self
            .graph_handle
            .execute(
                query(GET_SDG)
                    .param("codebase_uuid", codebase_uuid.to_string())
                    .param("commit_hash", commit_hash.to_string()),
            )
            .await?;

        let mut sdg = Sdg {
            services: Vec::new(),
            connections: Vec::new(),
        };

        if let Some(record) = result.next().await? {
            let services_bolt_type: Vec<neo4rs::BoltType> = record.get("all_services")?;
            let connections_bolt_type: Vec<neo4rs::BoltType> = record.get("all_connections")?;

            for bolt_service in services_bolt_type {
                if let BoltType::Node(node) = bolt_service {
                    match Service::try_from(node) {
                        Ok(service) => sdg.services.push(service),
                        Err(e) => warn!("Failed to deserialize Service: {e:?}"),
                    }
                } else {
                    warn!("Unexpected BoltType for service: {bolt_service:?}");
                }
            }

            for bolt_dep in connections_bolt_type {
                if let BoltType::Map(map) = bolt_dep {
                    if let Ok(dep) = Connection::try_from(map) {
                        sdg.connections.push(dep)
                    }
                } else {
                    warn!("Unexpected BoltType for connection: {bolt_dep:?}");
                }
            }
        }
        Ok(sdg)
    }

    async fn save(
        &self,
        sdg: &Sdg,
        codebase_uuid: Uuid,
        commit_hash: &str,
    ) -> Result<(), DatabaseError> {
        self.graph_handle
            .run(
                query(CREATE_SDG)
                    .param("services", sdg.services.clone())
                    .param("connections", sdg.connections.clone())
                    .param("codebase_uuid", codebase_uuid.to_string())
                    .param("commit_hash", commit_hash.to_string()),
            )
            .await?;
        Ok(())
    }

    async fn delete(&self, codebase_uuid: Uuid, commit_hash: &str) -> Result<(), DatabaseError> {
        self.graph_handle
            .run(
                query(DELETE_SDG)
                    .param("codebase_uuid", codebase_uuid.to_string())
                    .param("commit_hash", commit_hash.to_string()),
            )
            .await?;
        Ok(())
    }
}
