use actix_web::http::Error;
use s3::Bucket;
use serde::de::DeserializeOwned;

use crate::errors::s3::S3ClientError;

pub struct S3Client {
    bucket: Bucket,
}

impl S3Client {
    pub fn new(bucket: Bucket) -> Self {
        Self { bucket }
    }

    // pub async fn load_context_map(&self, base_dir_path: &str) -> Result<(), S3ClientError> {}

    async fn load_chunk<R: DeserializeOwned>(&self, chunk_path: &str) -> Result<R, S3ClientError>
    where
        R: DeserializeOwned + 'static,
    {
        let response = self.bucket.get_object(chunk_path).await?;
        let parsed: R = serde_json::from_slice(&response.bytes())?;

        Ok(parsed)
    }
}
