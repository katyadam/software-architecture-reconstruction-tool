use uuid::Uuid;

use crate::{
    connectors::{
        constant_scanner_connector::ConstantScannerConnector, manager_connector::ManagerConnector,
    },
    errors::service::ServiceError,
    sdg::{
        builder::{SdgBuilder, SdgBuilderImpl},
        dto::PostSDG,
        model::Sdg,
        repository::{SdgRepository, SdgRepositoryImpl},
    },
};

pub trait SdgService {
    async fn save(&self, sdg_payload: PostSDG) -> Result<Sdg, ServiceError>;
    async fn get_single(&self, codebase_uuid: Uuid, commit_hash: &str)
    -> Result<Sdg, ServiceError>;
    async fn delete(&self, codebase_uuid: Uuid, commit_hash: &str) -> Result<(), ServiceError>;
}

pub struct SdgServiceImpl {
    pub repository: SdgRepositoryImpl,
    pub builder: SdgBuilderImpl,
    pub manager_connector: ManagerConnector,
    pub constant_scanner_connector: ConstantScannerConnector,
}

impl SdgService for SdgServiceImpl {
    async fn save(&self, sdg_payload: PostSDG) -> Result<Sdg, ServiceError> {
        let configuration_dto = self
            .manager_connector
            .get_commit_configuration(&sdg_payload.commit_hash)
            .await?;

        let constants = self
            .constant_scanner_connector
            .get_commit_constants(&sdg_payload.commit_hash)
            .await?;

        let sdg = self.builder.build(
            &sdg_payload.endpoints,
            &sdg_payload.restcalls,
            &sdg_payload.message_edges,
            &configuration_dto.configuration_data,
            &constants,
        )?;

        self.repository
            .save(&sdg, sdg_payload.codebase_uuid, &sdg_payload.commit_hash)
            .await?;

        Ok(sdg)
    }

    async fn get_single(
        &self,
        codebase_uuid: Uuid,
        commit_hash: &str,
    ) -> Result<Sdg, ServiceError> {
        let sdg = self
            .repository
            .get_single(codebase_uuid, commit_hash)
            .await?;
        Ok(sdg)
    }

    async fn delete(&self, codebase_uuid: Uuid, commit_hash: &str) -> Result<(), ServiceError> {
        self.repository.delete(codebase_uuid, commit_hash).await?;
        Ok(())
    }
}
