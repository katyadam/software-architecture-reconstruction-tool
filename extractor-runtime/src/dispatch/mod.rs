use std::collections::HashMap;

use log::info;
use models::{CodeElementsAggregate, api::ExtractionError, ir::syntax::FileRecord};
use once_cell::sync::Lazy;

use crate::dispatch::extractor::{Extractor, JavaTreeSitterExtractor, PythonTreesitterExtractor};

mod extractor;

static PYTHON_TREESITTER_EXTRACTOR: Lazy<PythonTreesitterExtractor> =
    Lazy::new(PythonTreesitterExtractor::new);
static JAVA_TREESITTER_EXTRACTOR: Lazy<JavaTreeSitterExtractor> =
    Lazy::new(JavaTreeSitterExtractor::new);

static EXTRACTORS: Lazy<HashMap<&'static str, &'static dyn Extractor>> = Lazy::new(|| {
    HashMap::from([
        ("py", &*PYTHON_TREESITTER_EXTRACTOR as &dyn Extractor),
        ("java", &*JAVA_TREESITTER_EXTRACTOR as &dyn Extractor),
    ])
});

pub async fn dispatch(
    text: &str,
    file_path: &str,
) -> Result<Option<CodeElementsAggregate>, ExtractionError> {
    if let Some(extractor) = extractor_for(file_path) {
        match extractor.extract(text, file_path).await {
            Ok(res) => Ok(Some(res)),
            Err(ExtractionError::SymbolicEvaluation(e)) => {
                info!("Error occured during Symbolic Evaluation: {}", e);
                Ok(None)
            }
            Err(e) => Err(e),
        }
    } else {
        Ok(None)
    }
}

/// Counterpart to [`dispatch`]: extracts a single file into a [`FileRecord`]
/// (Pass 1 only — no cross-file resolution).
pub fn dispatch_syntactic(
    text: &str,
    file_path: &str,
) -> Result<Option<FileRecord>, ExtractionError> {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str());
    match ext {
        Some("java") => java_extractor::extraction::extract_syntactic(text, file_path).map(Some),
        Some("py") => {
            python_extractor::extraction::parse::extract_syntactic(text, file_path).map(Some)
        }
        _ => Ok(None),
    }
}

fn extractor_for(file_path: &str) -> Option<&'static dyn Extractor> {
    let ext = std::path::Path::new(file_path).extension()?.to_str()?;

    EXTRACTORS.get(ext).copied()
}
