use anyhow::{Context, Result};
pub mod configured_java_impact;
pub mod go_impact_output;
pub mod java_impact_inputs;
use extractor_runtime::call_graph::{CallGraphProvider, resolve_edges, wala::WalaJavaProvider};
use extractor_runtime::pipeline::pass3::{pass_attr, pass_module};
use extractor_runtime::pipeline::{build_project_ir, dispatch_syntactic, evaluate};
use models::{Callable, CodeElementsAggregate, ConfigurationData, ir::syntax::FileRecord};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

pub fn get_all_code_elements(
    project_dir: &PathBuf,
    external_constants: &HashMap<String, String>,
) -> Result<CodeElementsAggregate> {
    get_all_code_elements_with_progress(project_dir, external_constants, true)
}

/// Extracts code elements while optionally reporting per-file progress to stdout.
pub fn get_all_code_elements_with_progress(
    project_dir: &PathBuf,
    external_constants: &HashMap<String, String>,
    report_progress: bool,
) -> Result<CodeElementsAggregate> {
    get_code_elements_with_progress(project_dir, external_constants, report_progress, |_| true)
}

/// Extracts Java code elements while optionally reporting per-file progress to stdout.
pub fn get_java_code_elements_with_progress(
    project_dir: &PathBuf,
    external_constants: &HashMap<String, String>,
    report_progress: bool,
) -> Result<CodeElementsAggregate> {
    get_code_elements_with_progress(project_dir, external_constants, report_progress, |path| {
        path.extension()
            .is_some_and(|extension| extension == "java")
    })
}

/// Extracts Java code elements from the affected module roots only.
pub fn get_java_code_elements_for_modules_with_progress(
    module_roots: &[PathBuf],
    external_constants: &HashMap<String, String>,
    report_progress: bool,
) -> Result<CodeElementsAggregate> {
    let paths = java_paths_for_modules(module_roots)?;
    get_code_elements_from_paths_with_progress(paths, external_constants, report_progress)
}

/// Collects syntactic Java callable metadata from the affected module roots.
pub fn get_java_callables_for_modules_with_progress(
    module_roots: &[PathBuf],
    report_progress: bool,
) -> Result<Vec<Callable>> {
    let main_roots: Vec<_> = module_roots
        .iter()
        .map(|module_root| module_root.join("src/main/java"))
        .collect();
    get_java_callables_in_roots_with_progress(&main_roots, report_progress)
}

pub fn get_java_callables_in_roots_with_progress(
    source_roots: &[PathBuf],
    report_progress: bool,
) -> Result<Vec<Callable>> {
    let paths = java_paths_for_modules(source_roots)?;
    let files_to_process = paths.len();
    if report_progress {
        println!("Total files found: {files_to_process}");
    }
    let mut callables = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        if report_progress {
            println!(
                "Extracting ({}/{}): {:?}",
                index + 1,
                files_to_process,
                path.file_name().unwrap_or_default()
            );
        }
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                eprintln!(
                    "⚠️  Skipping file {:?}: {}",
                    path.file_name().unwrap_or_default(),
                    error
                );
                continue;
            }
        };
        callables.extend(
            java_extractor::extract_syntactic_callables(&source, &path.to_string_lossy())
                .with_context(|| format!("Unable to extract Java callables from {path:?}"))?,
        );
    }
    Ok(callables)
}

fn java_paths_for_modules(module_roots: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut roots: Vec<_> = module_roots.iter().cloned().collect();
    roots.sort();
    roots.dedup();

    let mut paths = roots
        .into_iter()
        .try_fold(Vec::new(), |mut paths, module_root| {
            paths.extend(collect_files(&module_root)?.into_iter().filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "java")
            }));
            Ok::<_, anyhow::Error>(paths)
        })?;
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn get_code_elements_with_progress(
    project_dir: &PathBuf,
    external_constants: &HashMap<String, String>,
    report_progress: bool,
    include_path: impl Fn(&std::path::Path) -> bool,
) -> Result<CodeElementsAggregate> {
    let paths: Vec<_> = collect_files(project_dir)?
        .into_iter()
        .filter(|path| include_path(path))
        .collect();
    get_code_elements_from_paths_with_progress(paths, external_constants, report_progress)
}

fn get_code_elements_from_paths_with_progress(
    paths: Vec<PathBuf>,
    external_constants: &HashMap<String, String>,
    report_progress: bool,
) -> Result<CodeElementsAggregate> {
    let file_records = collect_syntactic_records(paths, report_progress)?;
    let project_ir = build_project_ir(file_records);

    let per_file_attrs = pass_attr::resolve_all(&project_ir, external_constants);
    let per_file_module_consts =
        pass_module::resolve_all(&project_ir, external_constants, &per_file_attrs);
    let evaluated_ir = evaluate(
        project_ir,
        external_constants,
        &per_file_attrs,
        &per_file_module_consts,
    );
    Ok(CodeElementsAggregate::from(evaluated_ir))
}

fn collect_syntactic_records(
    paths: Vec<PathBuf>,
    report_progress: bool,
) -> Result<Vec<FileRecord>> {
    let files_to_process = paths.len();
    if report_progress {
        println!("Total files found: {files_to_process}");
    }

    let mut file_records = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        let code = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!(
                    "⚠️  Skipping file {:?}: {}",
                    path.file_name().unwrap_or_default(),
                    e
                );
                continue;
            }
        };
        if report_progress {
            println!(
                "Extracting ({}/{files_to_process}): {:?}",
                i + 1,
                path.file_name().unwrap_or_default()
            );
        }
        if let Some(record) = dispatch_syntactic(&code, path.to_str().unwrap_or_default())
            .with_context(|| format!("Error dispatching file: {:?}", path))?
        {
            file_records.push(record);
        }
    }

    Ok(file_records)
}

/// Adds uniquely resolved WALA Java edges for configured service directories when enabled.
///
/// A missing adapter JAR, a deadline breach, or a non-Java service leaves syntactic extraction unchanged.
pub fn enrich_with_wala(
    aggregate: CodeElementsAggregate,
    project_dir: &PathBuf,
    configuration: &ConfigurationData,
    jar: Option<&std::path::Path>,
    wala_timeout: Duration,
) -> CodeElementsAggregate {
    enrich_with_wala_inputs(
        aggregate,
        project_dir,
        configuration,
        jar,
        wala_timeout,
        None,
    )
}

/// Reuses declared bytecode and generated source roots without building the service.
pub fn enrich_with_wala_inputs(
    mut aggregate: CodeElementsAggregate,
    project_dir: &std::path::Path,
    configuration: &ConfigurationData,
    jar: Option<&std::path::Path>,
    wala_timeout: Duration,
    inputs: Option<&extractor_runtime::java_analysis::ConfiguredJavaInputs>,
) -> CodeElementsAggregate {
    let Some(jar) = jar else {
        return aggregate;
    };
    let project_root = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());
    if inputs.is_some() {
        for callable in &mut aggregate.callables {
            if let Ok(path) = std::path::Path::new(&callable.file_path).canonicalize() {
                callable.file_path = path.to_string_lossy().into();
            }
        }
    }
    let provider = WalaJavaProvider::with_timeout(jar, wala_timeout);
    for service in &configuration.service_descriptions {
        let root = project_dir.join(&service.base_dir_path);
        if !root.is_dir() {
            continue;
        }
        let root = if inputs.is_some() {
            root.canonicalize().unwrap_or(root)
        } else {
            root.components().collect()
        };
        let configured = inputs.and_then(|inputs| {
            inputs
                .contexts
                .iter()
                .find(|context| std::path::Path::new(&context.service_root) == root)
        });
        let (edges, diagnostics) = if let (Some(inputs), Some(context)) = (inputs, configured) {
            let modules = context
                .module_roots
                .iter()
                .filter_map(|root| inputs.modules.get(std::path::Path::new(root)))
                .collect::<Vec<_>>();
            let source_roots = modules
                .iter()
                .flat_map(|m| m.source_roots.clone())
                .collect::<Vec<_>>();
            match get_java_callables_in_roots_with_progress(&source_roots, false) {
                Ok(callables) => {
                    for callable in callables {
                        if !aggregate.callables.iter().any(|c| {
                            c.file_path == callable.file_path && c.signature == callable.signature
                        }) {
                            aggregate.callables.push(callable);
                        }
                    }
                }
                Err(error) => {
                    eprintln!("WALA configured source extraction failed: {error}");
                    continue;
                }
            }
            let bytecode = extractor_runtime::java_analysis::JavaBytecodeInputs::merge(
                &modules.iter().map(|m| &m.bytecode).collect::<Vec<_>>(),
                &modules.iter().map(|m| m.root.clone()).collect::<Vec<_>>(),
            );
            let outcome = provider.analyze_prepared(
                &models::call_graph::CallGraphRequest::new(
                    project_root.to_string_lossy(),
                    models::call_graph::Language::Java,
                ),
                &bytecode,
            );
            let layout = extractor_runtime::call_graph::JavaSourceLayout {
                production_roots: source_roots,
                test_roots: vec![],
            };
            extractor_runtime::call_graph::resolve_edges_with_java_layout(
                &outcome,
                &project_root,
                &aggregate.callables,
                &layout,
            )
        } else {
            let outcome = provider.analyze(&models::call_graph::CallGraphRequest::new(
                root.to_string_lossy(),
                models::call_graph::Language::Java,
            ));
            resolve_edges(&outcome, &root, &aggregate.callables)
        };
        for diagnostic in diagnostics {
            eprintln!("WALA: {diagnostic}");
        }
        for edge in edges {
            if !aggregate.resolved_call_edges.contains(&edge) {
                aggregate.resolved_call_edges.push(edge);
            }
        }
    }
    aggregate
}

/// Recursively collects project files while excluding Git metadata and generated build output.
pub fn collect_files(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut results = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if matches!(entry.file_name().to_str(), Some(".git" | "target")) {
                continue;
            }
            if path.is_dir() {
                results.extend(collect_files(&path)?);
            } else {
                results.push(path);
            }
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::{
        get_all_code_elements_with_progress, get_java_callables_for_modules_with_progress,
        get_java_code_elements_for_modules_with_progress, get_java_code_elements_with_progress,
    };
    use std::{collections::HashMap, fs, path::Path};

    #[test]
    fn java_extraction_ignores_non_java_and_binary_project_files() {
        let project = tempfile::tempdir().expect("temporary project directory");
        let source = project.path().join("src/main/java/example/Service.java");
        fs::create_dir_all(source.parent().expect("source directory"))
            .expect("source directory writes");
        fs::write(
            &source,
            "package example; public class Service { public void run() {} }",
        )
        .expect("Java source writes");
        fs::write(project.path().join("notes.txt"), "not Java").expect("text file writes");
        fs::write(project.path().join("image.bin"), [0, 159, 146, 150])
            .expect("binary file writes");
        let python = project.path().join("helper.py");
        fs::write(&python, "def helper():\n    return 1\n").expect("Python source writes");

        let aggregate = get_java_code_elements_with_progress(
            &project.path().to_path_buf(),
            &HashMap::new(),
            false,
        )
        .expect("Java extraction succeeds");

        assert!(
            aggregate
                .callables
                .iter()
                .any(|callable| callable.file_path == source.to_string_lossy()),
            "expected Java callable, got {:?}",
            aggregate.callables
        );
        assert!(
            aggregate
                .callables
                .iter()
                .all(|callable| callable.file_path != python.to_string_lossy())
        );

        let all_languages = get_all_code_elements_with_progress(
            &project.path().to_path_buf(),
            &HashMap::new(),
            false,
        )
        .expect("general extraction succeeds");
        assert!(
            all_languages
                .callables
                .iter()
                .any(|callable| callable.file_path == python.to_string_lossy())
        );
    }

    #[test]
    fn module_java_extraction_ignores_unaffected_java_modules() {
        let project = tempfile::tempdir().expect("temporary project directory");
        let changed_module = project.path().join("changed");
        let changed_source = changed_module.join("src/main/java/example/Changed.java");
        let unaffected_source = project
            .path()
            .join("unaffected/src/main/java/example/Unchanged.java");
        write_java_source(
            &changed_source,
            "package example; public class Changed { public void run() {} }",
        );
        write_java_source(
            &unaffected_source,
            "package example; public class Unchanged { public void run() {} }",
        );

        let aggregate = get_java_code_elements_for_modules_with_progress(
            &[changed_module.clone(), changed_module],
            &HashMap::new(),
            false,
        )
        .expect("module Java extraction succeeds");
        assert!(
            aggregate
                .callables
                .iter()
                .any(|callable| callable.file_path == changed_source.to_string_lossy())
        );
        assert!(
            !aggregate
                .callables
                .iter()
                .any(|callable| callable.file_path == unaffected_source.to_string_lossy())
        );
        assert_eq!(
            aggregate
                .callables
                .iter()
                .filter(|callable| callable.file_path == changed_source.to_string_lossy())
                .count(),
            1,
            "duplicate module roots must not duplicate callables"
        );
    }

    #[test]
    fn java_callable_collection_returns_syntactic_metadata_from_selected_modules() {
        let project = tempfile::tempdir().expect("temporary project directory");
        let module = project.path().join("module");
        let source = module.join("src/main/java/example/Service.java");
        let test_source = module.join("src/test/java/example/ServiceTest.java");
        write_java_source(
            &source,
            "package example; public class Service { public void run() {} }",
        );
        write_java_source(
            &test_source,
            "package example; public class ServiceTest { public void covers() {} }",
        );

        let callables = get_java_callables_for_modules_with_progress(&[module], false)
            .expect("callable collection succeeds");

        assert!(callables.iter().any(|callable| {
            callable.file_path == source.to_string_lossy() && callable.name == "void run()"
        }));
        assert!(
            callables
                .iter()
                .all(|callable| callable.file_path != test_source.to_string_lossy())
        );
    }

    #[test]
    fn java_callable_collection_retains_generic_methods_without_symbolic_bodies() {
        let project = tempfile::tempdir().expect("temporary project directory");
        let module = project.path().join("module");
        let source = module.join("src/main/java/example/Transformation.java");
        write_java_source(
            &source,
            "package example; public class Transformation { public void configure(final Map<String, ?> configs) { unsupported(configs); } }",
        );

        let callables = get_java_callables_for_modules_with_progress(&[module], false)
            .expect("callable collection succeeds");

        assert!(callables.iter().any(|callable| {
            callable.file_path == source.to_string_lossy()
                && callable.name == "void configure(final Map<String, ?> configs)"
        }));
    }

    fn write_java_source(path: &Path, source: &str) {
        fs::create_dir_all(path.parent().expect("source directory"))
            .expect("source directory writes");
        fs::write(path, source).expect("Java source writes");
    }
}
