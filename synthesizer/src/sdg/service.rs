use crate::{
    errors::service::ServiceError,
    sdg::{
        builder::{SdgBuilder, SdgBuilderImpl},
        dto::PostSDG,
        model::SDG,
        repository::{SdgRepository, SdgRepositoryImpl},
    },
};

pub trait SdgService {
    async fn save(&self, sdg_payload: PostSDG) -> Result<SDG, ServiceError>;
    async fn get_single(&self, codebase_uuid: &str) -> Result<SDG, ServiceError>;
    async fn delete(&self, codebase_uuid: &str) -> Result<(), ServiceError>;
}

pub struct SdgServiceImpl {
    repository: SdgRepositoryImpl,
    builder: SdgBuilderImpl,
}

impl SdgServiceImpl {
    pub fn new(repository: SdgRepositoryImpl, builder: SdgBuilderImpl) -> Self {
        Self {
            repository,
            builder,
        }
    }
}

impl SdgService for SdgServiceImpl {
    async fn save(&self, sdg_payload: PostSDG) -> Result<SDG, ServiceError> {
        let sdg = self
            .builder
            .build_sdg(sdg_payload.endpoints, sdg_payload.restcalls)?;

        self.repository
            .save(&sdg, &sdg_payload.codebase_uuid)
            .await?;

        Ok(sdg)
    }

    async fn get_single(&self, codebase_uuid: &str) -> Result<SDG, ServiceError> {
        let sdg = self.repository.get_single(codebase_uuid).await?;
        Ok(sdg)
    }

    async fn delete(&self, codebase_uuid: &str) -> Result<(), ServiceError> {
        self.repository.delete(codebase_uuid).await?;
        Ok(())
    }
}
