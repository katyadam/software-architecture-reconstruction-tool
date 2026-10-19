//! Explicit Java build inputs shared by reconstruction and test-impact analysis.
mod manifest;

use models::{ConfigurationData, JavaTestCase, java_analysis::JavaAnalysisContextRecord};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JavaBytecodeInputs {
    pub application_class_dirs: Vec<PathBuf>,
    pub test_class_dirs: Vec<PathBuf>,
    pub classpath: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedJavaModule {
    pub root: PathBuf,
    pub source_roots: Vec<PathBuf>,
    pub test_source_roots: Vec<PathBuf>,
    pub bytecode: JavaBytecodeInputs,
    pub dependencies: BTreeSet<PathBuf>,
    pub dependency_inventory_complete: bool,
    pub test_inventory_complete: bool,
    pub tests: Vec<JavaTestCase>,
}

#[derive(Debug, Clone)]
pub struct ConfiguredJavaService {
    pub name: String,
    pub root: PathBuf,
    pub manifest_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct ConfiguredJavaInputs {
    pub services: Vec<ConfiguredJavaService>,
    pub modules: BTreeMap<PathBuf, ResolvedJavaModule>,
    pub contexts: Vec<JavaAnalysisContextRecord>,
    pub module_inventory_complete: bool,
    pub diagnostics: Vec<String>,
}

impl ConfiguredJavaInputs {
    pub fn requires_full_test_run(&self) -> bool {
        !self.module_inventory_complete
            || self
                .modules
                .values()
                .any(|m| !m.test_inventory_complete || !m.dependency_inventory_complete)
    }
}

#[derive(Debug, Error)]
pub enum JavaInputsError {
    #[error("Java analysis configuration: {0}")]
    Configuration(String),
    #[error("Java analysis path: {0}")]
    Path(String),
    #[error("Java analysis manifest: {0}")]
    Manifest(String),
}

pub fn load_java_analysis_inputs(
    checkout: &Path,
    config_base: &Path,
    configuration: &ConfigurationData,
    candidate_revision: &str,
) -> Result<ConfiguredJavaInputs, JavaInputsError> {
    let checkout = manifest::directory(checkout, None)?;
    let base = manifest::directory(config_base, None)?;
    let mut inputs = ConfiguredJavaInputs {
        module_inventory_complete: true,
        ..Default::default()
    };
    for service in &configuration.service_descriptions {
        if service.base_dir_path.trim().is_empty() {
            return Err(JavaInputsError::Configuration(format!(
                "service {} has an empty base_dir_path",
                service.name
            )));
        }
        let root = manifest::directory(&base.join(&service.base_dir_path), None)?;
        if !root.starts_with(&checkout) {
            if service.java_analysis.is_some() {
                return Err(JavaInputsError::Configuration(format!(
                    "configured Java service {} is outside candidate checkout: {}",
                    service.name,
                    root.display()
                )));
            }
            inputs.diagnostics.push(format!(
                "service {} outside candidate checkout: {}",
                service.name,
                root.display()
            ));
            continue;
        }
        if inputs
            .services
            .iter()
            .any(|other| root.starts_with(&other.root) || other.root.starts_with(&root))
        {
            return Err(JavaInputsError::Configuration(format!(
                "duplicate or overlapping service root: {}",
                root.display()
            )));
        }
        let manifest_path = match &service.java_analysis {
            Some(settings) => match &settings.artifacts_file {
                Some(file) => {
                    if file.trim().is_empty() {
                        return Err(JavaInputsError::Configuration(format!(
                            "service {} has an empty artifacts_file",
                            service.name
                        )));
                    }
                    let path = Path::new(file);
                    Some(manifest::file(
                        &root.join(path),
                        (!path.is_absolute()).then_some(root.as_path()),
                    )?)
                }
                None if !settings.maven_profiles.is_empty() => {
                    return Err(JavaInputsError::Configuration(format!(
                        "service {}: maven_profiles requires artifacts_file",
                        service.name
                    )));
                }
                None => None,
            },
            None => None,
        };
        if let Some(path) = &manifest_path {
            manifest::load(
                &checkout,
                &root,
                service,
                path,
                candidate_revision,
                &mut inputs,
            )
            .map_err(|e| {
                JavaInputsError::Manifest(format!(
                    "service {} ({}): {e}",
                    service.name,
                    path.display()
                ))
            })?;
        }
        inputs.services.push(ConfiguredJavaService {
            name: service.name.clone(),
            root,
            manifest_path,
        });
    }
    for module in inputs.modules.values() {
        for dep in &module.dependencies {
            if !inputs.modules.contains_key(dep) {
                return Err(JavaInputsError::Manifest(format!(
                    "{} dependency {} has no module entry",
                    module.root.display(),
                    dep.display()
                )));
            }
        }
    }
    Ok(inputs)
}
