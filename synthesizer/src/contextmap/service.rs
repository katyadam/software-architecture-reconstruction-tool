use uuid::Uuid;

use crate::{
    contextmap::{
        builder::{ContextMapBuilder, ContextMapBuilderImpl},
        dto::PostContextMap,
        model::ContextMap,
        repository::{ContextMapRepository, ContextMapRepositoryImpl},
    },
    errors::service::ServiceError,
};

pub trait ContextMapService {
    fn save(
        &self,
        cm_payload: PostContextMap,
    ) -> impl std::future::Future<Output = Result<ContextMap, ServiceError>> + Send;
    fn get_single(
        &self,
        codebase_uuid: Uuid,
    ) -> impl std::future::Future<Output = Result<ContextMap, ServiceError>> + Send;
    fn delete(
        &self,
        codebase_uuid: Uuid,
    ) -> impl std::future::Future<Output = Result<(), ServiceError>> + Send;
}

pub struct ContextMapServiceImpl {
    repository: ContextMapRepositoryImpl,
    builder: ContextMapBuilderImpl,
}

impl ContextMapServiceImpl {
    pub fn new(repository: ContextMapRepositoryImpl, builder: ContextMapBuilderImpl) -> Self {
        Self {
            repository,
            builder,
        }
    }
}

impl ContextMapService for ContextMapServiceImpl {
    async fn save(&self, cm_payload: PostContextMap) -> Result<ContextMap, ServiceError> {
        let cm = self.builder.build(&cm_payload.entities)?;
        self.repository.save(&cm, cm_payload.codebase_uuid).await?;

        Ok(cm)
    }

    async fn get_single(&self, codebase_uuid: Uuid) -> Result<ContextMap, ServiceError> {
        let cm = self.repository.get_single(codebase_uuid).await?;
        Ok(cm)
    }

    async fn delete(&self, codebase_uuid: Uuid) -> Result<(), ServiceError> {
        self.repository.delete(codebase_uuid).await?;
        Ok(())
    }
}
