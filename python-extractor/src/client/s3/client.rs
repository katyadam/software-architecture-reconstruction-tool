use log::info;
use models::{CallStatement, Callable, Endpoint, Entity, RestCall};
use s3::Bucket;
use serde::Serialize;

use crate::{
    client::s3::model::{S3ContextMapCodeElements, S3ImcgCodeElements, S3SdgCodeElements},
    error::S3ClientError,
};

pub struct S3Client {
    bucket: Bucket,
}

impl S3Client {
    pub fn new(bucket: Bucket) -> Self {
        Self { bucket }
    }

    pub async fn save_context_map(
        &self,
        entities: &Vec<Entity>,
        path: &str,
    ) -> Result<(), S3ClientError> {
        self.save_chunk(
            &S3ContextMapCodeElements::new(entities),
            &format!("{}/cm", path),
        )
        .await
    }

    pub async fn save_sdg(
        &self,
        endpoints: &Vec<Endpoint>,
        restcalls: &Vec<RestCall>,
        path: &str,
    ) -> Result<(), S3ClientError> {
        self.save_chunk(
            &S3SdgCodeElements::new(endpoints, restcalls),
            &format!("{}/sdg", path),
        )
        .await
    }

    pub async fn save_imcg(
        &self,
        callables: &Vec<Callable>,
        calls: &Vec<CallStatement>,
        path: &str,
    ) -> Result<(), S3ClientError> {
        self.save_chunk(
            &S3ImcgCodeElements::new(callables, calls),
            &format!("{}/imcg", path),
        )
        .await
    }

    async fn save_chunk<T: Serialize>(
        &self,
        chunk: &T,
        chunk_dir_path: &str,
    ) -> Result<(), S3ClientError> {
        let data = serde_json::to_vec(chunk)?;
        let full_chunk_path = format!("{}/chunk-{}.json", chunk_dir_path, uuid::Uuid::new_v4());
        self.bucket
            .put_object(full_chunk_path.clone(), &data)
            .await?;

        self.update_index(chunk_dir_path, &full_chunk_path).await?;

        Ok(())
    }

    async fn update_index(
        &self,
        chunk_dir_path: &str,
        full_chunk_path: &str,
    ) -> Result<(), S3ClientError> {
        let index_path = format!("{}/index.json", chunk_dir_path);

        if !self.bucket.object_exists(&index_path).await? {
            self.bucket.put_object(&index_path, b"[]").await?;
        }

        let index_data = self.bucket.get_object(&index_path).await?;

        let mut index = serde_json::from_slice::<Vec<String>>(&index_data.bytes())?;
        index.push(full_chunk_path.to_string());

        let index_data = serde_json::to_vec(&index)?;
        self.bucket.put_object(&index_path, &index_data).await?;

        Ok(())
    }
}
