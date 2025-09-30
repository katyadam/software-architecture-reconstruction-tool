use std::sync::Arc;

use actix_web::web;
use log::warn;
use neo4rs::{BoltType, Error, Graph, query};

use crate::{
    errors::database::DatabaseError,
    sdg::{
        model::{Connection, SDG, Service},
        queries::{CREATE_SDG, DELETE_SDG, GET_SDG},
    },
};

pub trait SdgRepository {
    async fn get_single(&self, codebase_uuid: &str) -> Result<SDG, DatabaseError>;
    async fn save(&self, sdg: &SDG, codebase_uuid: &str) -> Result<(), DatabaseError>;
    async fn delete_sdg(&self, codebase_uuid: &str) -> Result<(), DatabaseError>;
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
    async fn get_single(&self, codebase_uuid: &str) -> Result<SDG, DatabaseError> {
        let mut result = self
            .graph_handle
            .execute(query(GET_SDG).param("codebase_uuid", codebase_uuid))
            .await?;

        let mut sdg = SDG {
            services: Vec::new(),
            connections: Vec::new(),
        };

        if let Some(record) = result.next().await? {
            let services_bolt_type: Vec<neo4rs::BoltType> = record.get("all_services")?;
            let connections_bolt_type: Vec<neo4rs::BoltType> = record.get("all_connections")?;

            for bolt_service in services_bolt_type {
                if let BoltType::Node(node) = bolt_service {
                    match Service::try_from(node) {
                        Ok(entity) => sdg.services.push(entity),
                        Err(e) => warn!("Failed to deserialize Entity: {:?}", e),
                    }
                } else {
                    warn!("Unexpected BoltType for entity: {:?}", bolt_service);
                }
            }

            for bolt_dep in connections_bolt_type {
                if let BoltType::Map(map) = bolt_dep {
                    match Connection::try_from(map) {
                        Ok(dep) => sdg.connections.push(dep),
                        Err(e) => warn!("Failed to deserialize Connection: {:?}", e),
                    }
                } else {
                    warn!("Unexpected BoltType for connection: {:?}", bolt_dep);
                }
            }
        }
        Ok(sdg)
    }

    async fn save(&self, sdg: &SDG, codebase_uuid: &str) -> Result<(), DatabaseError> {
        self.graph_handle
            .run(
                query(CREATE_SDG)
                    .param("services", sdg.services.clone())
                    .param("connections", sdg.connections.clone())
                    .param("codebase_uuid", codebase_uuid),
            )
            .await?;
        Ok(())
    }

    async fn delete_sdg(&self, codebase_uuid: &str) -> Result<(), DatabaseError> {
        self.graph_handle
            .run(query(DELETE_SDG).param("codebase_uuid", codebase_uuid))
            .await?;
        Ok(())
    }
}
