use models::{CodeElementsAggregate, api::ExtractionError};

use crate::dispatch::extractor::{Extractor, JavaTreeSitterExtractor, PythonTreesitterExtractor};

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

    // Would be great to use Singleton. Currently, for each file a new extractor is always created which is not optimal.
    match ext {
        "py" => Ok(Box::new(PythonTreesitterExtractor::new())),
        "java" => Ok(Box::new(JavaTreeSitterExtractor::new())),
        _ => Err(ExtractionError::ExtractorNotFound(ext.into())),
    }
}
