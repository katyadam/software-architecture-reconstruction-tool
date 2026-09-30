use super::registry::CallGraphProvider;
use models::call_graph::{CallGraphOutcome, CallGraphRequest, CallGraphStatus, Language};
use std::{path::PathBuf, process::Command};

/// Invokes the standalone WALA adapter for Java source trees.
pub struct WalaJavaProvider {
    jar: PathBuf,
}
impl WalaJavaProvider {
    /// Creates a Java provider that launches the adapter JAR at `jar`.
    pub fn new(jar: impl Into<PathBuf>) -> Self {
        Self { jar: jar.into() }
    }
}
impl CallGraphProvider for WalaJavaProvider {
    /// Declares that this adapter accepts Java source code.
    fn language(&self) -> Language {
        Language::Java
    }
    /// Executes WALA and validates its versioned JSON response before returning it.
    fn analyze(&self, request: &CallGraphRequest) -> CallGraphOutcome {
        let output = Command::new("java")
            .args([
                "-jar",
                self.jar.to_string_lossy().as_ref(),
                "--source-dir",
                &request.source_root,
            ])
            .output();
        match output {
            Ok(output) if output.status.success() => {
                match serde_json::from_slice::<CallGraphOutcome>(&output.stdout) {
                    Ok(result)
                        if result.schema_version == 1
                            && result.status != CallGraphStatus::Failed =>
                    {
                        result
                    }
                    Ok(_) | Err(_) => failed(request, "WALA produced invalid call-graph JSON"),
                }
            }
            Ok(output) => failed(request, &String::from_utf8_lossy(&output.stderr)),
            Err(error) => failed(request, &error.to_string()),
        }
    }
}

/// Converts adapter process or protocol failures into a non-fatal provider outcome.
fn failed(request: &CallGraphRequest, diagnostic: &str) -> CallGraphOutcome {
    CallGraphOutcome {
        schema_version: 1,
        status: CallGraphStatus::Failed,
        provider_id: "wala-java".into(),
        source_root: request.source_root.clone(),
        algorithm: "zero_one_container_cfa".into(),
        diagnostics: vec![diagnostic.into()],
        edges: vec![],
    }
}
