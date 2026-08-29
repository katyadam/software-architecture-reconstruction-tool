use models::ir::{language::Language, project::TypedFileRecord};

trait EndpointPostProcessor {
    fn process(&self, files: &mut [TypedFileRecord]);
}

struct NoopEndpointPostProcessor;

impl EndpointPostProcessor for NoopEndpointPostProcessor {
    fn process(&self, _files: &mut [TypedFileRecord]) {}
}

struct PythonEndpointPostProcessor;

impl EndpointPostProcessor for PythonEndpointPostProcessor {
    fn process(&self, files: &mut [TypedFileRecord]) {
        let mut python_files = files
            .iter_mut()
            .filter(|file| file.language == Language::Python)
            .collect::<Vec<_>>();
        python_extractor::extraction::post_process(&mut python_files);
    }
}

static NOOP: NoopEndpointPostProcessor = NoopEndpointPostProcessor;
static PYTHON: PythonEndpointPostProcessor = PythonEndpointPostProcessor;

fn strategy(language: &Language) -> &'static dyn EndpointPostProcessor {
    match language {
        Language::Java => &NOOP,
        Language::Python => &PYTHON,
    }
}

/// Apply endpoint-specific enrichment without exposing language implementations
/// to the project-building pipeline.
pub fn post_process_endpoints(files: &mut [TypedFileRecord]) {
    if files.iter().any(|file| file.language == Language::Python) {
        strategy(&Language::Python).process(files);
    }
}
