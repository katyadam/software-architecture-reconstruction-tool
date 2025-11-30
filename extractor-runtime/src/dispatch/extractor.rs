use async_trait::async_trait;
use models::CodeElementsAggregate;
use python_extractor::extraction::parse::parse;

use crate::error::ExtractionError;

#[async_trait]
pub trait Extractor: Send + Sync {
    async fn extract(
        &self,
        text: &str,
        file_path: &str,
    ) -> Result<CodeElementsAggregate, ExtractionError>;
}

pub struct PythonTreesitterExtractor {}

impl PythonTreesitterExtractor {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Extractor for PythonTreesitterExtractor {
    async fn extract(
        &self,
        text: &str,
        file_path: &str,
    ) -> Result<CodeElementsAggregate, ExtractionError> {
        let result = parse(text, file_path).await;

        Ok(result)
    }
}
