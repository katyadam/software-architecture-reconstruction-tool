use super::adapter_process::{isolate_process_group, wait_for_output};
use super::registry::CallGraphProvider;
use models::{
    JavaTestCase,
    call_graph::{CallGraphOutcome, CallGraphRequest, CallGraphStatus, Language},
};
use serde::Deserialize;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

pub const DEFAULT_ANALYSIS_TIMEOUT: Duration = Duration::from_secs(300);

/// Launches a standalone Java call-graph adapter using the versioned JSON protocol.
pub struct JavaAdapterProvider {
    provider_id: String,
    jar: PathBuf,
    timeout: Duration,
    algorithm: String,
    prepare_artifacts: bool,
}

impl JavaAdapterProvider {
    pub fn new(
        provider_id: impl Into<String>,
        jar: impl Into<PathBuf>,
        timeout: Duration,
        algorithm: impl Into<String>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            jar: jar.into(),
            timeout,
            algorithm: algorithm.into(),
            prepare_artifacts: true,
        }
    }

    /// Keeps a legacy adapter's source-root preparation path while the provider-neutral path migrates.
    pub fn without_artifact_preparation(mut self) -> Self {
        self.prepare_artifacts = false;
        self
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Analyzes one module from discovered JUnit methods and changed callable targets.
    pub fn analyze_test_roots(
        &self,
        source_root: &std::path::Path,
        test_roots: &[JavaTestCase],
        targets: &[String],
    ) -> CallGraphOutcome {
        let request = CallGraphRequest::new(source_root.to_string_lossy(), Language::Java);
        let artifacts = if self.prepare_artifacts {
            match self.prepare_artifacts(&request) {
                Ok(artifacts) => Some(artifacts),
                Err(diagnostic) => {
                    return failed(&request, &self.provider_id, &self.algorithm, &diagnostic);
                }
            }
        } else {
            None
        };
        self.invoke_with_test_roots(&request, test_roots, targets, artifacts.as_ref())
    }

    /// Prepares all in-project application modules for a consumer's test roots.
    /// This explicit path also lets legacy providers use the neutral preparer.
    pub fn analyze_modules(
        &self,
        project_root: &std::path::Path,
        module_roots: &[PathBuf],
        test_roots: &[JavaTestCase],
        targets: &[String],
    ) -> CallGraphOutcome {
        let request = CallGraphRequest::new(project_root.to_string_lossy(), Language::Java);
        let mut merged = PreparedJavaArtifacts {
            application_class_dirs: vec![],
            test_class_dirs: vec![],
            classpath: vec![],
            diagnostic: String::new(),
        };
        for module in module_roots {
            let module_request = CallGraphRequest::new(
                module.join("src/main/java").to_string_lossy(),
                Language::Java,
            );
            let artifacts = match self.prepare_artifacts(&module_request) {
                Ok(artifacts) => artifacts,
                Err(diagnostic) => {
                    return failed(&request, &self.provider_id, &self.algorithm, &diagnostic);
                }
            };
            merged
                .application_class_dirs
                .extend(artifacts.application_class_dirs);
            merged.test_class_dirs.extend(artifacts.test_class_dirs);
            merged.classpath.extend(artifacts.classpath);
        }
        for paths in [
            &mut merged.application_class_dirs,
            &mut merged.test_class_dirs,
            &mut merged.classpath,
        ] {
            paths.sort();
            paths.dedup();
        }
        // A reactor artifact already loaded as application bytecode must not also
        // appear in a parent/library loader, where it could shadow that bytecode.
        merged.classpath.retain(|path| {
            !duplicates_application_module(std::path::Path::new(path), module_roots)
        });
        self.invoke_with_test_roots(&request, test_roots, targets, Some(&merged))
    }

    /// Prepares only an automatic module when a configured analysis mixes input modes.
    pub fn prepare_module_inputs(
        &self,
        module: &std::path::Path,
    ) -> Result<crate::java_analysis::JavaBytecodeInputs, String> {
        let artifacts = self.prepare_artifacts(&CallGraphRequest::new(
            module.to_string_lossy(),
            Language::Java,
        ))?;
        Ok(crate::java_analysis::JavaBytecodeInputs {
            application_class_dirs: artifacts
                .application_class_dirs
                .into_iter()
                .map(PathBuf::from)
                .collect(),
            test_class_dirs: artifacts
                .test_class_dirs
                .into_iter()
                .map(PathBuf::from)
                .collect(),
            classpath: artifacts.classpath.into_iter().map(PathBuf::from).collect(),
        })
    }

    /// Uses supplied bytecode without executing any build preparation.
    pub fn analyze_prepared_test_roots(
        &self,
        source_root: &std::path::Path,
        tests: &[JavaTestCase],
        targets: &[String],
        bytecode: &crate::java_analysis::JavaBytecodeInputs,
    ) -> CallGraphOutcome {
        let request = CallGraphRequest::new(source_root.to_string_lossy(), Language::Java);
        self.invoke_with_test_roots(
            &request,
            tests,
            targets,
            Some(&PreparedJavaArtifacts::from(bytecode)),
        )
    }

    pub fn analyze_prepared(
        &self,
        request: &CallGraphRequest,
        bytecode: &crate::java_analysis::JavaBytecodeInputs,
    ) -> CallGraphOutcome {
        self.invoke_with_test_roots(
            request,
            &[],
            &[],
            Some(&PreparedJavaArtifacts::from(bytecode)),
        )
    }

    fn invoke_with_test_roots(
        &self,
        request: &CallGraphRequest,
        tests: &[JavaTestCase],
        targets: &[String],
        artifacts: Option<&PreparedJavaArtifacts>,
    ) -> CallGraphOutcome {
        match test_root_arguments(
            std::path::Path::new(&request.source_root),
            tests,
            targets,
            artifacts,
        ) {
            Ok(arguments) => self.invoke(request, arguments),
            Err(error) => failed(request, &self.provider_id, &self.algorithm, &error),
        }
    }

    fn prepare_artifacts(
        &self,
        request: &CallGraphRequest,
    ) -> Result<PreparedJavaArtifacts, String> {
        let mut command = Command::new("java");
        command
            .arg("-cp")
            .arg(&self.jar)
            .arg("com.voyantclair.callgraph.ArtifactPreparationMain")
            .args(["--source-dir", &request.source_root])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        isolate_process_group(&mut command);
        let output = command
            .spawn()
            .and_then(|child| {
                wait_for_output(child, self.timeout, &self.provider_id)
                    .map_err(std::io::Error::other)
            })
            .map_err(|error| format!("artifact preparation failed: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "artifact preparation failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let artifacts: PreparedJavaArtifacts = serde_json::from_slice(&output.stdout)
            .map_err(|_| "artifact preparation failed: invalid preparation JSON".to_owned())?;
        if artifacts.diagnostic.trim().is_empty() {
            Ok(artifacts)
        } else {
            Err(format!(
                "artifact preparation failed: {}",
                artifacts.diagnostic
            ))
        }
    }

    fn invoke(&self, request: &CallGraphRequest, arguments: Vec<String>) -> CallGraphOutcome {
        let mut command = Command::new("java");
        command
            .arg("-jar")
            .arg(&self.jar)
            .args(["--algorithm", &self.algorithm])
            .args(arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        isolate_process_group(&mut command);
        let child = command.spawn();
        let output = child.and_then(|child| {
            wait_for_output(child, self.timeout, &self.provider_id).map_err(std::io::Error::other)
        });
        match output {
            Ok(output) if output.status.success() => {
                parse_adapter_output(request, &output.stdout, &self.provider_id, &self.algorithm)
            }
            Ok(output) => failed(
                request,
                &self.provider_id,
                &self.algorithm,
                &String::from_utf8_lossy(&output.stderr),
            ),
            Err(error) => failed(
                request,
                &self.provider_id,
                &self.algorithm,
                &error.to_string(),
            ),
        }
    }
}

pub(crate) fn duplicates_application_module(jar: &std::path::Path, modules: &[PathBuf]) -> bool {
    modules.iter().any(|module| {
        if jar.starts_with(module.join("build/libs")) {
            return true;
        }
        let Ok(properties) =
            std::fs::read_to_string(module.join("target/maven-archiver/pom.properties"))
        else {
            return false;
        };
        let values: std::collections::BTreeMap<_, _> = properties
            .lines()
            .filter_map(|line| line.split_once('='))
            .collect();
        let (Some(group), Some(artifact), Some(version)) = (
            values.get("groupId"),
            values.get("artifactId"),
            values.get("version"),
        ) else {
            return false;
        };
        let folder = PathBuf::from(group.replace('.', "/"))
            .join(artifact)
            .join(version);
        let main = folder.join(format!("{artifact}-{version}.jar"));
        let tests = folder.join(format!("{artifact}-{version}-tests.jar"));
        jar.ends_with(main) || (module.join("target/test-classes").is_dir() && jar.ends_with(tests))
    })
}

impl CallGraphProvider for JavaAdapterProvider {
    fn language(&self) -> Language {
        Language::Java
    }

    fn analyze(&self, request: &CallGraphRequest) -> CallGraphOutcome {
        self.invoke(
            request,
            vec!["--source-dir".to_owned(), request.source_root.clone()],
        )
    }
}

pub(crate) fn parse_adapter_output(
    request: &CallGraphRequest,
    output: &[u8],
    provider_id: &str,
    algorithm: &str,
) -> CallGraphOutcome {
    match serde_json::from_slice::<CallGraphOutcome>(output) {
        Ok(result) if result.schema_version == 2 => result,
        Ok(_) | Err(_) => failed(
            request,
            provider_id,
            algorithm,
            &format!("{provider_id} produced invalid call-graph JSON"),
        ),
    }
}

fn test_root_arguments(
    source_root: &std::path::Path,
    test_roots: &[JavaTestCase],
    targets: &[String],
    artifacts: Option<&PreparedJavaArtifacts>,
) -> Result<Vec<String>, String> {
    let mut arguments = vec![
        "--source-dir".to_owned(),
        source_root.to_string_lossy().into_owned(),
    ];
    if let Some(artifacts) = artifacts {
        for classes_dir in &artifacts.application_class_dirs {
            arguments.push("--classes-dir".to_owned());
            arguments.push(classes_dir.clone());
        }
        for test_classes_dir in &artifacts.test_class_dirs {
            arguments.push("--test-classes-dir".to_owned());
            arguments.push(test_classes_dir.clone());
        }
        if !artifacts.classpath.is_empty() {
            arguments.push("--classpath".to_owned());
            arguments.push(
                std::env::join_paths(&artifacts.classpath)
                    .map_err(|e| format!("invalid classpath: {e}"))?
                    .into_string()
                    .map_err(|_| "non-UTF8 classpath".to_owned())?,
            );
        }
    }
    for test in test_roots {
        arguments.push("--entrypoint".to_owned());
        arguments.push(format!(
            "{}#{}{}",
            test.class_name, test.method_name, test.descriptor
        ));
    }
    for target in targets {
        arguments.push("--target".to_owned());
        arguments.push(target.clone());
    }
    Ok(arguments)
}

#[derive(Debug, Deserialize)]
struct PreparedJavaArtifacts {
    #[serde(rename = "applicationClassDirs")]
    application_class_dirs: Vec<String>,
    #[serde(rename = "testClassDirs")]
    test_class_dirs: Vec<String>,
    classpath: Vec<String>,
    diagnostic: String,
}

impl From<&crate::java_analysis::JavaBytecodeInputs> for PreparedJavaArtifacts {
    fn from(inputs: &crate::java_analysis::JavaBytecodeInputs) -> Self {
        let strings = |paths: &[PathBuf]| {
            paths
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect()
        };
        Self {
            application_class_dirs: strings(&inputs.application_class_dirs),
            test_class_dirs: strings(&inputs.test_class_dirs),
            classpath: strings(&inputs.classpath),
            diagnostic: String::new(),
        }
    }
}

fn failed(
    request: &CallGraphRequest,
    provider_id: &str,
    algorithm: &str,
    diagnostic: &str,
) -> CallGraphOutcome {
    CallGraphOutcome {
        schema_version: 1,
        status: CallGraphStatus::Failed,
        provider_id: provider_id.into(),
        source_root: request.source_root.clone(),
        algorithm: algorithm.into(),
        diagnostics: vec![diagnostic.into()],
        reachability: vec![],
        edges: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::{isolate_process_group, test_root_arguments, wait_for_output};
    #[test]
    fn omits_only_the_maven_jar_already_loaded_as_application_bytecode() {
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("target/maven-archiver");
        std::fs::create_dir_all(&archive).unwrap();
        std::fs::write(
            archive.join("pom.properties"),
            "groupId=example.control\nartifactId=library\nversion=1\n",
        )
        .unwrap();
        let modules = vec![root.path().to_path_buf()];
        assert!(super::duplicates_application_module(
            std::path::Path::new("/cache/example/control/library/1/library-1.jar"),
            &modules
        ));
        assert!(!super::duplicates_application_module(
            std::path::Path::new("/cache/example/control/library/2/library-2.jar"),
            &modules
        ));
        assert!(!super::duplicates_application_module(
            std::path::Path::new("/cache/other/control/library/1/library-1.jar"),
            &modules
        ));
        assert!(!super::duplicates_application_module(
            std::path::Path::new("/cache/example/control/library/1/library-1-tests.jar"),
            &modules
        ));
    }
    use models::JavaTestCase;
    use std::{
        path::Path,
        process::{Command, Stdio},
        time::Duration,
    };

    #[test]
    fn preserves_phase_telemetry_when_an_adapter_times_out() {
        let child = Command::new("sh")
            .args([
                "-c",
                "echo callgraph_phase=building elapsed_ms=1 >&2; sleep 1",
            ])
            .stderr(Stdio::piped())
            .spawn()
            .expect("test command should start");

        let error = wait_for_output(child, Duration::from_millis(10), "sootup-java")
            .expect_err("a process that exceeds its deadline must time out");

        assert!(error.contains("sootup-java analysis timed out"), "{error}");
        assert!(error.contains("callgraph_phase=building"), "{error}");
    }

    #[test]
    fn isolates_adapter_process_groups() {
        let mut command = Command::new("sh");
        isolate_process_group(&mut command);
        let child = command
            .args(["-c", "sleep 1"])
            .spawn()
            .expect("test command should start");

        let error = wait_for_output(child, Duration::from_millis(10), "sootup-java")
            .expect_err("a process that exceeds its deadline must time out");

        assert!(error.contains("timed out"), "{error}");
    }

    #[test]
    fn serializes_each_test_root_as_an_adapter_entrypoint() {
        let tests = vec![JavaTestCase {
            module_root: "module".into(),
            class_name: "Lexample/ServiceTest".into(),
            method_name: "changesService".into(),
            descriptor: "()V".into(),
            callable_signature: "ServiceTest.changesService()V".into(),
            source_path: "src/test/java/example/ServiceTest.java".into(),
        }];

        assert_eq!(
            test_root_arguments(
                Path::new("module/src/main/java"),
                &tests,
                &["Lexample/Service#run/0".into()],
                None,
            )
            .unwrap(),
            vec![
                "--source-dir",
                "module/src/main/java",
                "--entrypoint",
                "Lexample/ServiceTest#changesService()V",
                "--target",
                "Lexample/Service#run/0",
            ]
        );
    }
}
