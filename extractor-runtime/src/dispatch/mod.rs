use models::CodeElementsAggregate;

use crate::{
    dispatch::extractor::{Extractor, PythonTreesitterExtractor},
    error::ExtractionError,
};

mod extractor;

pub async fn dispatch(
    text: &str,
    file_path: &str,
) -> Result<CodeElementsAggregate, ExtractionError> {
    get(file_path)?.extract(text, file_path).await
}

fn get(file_path: &str) -> Result<Box<dyn Extractor>, ExtractionError> {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| ExtractionError::ExtractorNotFound(file_path.into()))?;

    match ext {
        "py" => Ok(Box::new(PythonTreesitterExtractor::new())),
        _ => Err(ExtractionError::ExtractorNotFound(ext.into())),
    }
}
