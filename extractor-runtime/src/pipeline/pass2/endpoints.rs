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
