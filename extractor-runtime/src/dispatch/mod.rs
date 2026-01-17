use models::{CodeElementsAggregate, api::ExtractionError};
use once_cell::sync::Lazy;

use crate::dispatch::extractor::{Extractor, JavaTreeSitterExtractor, PythonTreesitterExtractor};

mod extractor;

static PYTHON_TREESITTER_EXTRACTOR: Lazy<PythonTreesitterExtractor> =
    Lazy::new(PythonTreesitterExtractor::new);
static JAVA_TREESITTER_EXTRACTOR: Lazy<JavaTreeSitterExtractor> =
    Lazy::new(JavaTreeSitterExtractor::new);

pub async fn dispatch(
    text: &str,
    file_path: &str,
) -> Result<CodeElementsAggregate, ExtractionError> {
    get(file_path)?.extract(text, file_path).await
}

fn get(file_path: &str) -> Result<&'static dyn Extractor, ExtractionError> {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| ExtractionError::ExtractorNotFound(file_path.into()))?;

    match ext {
        "py" => Ok(&*PYTHON_TREESITTER_EXTRACTOR),
        "java" => Ok(&*JAVA_TREESITTER_EXTRACTOR),
        _ => Err(ExtractionError::ExtractorNotFound(ext.into())),
    }
}
