use uuid::Uuid;

use crate::{
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
}

impl ImcgServiceImpl {
    pub fn new(repository: ImcgRepositoryImpl, builder: ImcgBuilderImpl) -> Self {
        Self {
            repository,
            builder,
        }
    }
}

impl ImcgService for ImcgServiceImpl {
    async fn save(&self, imcg_payload: PostIMCG) -> Result<IMCG, ServiceError> {
        let imcg: IMCG = self.builder.build(
            imcg_payload.callables,
            imcg_payload.call_statements,
            imcg_payload.imports,
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
