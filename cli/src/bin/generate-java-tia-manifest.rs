use anyhow::{Context, Result};
use clap::Parser;
use extractor_runtime::impact::{checkout_modules, discover_java_tests_in_roots};
use models::{
    JavaTestCase,
    java_analysis::{JavaArtifactManifest, JavaBuildContext, JavaModuleManifest},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Generates a tia-artifacts.json manifest for a Maven project by scanning test sources.
#[derive(Parser, Debug)]
#[command(
    about = "Generate a tia-artifacts.json manifest for configured Java TIA from Maven source files"
)]
struct Arguments {
    /// Maven project root to scan.
    #[arg(long, value_name = "DIR")]
    project_dir: PathBuf,
    /// Destination path for the generated tia-artifacts.json.
    #[arg(long, value_name = "FILE")]
    output: PathBuf,
    /// Optional: also write a service-config.json pointing to the manifest.
    #[arg(long, value_name = "FILE")]
    service_config: Option<PathBuf>,
    /// Service name used in the generated service-config.json (defaults to project dir name).
    #[arg(long, value_name = "NAME")]
    service_name: Option<String>,
    /// Build tool recorded in the manifest build_context.
    #[arg(long, default_value = "maven", value_name = "TOOL")]
    build_tool: String,
    /// Java version recorded in the manifest build_context.
    #[arg(long, default_value = "17", value_name = "VERSION")]
    java_version: String,
    /// Comma-separated Maven profiles recorded in the manifest build_context.
    #[arg(long, value_name = "PROFILES")]
    profiles: Option<String>,
}

fn main() {
    let arguments = Arguments::parse();
    if let Err(error) = run(arguments) {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run(arguments: Arguments) -> Result<()> {
    let project = arguments
        .project_dir
        .canonicalize()
        .with_context(|| format!("cannot access project dir: {}", arguments.project_dir.display()))?;

    let candidate_revision = git_revision(&project, "HEAD")
        .context("failed to read git HEAD revision")?;

    let profiles: Vec<String> = arguments
        .profiles
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();

    let modules =
        checkout_modules(&project).context("failed to discover Maven modules")?;

    let mut manifest_modules = Vec::new();
    for module in &modules {
        manifest_modules.push(module_entry(module, &project)?);
    }

    let manifest = JavaArtifactManifest {
        schema_version: 1,
        candidate_revision,
        build_context: JavaBuildContext {
            tool: arguments.build_tool,
            requested_profiles: profiles.clone(),
            active_profiles: profiles,
            java_version: arguments.java_version,
        },
        module_inventory_complete: true,
        modules: manifest_modules,
    };

    if let Some(parent) = arguments.output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create output directory: {}", parent.display()))?;
        }
    }
    let json = serde_json::to_string_pretty(&manifest)?;
    fs::write(&arguments.output, json)
        .with_context(|| format!("cannot write manifest: {}", arguments.output.display()))?;
    eprintln!("wrote {}", arguments.output.display());

    if let Some(sc_path) = arguments.service_config {
        let service_name = arguments
            .service_name
            .as_deref()
            .or_else(|| project.file_name()?.to_str())
            .unwrap_or("service")
            .to_owned();
        let artifacts_file = arguments
            .output
            .canonicalize()
            .ok()
            .and_then(|p| p.to_str().map(str::to_owned))
            .unwrap_or_else(|| arguments.output.to_string_lossy().into_owned());
        write_service_config(&service_name, &artifacts_file, &sc_path)?;
    }

    Ok(())
}

fn module_entry(module: &Path, project: &Path) -> Result<JavaModuleManifest> {
    let src_root = module.join("src/main/java");
    let test_root = module.join("src/test/java");
    // Maven: target/classes; Gradle: build/classes/java/main
    let classes_dir = first_existing(&[
        module.join("target/classes"),
        module.join("build/classes/java/main"),
    ]);
    // Maven: target/test-classes; Gradle: build/classes/java/test
    let test_classes_dir = first_existing(&[
        module.join("target/test-classes"),
        module.join("build/classes/java/test"),
    ]);

    let tests = discover_java_tests_in_roots(module, &[test_root.clone()])
        .with_context(|| format!("failed to scan tests in {}", module.display()))?;

    let tests = relocate_tests(tests, module);

    Ok(JavaModuleManifest {
        root: relative_posix(module, project),
        source_roots: if src_root.is_dir() {
            vec![relative_posix(&src_root, module)]
        } else {
            vec![]
        },
        test_source_roots: if test_root.is_dir() {
            vec![relative_posix(&test_root, module)]
        } else {
            vec![]
        },
        application_class_dirs: classes_dir
            .as_deref()
            .map(|d| vec![relative_posix(d, module)])
            .unwrap_or_default(),
        test_class_dirs: test_classes_dir
            .as_deref()
            .map(|d| vec![relative_posix(d, module)])
            .unwrap_or_default(),
        classpath: vec![],
        dependencies: vec![],
        dependency_inventory_complete: true,
        test_inventory_complete: true,
        tests,
    })
}

/// Returns a project-relative POSIX path string.
fn relative_posix(path: &Path, project: &Path) -> String {
    path.strip_prefix(project)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Returns the first path in the slice that exists as a directory, if any.
fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.is_dir()).cloned()
}

/// Converts absolute module_root and source_path fields to module-relative POSIX paths.
fn relocate_tests(tests: Vec<JavaTestCase>, module: &Path) -> Vec<JavaTestCase> {
    tests
        .into_iter()
        .map(|t| {
            let module_root = relative_posix(Path::new(&t.module_root), module);
            let source_path = relative_posix(Path::new(&t.source_path), module);
            JavaTestCase {
                module_root,
                source_path,
                ..t
            }
        })
        .collect()
}

fn write_service_config(service_name: &str, artifacts_file: &str, path: &Path) -> Result<()> {
    let config = serde_json::json!({
        "service_descriptions": [{
            "name": service_name,
            "base_dir_path": ".",
            "urls": [],
            "java_analysis": {
                "maven_profiles": [],
                "artifacts_file": artifacts_file
            }
        }]
    });
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let json = serde_json::to_string_pretty(&config)?;
    fs::write(path, json)
        .with_context(|| format!("cannot write service-config: {}", path.display()))?;
    eprintln!("wrote {}", path.display());
    Ok(())
}

fn git_revision(project: &Path, rev: &str) -> Result<String> {
    let output = Command::new("git")
        .current_dir(project)
        .args(["rev-parse", "--verify", rev])
        .output()
        .context("git rev-parse failed")?;
    if !output.status.success() {
        anyhow::bail!(
            "git rev-parse returned non-zero: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8(output.stdout)
        .context("git output is not valid UTF-8")?
        .trim()
        .to_owned())
}
