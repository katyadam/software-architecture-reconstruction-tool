use std::sync::Arc;

use uuid::Uuid;

use crate::{
    connectors::manager_connector::ManagerConnector,
    errors::service::ServiceError,
    imcg::{
        construction::inter::{ImcgBuilder, ImcgBuilderImpl},
        dto::PostIMCG,
        model::Imcg,
        repository::{ImcgRepository, ImcgRepositoryImpl},
    },
    sdg::{
        model::Sdg,
        service::{SdgService, SdgServiceImpl},
    },
};

pub trait ImcgService {
    async fn save(&self, imcg_payload: PostIMCG) -> Result<Imcg, ServiceError>;
    async fn get_single(
        &self,
        codebase_uuid: Uuid,
        commit_hash: &str,
    ) -> Result<Imcg, ServiceError>;
    async fn delete(&self, codebase_uuid: Uuid, commit_hash: &str) -> Result<(), ServiceError>;
}

pub struct ImcgServiceImpl {
    repository: ImcgRepositoryImpl,
    builder: ImcgBuilderImpl,
    manager_connector: ManagerConnector,
    sdg_service: Arc<SdgServiceImpl>,
}

impl ImcgServiceImpl {
    pub fn new(
        repository: ImcgRepositoryImpl,
        builder: ImcgBuilderImpl,
        manager_connector: ManagerConnector,
        sdg_service: Arc<SdgServiceImpl>,
    ) -> Self {
        Self {
            repository,
            builder,
            manager_connector,
            sdg_service,
        }
    }
}

impl ImcgService for ImcgServiceImpl {
    async fn save(&self, imcg_payload: PostIMCG) -> Result<Imcg, ServiceError> {
        let codebase_configuration = self
            .manager_connector
            .get_commit_configuration(&imcg_payload.commit_hash)
            .await?;
        let sdg: Sdg = self
            .sdg_service
            .get_single(imcg_payload.codebase_uuid, &imcg_payload.commit_hash)
            .await?;
        let imcg: Imcg = self.builder.build(
            imcg_payload.callables,
            imcg_payload.call_statements,
            codebase_configuration
                .configuration_data
                .service_descriptions,
            &sdg,
        )?;

        self.repository
            .save(&imcg, imcg_payload.codebase_uuid, &imcg_payload.commit_hash)
            .await?;

        Ok(imcg)
    }

    async fn get_single(
        &self,
        codebase_uuid: Uuid,
        commit_hash: &str,
    ) -> Result<Imcg, ServiceError> {
        let imcg = self
            .repository
            .get_single(codebase_uuid, commit_hash)
            .await?;
        Ok(imcg)
    }

    async fn delete(&self, codebase_uuid: Uuid, commit_hash: &str) -> Result<(), ServiceError> {
        self.repository.delete(codebase_uuid, commit_hash).await?;
        Ok(())
    }
}
