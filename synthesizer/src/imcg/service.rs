use std::sync::Arc;

use uuid::Uuid;

use crate::{
    connectors::manager_connector::ManagerConnector,
    errors::service::ServiceError,
    imcg::{
        construction::inter::{ImcgBuilder, ImcgBuilderImpl},
        dto::PostIMCG,
        model::IMCG,
        repository::{ImcgRepository, ImcgRepositoryImpl},
    },
    sdg::{
        model::SDG,
        service::{SdgService, SdgServiceImpl},
    },
};

pub trait ImcgService {
    async fn save(&self, imcg_payload: PostIMCG) -> Result<IMCG, ServiceError>;
    async fn get_single(&self, codebase_uuid: Uuid) -> Result<IMCG, ServiceError>;
    async fn delete(&self, codebase_uuid: Uuid) -> Result<(), ServiceError>;
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
    async fn save(&self, imcg_payload: PostIMCG) -> Result<IMCG, ServiceError> {
        let codebase_configuration = self
            .manager_connector
            .get_codebase_configuration(imcg_payload.codebase_uuid)
            .await?;
        let sdg: SDG = self
            .sdg_service
            .get_single(imcg_payload.codebase_uuid)
            .await?;
        let imcg: IMCG = self.builder.build(
            imcg_payload.callables,
            imcg_payload.call_statements,
            codebase_configuration
                .configuration_data
                .service_descriptions,
            &sdg,
        )?;

        self.repository
            .save(&imcg, imcg_payload.codebase_uuid)
            .await?;

        Ok(imcg)
    }

    async fn get_single(&self, codebase_uuid: Uuid) -> Result<IMCG, ServiceError> {
        let imcg = self.repository.get_single(codebase_uuid).await?;
        Ok(imcg)
    }

    async fn delete(&self, codebase_uuid: Uuid) -> Result<(), ServiceError> {
        self.repository.delete(codebase_uuid).await?;
        Ok(())
    }
}
