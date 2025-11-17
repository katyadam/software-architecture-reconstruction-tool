use std::sync::Arc;

use crate::{
    contextmap::{
        dto::PostContextMap,
        service::{ContextMapService, ContextMapServiceImpl},
    },
    errors::s3::{S3ClientError, S3ServiceError},
    imcg::{
        dto::PostIMCG,
        service::{ImcgService, ImcgServiceImpl},
    },
    s3::{
        client::S3Client,
        dto::PostViews,
        model::{S3ContextMapCodeElements, S3ImcgCodeElements, S3SdgCodeElements},
    },
    sdg::{
        dto::PostSDG,
        service::{SdgService, SdgServiceImpl},
    },
};
use futures::stream::{self, StreamExt};

pub struct S3Service {
    s3_client: S3Client,
    context_map_service: Arc<ContextMapServiceImpl>,
    sdg_service: Arc<SdgServiceImpl>,
    imcg_service: Arc<ImcgServiceImpl>,
}

impl S3Service {
    pub fn new(
        s3_client: S3Client,
        context_map_service: Arc<ContextMapServiceImpl>,
        sdg_service: Arc<SdgServiceImpl>,
        imcg_service: Arc<ImcgServiceImpl>,
    ) -> Self {
        Self {
            s3_client,
            context_map_service,
            sdg_service,
            imcg_service,
        }
    }

    pub async fn save_views(&self, dto: PostViews) -> Result<(), S3ServiceError> {
        let loaded_cm_elements = self.load_context_map_elements(&dto.base_dir_path).await?;
        let loaded_sdg_elements = self.load_sdg_elements(&dto.base_dir_path).await?;
        let loaded_imcg_elements = self.load_imcg_elements(&dto.base_dir_path).await?;
        self.context_map_service
            .save(PostContextMap {
                codebase_uuid: dto.codebase_uuid.clone(),
                entities: loaded_cm_elements.entities,
            })
            .await?;

        self.sdg_service
            .save(PostSDG {
                codebase_uuid: dto.codebase_uuid.clone(),
                endpoints: loaded_sdg_elements.endpoints,
                restcalls: loaded_sdg_elements.restcalls,
            })
            .await?;

        self.imcg_service
            .save(PostIMCG {
                codebase_uuid: dto.codebase_uuid,
                callables: loaded_imcg_elements.callables,
                call_statements: loaded_imcg_elements.calls,
            })
            .await?;
        Ok(())
    }

    async fn load_and_merge_chunks<T, F, R>(
        &self,
        index_path: &str,
        merge_fn: F,
    ) -> Result<R, S3ServiceError>
    where
        T: Send + 'static + serde::de::DeserializeOwned,
        F: FnOnce(Vec<T>) -> R,
    {
        let chunk_paths = self.s3_client.load_chunks_index(index_path).await?;

        let chunks = stream::iter(chunk_paths)
            .map(|chunk_path| async move { self.s3_client.load_chunk::<T>(&chunk_path).await })
            .buffer_unordered(8)
            .collect::<Vec<Result<T, S3ClientError>>>()
            .await;

        let chunks: Result<Vec<T>, _> = chunks.into_iter().collect();
        Ok(merge_fn(chunks?))
    }

    pub async fn load_context_map_elements(
        &self,
        base_dir_path: &str,
    ) -> Result<S3ContextMapCodeElements, S3ServiceError> {
        let index_path = format!("{}/cm/index.json", base_dir_path);
        self.load_and_merge_chunks::<S3ContextMapCodeElements, _, S3ContextMapCodeElements>(
            &index_path,
            |chunks| {
                let entities = chunks
                    .into_iter()
                    .flat_map(|chunk| chunk.entities)
                    .collect();
                S3ContextMapCodeElements::new(entities)
            },
        )
        .await
    }

    pub async fn load_sdg_elements(
        &self,
        base_dir_path: &str,
    ) -> Result<S3SdgCodeElements, S3ServiceError> {
        let index_path = format!("{}/sdg/index.json", base_dir_path);
        self.load_and_merge_chunks::<S3SdgCodeElements, _, S3SdgCodeElements>(
            &index_path,
            |chunks| {
                let (endpoints, restcalls) = chunks.into_iter().fold(
                    (Vec::new(), Vec::new()),
                    |(mut endpoints_aggr, mut restcalls_aggr), chunk| {
                        endpoints_aggr.extend(chunk.endpoints);
                        restcalls_aggr.extend(chunk.restcalls);

                        (endpoints_aggr, restcalls_aggr)
                    },
                );
                S3SdgCodeElements::new(endpoints, restcalls)
            },
        )
        .await
    }

    pub async fn load_imcg_elements(
        &self,
        base_dir_path: &str,
    ) -> Result<S3ImcgCodeElements, S3ServiceError> {
        let index_path = format!("{}/imcg/index.json", base_dir_path);
        self.load_and_merge_chunks::<S3ImcgCodeElements, _, S3ImcgCodeElements>(
            &index_path,
            |chunks| {
                let (callables, calls) = chunks.into_iter().fold(
                    (Vec::new(), Vec::new()),
                    |(mut callables_aggr, mut calls_aggr), chunk| {
                        callables_aggr.extend(chunk.callables);
                        calls_aggr.extend(chunk.calls);

                        (callables_aggr, calls_aggr)
                    },
                );
                S3ImcgCodeElements::new(callables, calls)
            },
        )
        .await
    }
}
