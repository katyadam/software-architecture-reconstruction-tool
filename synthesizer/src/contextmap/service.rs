use uuid::Uuid;

use crate::{
    connectors::manager_connector::ManagerConnector,
    contextmap::{
        builder::{ContextMapBuilder, ContextMapBuilderImpl},
        dto::PostContextMap,
        model::ContextMap,
        repository::{ContextMapRepository, ContextMapRepositoryImpl},
    },
    errors::service::ServiceError,
};

pub trait ContextMapService {
    async fn save(&self, cm_payload: PostContextMap) -> Result<ContextMap, ServiceError>;
    async fn get_single(
        &self,
        codebase_uuid: Uuid,
        commit_hash: &str,
    ) -> Result<ContextMap, ServiceError>;
    async fn delete(&self, codebase_uuid: Uuid, commit_hash: &str) -> Result<(), ServiceError>;
}

pub struct ContextMapServiceImpl {
    repository: ContextMapRepositoryImpl,
    builder: ContextMapBuilderImpl,
    manager_connector: ManagerConnector,
}

impl ContextMapServiceImpl {
    pub fn new(
        repository: ContextMapRepositoryImpl,
        builder: ContextMapBuilderImpl,
        manager_connector: ManagerConnector,
    ) -> Self {
        Self {
            repository,
            builder,
            manager_connector,
        }
    }
}

impl ContextMapService for ContextMapServiceImpl {
    async fn save(&self, cm_payload: PostContextMap) -> Result<ContextMap, ServiceError> {
        let configuration = self
            .manager_connector
            .get_commit_configuration(&cm_payload.commit_hash)
            .await?;

        let cm = self.builder.build(
            &cm_payload.entities,
            &configuration.configuration_data.service_descriptions,
        )?;
        self.repository
            .save(&cm, cm_payload.codebase_uuid, &cm_payload.commit_hash)
            .await?;

        Ok(cm)
    }

    async fn get_single(
        &self,
        codebase_uuid: Uuid,
        commit_hash: &str,
    ) -> Result<ContextMap, ServiceError> {
        let cm = self
            .repository
            .get_single(codebase_uuid, commit_hash)
            .await?;
        Ok(cm)
    }

    async fn delete(&self, codebase_uuid: Uuid, commit_hash: &str) -> Result<(), ServiceError> {
        self.repository.delete(codebase_uuid, commit_hash).await?;
        Ok(())
    }
}
