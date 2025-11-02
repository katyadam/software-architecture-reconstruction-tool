use uuid::Uuid;

use crate::{
    connectors::manager_connector::ManagerConnector,
    errors::service::ServiceError,
    imcg::{
        construction::builder::{ImcgBuilder, ImcgBuilderImpl},
        dto::PostIMCG,
        model::IMCG,
        repository::{ImcgRepository, ImcgRepositoryImpl},
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
}

impl ImcgServiceImpl {
    pub fn new(
        repository: ImcgRepositoryImpl,
        builder: ImcgBuilderImpl,
        manager_connector: ManagerConnector,
    ) -> Self {
        Self {
            repository,
            builder,
            manager_connector,
        }
    }
}

impl ImcgService for ImcgServiceImpl {
    async fn save(&self, imcg_payload: PostIMCG) -> Result<IMCG, ServiceError> {
        let codebase_configuration = self
            .manager_connector
            .get_codebase_configuration(imcg_payload.codebase_uuid)
            .await?;

        let imcg: IMCG = self.builder.build(
            imcg_payload.callables,
            imcg_payload.call_statements,
            imcg_payload.imports,
            codebase_configuration
                .configuration_data
                .service_descriptions,
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
