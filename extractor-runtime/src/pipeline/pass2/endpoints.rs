use models::ir::{language::Language, project::TypedFileRecord};

pub fn resolve_endpoint_handlers(files: &mut [TypedFileRecord]) {
    for language in files.iter().map(|file| file.language).collect::<Vec<_>>() {
        match language {
            Language::Go => {
                go_extractor::extraction::resolve_package_endpoint_handlers(files);
                break;
            }
            Language::Java | Language::Python => {}
        }
    }
}

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
        Language::Go => &NOOP,
    }
}

pub fn post_process_endpoints(files: &mut [TypedFileRecord]) {
    if files.iter().any(|file| file.language == Language::Python) {
        strategy(&Language::Python).process(files);
    }
}
