use models::ir::{language::Language, project::TypedFileRecord};

/// Runs language-owned edge identification stages from the shared Pass 2 pipeline.
pub(super) trait IdentificationBackend: Sync {
    /// Identifies edges in one type-resolved file.
    fn identify(&self, file: &mut TypedFileRecord);

    /// Resolves edges that require context from files in the same project.
    fn resolve_project_edges(&self, _files: &mut [TypedFileRecord]) {}
}

struct JavaBackend;
struct PythonBackend;
struct GoBackend;

static JAVA: JavaBackend = JavaBackend;
static PYTHON: PythonBackend = PythonBackend;
static GO: GoBackend = GoBackend;

/// Returns the identification backend for one source language.
pub(super) fn strategy(language: &Language) -> &'static dyn IdentificationBackend {
    match language {
        Language::Java => &JAVA,
        Language::Python => &PYTHON,
        Language::Go => &GO,
    }
}

impl IdentificationBackend for JavaBackend {
    fn identify(&self, file: &mut TypedFileRecord) {
        java_extractor::extraction::identify(file);
    }
}

impl IdentificationBackend for PythonBackend {
    fn identify(&self, file: &mut TypedFileRecord) {
        python_extractor::extraction::parse::identify(file);
    }
}

impl IdentificationBackend for GoBackend {
    fn identify(&self, file: &mut TypedFileRecord) {
        go_extractor::extraction::identify(file);
    }

    fn resolve_project_edges(&self, files: &mut [TypedFileRecord]) {
        go_extractor::extraction::resolve_project_message_edges(files);
    }
}
