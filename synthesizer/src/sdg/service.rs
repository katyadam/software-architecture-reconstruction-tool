use uuid::Uuid;

use crate::{
    connectors::manager_connector::ManagerConnector,
    errors::service::ServiceError,
    sdg::{
        builder::{SdgBuilder, SdgBuilderImpl},
        dto::PostSDG,
        model::types::SDG,
        repository::{SdgRepository, SdgRepositoryImpl},
    },
};

pub trait SdgService {
    async fn save(&self, sdg_payload: PostSDG) -> Result<SDG, ServiceError>;
    async fn get_single(&self, codebase_uuid: Uuid) -> Result<SDG, ServiceError>;
    async fn delete(&self, codebase_uuid: Uuid) -> Result<(), ServiceError>;
}

pub struct SdgServiceImpl {
    repository: SdgRepositoryImpl,
    builder: SdgBuilderImpl,
    manager_connector: ManagerConnector,
}

impl SdgServiceImpl {
    pub fn new(
        repository: SdgRepositoryImpl,
        builder: SdgBuilderImpl,
        manager_connector: ManagerConnector,
    ) -> Self {
        Self {
            repository,
            builder,
            manager_connector,
        }
    }
}

impl SdgService for SdgServiceImpl {
    async fn save(&self, sdg_payload: PostSDG) -> Result<SDG, ServiceError> {
        let configuration = self
            .manager_connector
            .get_codebase_configuration(sdg_payload.codebase_uuid)
            .await?;

        let sdg = self.builder.build(
            sdg_payload.endpoints,
            sdg_payload.restcalls,
            configuration.configuration_data,
        )?;

        self.repository
            .save(&sdg, sdg_payload.codebase_uuid)
            .await?;

        Ok(sdg)
    }

    async fn get_single(&self, codebase_uuid: Uuid) -> Result<SDG, ServiceError> {
        let sdg = self.repository.get_single(codebase_uuid).await?;
        Ok(sdg)
    }

    async fn delete(&self, codebase_uuid: Uuid) -> Result<(), ServiceError> {
        self.repository.delete(codebase_uuid).await?;
        Ok(())
    }
}
