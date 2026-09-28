use super::registry::CallGraphProvider;
use models::call_graph::{CallGraphOutcome, CallGraphRequest, CallGraphStatus, Language};
use std::{path::PathBuf, process::Command};

pub struct WalaJavaProvider {
    jar: PathBuf,
}
impl WalaJavaProvider {
    pub fn new(jar: impl Into<PathBuf>) -> Self {
        Self { jar: jar.into() }
    }
}
impl CallGraphProvider for WalaJavaProvider {
    fn language(&self) -> Language {
        Language::Java
    }
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
