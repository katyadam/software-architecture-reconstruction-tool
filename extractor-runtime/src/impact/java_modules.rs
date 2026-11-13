use quick_xml::{Reader, events::Event};
use regex::Regex;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

/// Records declared Java build modules or a conservative reason that metadata is unusable.
#[derive(Clone)]
pub struct ModuleCatalog {
    root: PathBuf,
    state: CatalogState,
    configured_roots: Vec<PathBuf>,
    configured_sources: Vec<PathBuf>,
}

#[derive(Clone)]
enum CatalogState {
    Complete(Vec<PathBuf>),
    Partial(Vec<PathBuf>, String),
    Uncertain(String),
}

impl ModuleCatalog {
    /// Loads Java build metadata; parsing is added by the Maven and Gradle catalog stages.
    pub fn load(project_root: &Path) -> Self {
        let root = normalized(project_root);
        if root.join("pom.xml").is_file() {
            return match maven_modules(&root) {
                Ok(modules) if modules.profile_defined => Self::partial(
                    root,
                    modules.roots,
                    "Maven profiles define additional modules; profile-dependent paths use broad fallback"
                        .into(),
                ),
                Ok(modules) => Self::complete(root, modules.roots),
                Err(diagnostic) => Self::uncertain(root, diagnostic),
            };
        }
        if let Some(settings) = [
            root.join("settings.gradle"),
            root.join("settings.gradle.kts"),
        ]
        .into_iter()
        .find(|path| path.is_file())
        {
            return match gradle_modules(&root, &settings) {
                Ok(modules) => Self::complete(root, modules),
                Err(diagnostic) => Self::uncertain(root, diagnostic),
            };
        }
        Self::uncertain(root, "no Maven or Gradle build metadata found".into())
    }

    pub fn with_configured_inputs(
        &self,
        inputs: &crate::java_analysis::ConfiguredJavaInputs,
    ) -> Self {
        let mut catalog = self.clone();
        for module in inputs.modules.values() {
            catalog.configured_roots.push(module.root.clone());
            catalog
                .configured_sources
                .extend(module.source_roots.clone());
            catalog
                .configured_sources
                .extend(module.test_source_roots.clone());
        }
        for service in &inputs.services {
            if service.manifest_path.is_none() {
                catalog.configured_roots.push(service.root.clone());
                catalog
                    .configured_sources
                    .push(service.root.join("src/main/java"));
                catalog
                    .configured_sources
                    .push(service.root.join("src/test/java"));
            }
        }
        catalog
    }

    /// Returns the deepest declared module containing a candidate-relative source path.
    pub fn owner_for(&self, source_path: &str) -> PathBuf {
        if let Some(candidate) = normalized_candidate(&self.root, source_path) {
            let automatic = match &self.state {
                CatalogState::Complete(m) | CatalogState::Partial(m, _) => m.as_slice(),
                _ => &[],
            };
            if let Some(owner) = automatic
                .iter()
                .chain(&self.configured_roots)
                .filter(|p| candidate.starts_with(p))
                .max_by_key(|p| p.components().count())
            {
                return owner.clone();
            }
        }
        match &self.state {
            CatalogState::Complete(modules) | CatalogState::Partial(modules, _) => {
                let Some(candidate) = normalized_candidate(&self.root, source_path) else {
                    return self.root.clone();
                };
                modules
                    .iter()
                    .filter(|module| candidate.starts_with(module))
                    .max_by_key(|module| module.components().count())
                    .cloned()
                    .unwrap_or_else(|| self.root.clone())
            }
            CatalogState::Uncertain(_) => self.root.clone(),
        }
    }

    /// Returns declared descendants for a complete catalog, or the supplied root when uncertain.
    pub fn descendants_of(&self, module_root: &Path) -> Vec<PathBuf> {
        match &self.state {
            CatalogState::Complete(modules) | CatalogState::Partial(modules, _) => modules
                .iter()
                .filter(|module| module.starts_with(module_root))
                .cloned()
                .collect(),
            CatalogState::Uncertain(_) => vec![module_root.to_path_buf()],
        }
    }

    /// Explains why metadata cannot safely narrow module ownership.
    pub fn diagnostic(&self) -> Option<&str> {
        match &self.state {
            CatalogState::Complete(_) => None,
            CatalogState::Partial(_, diagnostic) | CatalogState::Uncertain(diagnostic) => {
                Some(diagnostic)
            }
        }
    }

    pub fn is_complete(&self) -> bool {
        matches!(self.state, CatalogState::Complete(_))
    }

    /// Returns whether a Java source path has a fully known module scope.
    pub fn can_narrow_source(&self, source_path: &str) -> bool {
        let Some(candidate) = normalized_candidate(&self.root, source_path) else {
            return false;
        };
        if self
            .configured_sources
            .iter()
            .any(|p| candidate.starts_with(p))
        {
            return true;
        }
        match &self.state {
            CatalogState::Complete(_) => true,
            CatalogState::Partial(modules, _) => modules
                .iter()
                .any(|module| module != &self.root && candidate.starts_with(module)),
            CatalogState::Uncertain(_) => false,
        }
    }

    pub(super) fn complete(root: PathBuf, modules: Vec<PathBuf>) -> Self {
        Self {
            root,
            configured_roots: Vec::new(),
            configured_sources: Vec::new(),
            state: CatalogState::Complete(modules),
        }
    }

    pub(super) fn uncertain(root: PathBuf, diagnostic: String) -> Self {
        Self {
            root,
            configured_roots: Vec::new(),
            configured_sources: Vec::new(),
            state: CatalogState::Uncertain(diagnostic),
        }
    }

    fn partial(root: PathBuf, modules: Vec<PathBuf>, diagnostic: String) -> Self {
        Self {
            root,
            configured_roots: Vec::new(),
            configured_sources: Vec::new(),
            state: CatalogState::Partial(modules, diagnostic),
        }
    }
}

/// Normalizes existing roots so path-prefix ownership checks are stable.
fn normalized(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .expect("current directory exists")
            .join(path)
    };
    absolute.canonicalize().unwrap_or(absolute)
}

/// Resolves a Git-relative path without letting it escape the catalog root.
fn normalized_candidate(root: &Path, source_path: &str) -> Option<PathBuf> {
    let mut candidate = root.to_path_buf();
    for component in Path::new(source_path).components() {
        match component {
            Component::Normal(part) => candidate.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if candidate == root {
                    return None;
                }
                candidate.pop();
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(candidate)
}

/// Collects only modules directly declared by each reachable Maven reactor POM.
struct MavenModules {
    roots: Vec<PathBuf>,
    profile_defined: bool,
}

fn maven_modules(root: &Path) -> Result<MavenModules, String> {
    let mut modules = MavenModules {
        roots: vec![root.to_path_buf()],
        profile_defined: false,
    };
    collect_maven_modules(root, root, &mut modules)?;
    Ok(modules)
}

fn collect_maven_modules(
    root: &Path,
    module: &Path,
    modules: &mut MavenModules,
) -> Result<(), String> {
    let declarations = maven_declared_modules(&module.join("pom.xml"))?;
    modules.profile_defined |= declarations.profile_defined;
    for declared in declarations.unconditional {
        if declared.contains("${") {
            return Err(format!(
                "Maven module path uses property expansion: {declared}"
            ));
        }
        let child = module.join(&declared);
        let Some(child) = child.canonicalize().ok() else {
            return Err(format!("Maven module path does not exist: {declared}"));
        };
        if !child.starts_with(root) {
            return Err(format!(
                "Maven module path escapes project root: {declared}"
            ));
        }
        if !child.join("pom.xml").is_file() {
            return Err(format!("Maven module has no POM: {declared}"));
        }
        if modules.roots.contains(&child) {
            return Err(format!(
                "Maven module is declared more than once: {declared}"
            ));
        }
        modules.roots.push(child.clone());
        collect_maven_modules(root, &child, modules)?;
    }
    Ok(())
}

struct MavenModuleDeclarations {
    unconditional: Vec<String>,
    profile_defined: bool,
}

fn maven_declared_modules(pom: &Path) -> Result<MavenModuleDeclarations, String> {
    let xml = fs::read_to_string(pom).map_err(|error| error.to_string())?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut elements: Vec<String> = Vec::new();
    let mut module_value: Option<String> = None;
    let mut modules = Vec::new();
    let mut profile_defined = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                let name = String::from_utf8_lossy(element.name().as_ref()).into_owned();
                if name == "module" && elements.as_slice() == ["project", "modules"] {
                    module_value = Some(String::new());
                } else if name == "module"
                    && elements.as_slice() == ["project", "profiles", "profile", "modules"]
                {
                    profile_defined = true;
                } else if module_value.is_some() {
                    return Err("Maven module declaration has nested content".into());
                }
                elements.push(name);
            }
            Ok(Event::Text(text)) if module_value.is_some() => module_value
                .as_mut()
                .expect("module value exists")
                .push_str(&text.decode().map_err(|error| error.to_string())?),
            Ok(Event::CData(text)) if module_value.is_some() => module_value
                .as_mut()
                .expect("module value exists")
                .push_str(&text.decode().map_err(|error| error.to_string())?),
            Ok(Event::End(element)) => {
                let name = String::from_utf8_lossy(element.name().as_ref()).into_owned();
                if name == "module" && module_value.is_some() {
                    let value = module_value.take().expect("module value exists");
                    if value.is_empty() {
                        return Err("Maven module path is empty".into());
                    }
                    modules.push(value);
                }
                elements.pop();
            }
            Ok(Event::Eof) => {
                return Ok(MavenModuleDeclarations {
                    unconditional: modules,
                    profile_defined,
                });
            }
            Err(error) => return Err(format!("unable to parse Maven POM: {error}")),
            _ => {}
        }
    }
}

fn gradle_modules(root: &Path, settings: &Path) -> Result<Vec<PathBuf>, String> {
    let source = fs::read_to_string(settings).map_err(|error| error.to_string())?;
    let mut projects = Vec::new();
    for line in source.lines().map(str::trim) {
        if !line.contains("include") {
            continue;
        }
        if !line.starts_with("include") {
            return Err(format!(
                "Gradle include is not a top-level literal declaration: {line}"
            ));
        }
        let remainder = line["include".len()..]
            .trim()
            .trim_matches(|c| c == '(' || c == ')');
        for value in remainder.split(',').map(str::trim) {
            let literal = value.trim_matches(['\'', '"']);
            if literal == value || !literal.starts_with(':') || literal.contains('$') {
                return Err(format!(
                    "Gradle include is not a literal project path: {line}"
                ));
            }
            projects.push(literal.to_owned());
        }
    }
    let assignment =
        Regex::new(r#"(?m)^\s*project\(\s*[\"'](:[^\"']+)[\"']\s*\)\.projectDir\s*=\s*(.+?)\s*$"#)
            .expect("Gradle projectDir assignment pattern is valid");
    let file_mapping = Regex::new(r#"^file\(\s*[\"']([^\"']+)[\"']\s*\)$"#)
        .expect("Gradle file projectDir pattern is valid");
    let root_dir_mapping = Regex::new(r#"^\"\$rootDir/([^\"$]+)\"\s+as\s+File$"#)
        .expect("Gradle rootDir projectDir pattern is valid");
    let mut modules = vec![root.to_path_buf()];
    for project in projects {
        let assignments: Vec<_> = assignment
            .captures_iter(&source)
            .filter(|captures| captures[1] == project)
            .collect();
        let path = match assignments.as_slice() {
            [] => root.join(project.trim_start_matches(':').replace(':', "/")),
            [captures] => {
                let value = captures[2].trim();
                let literal = file_mapping
                    .captures(value)
                    .and_then(|captures| captures.get(1))
                    .filter(|path| !path.as_str().contains('$'))
                    .or_else(|| {
                        root_dir_mapping
                            .captures(value)
                            .and_then(|captures| captures.get(1))
                    })
                    .ok_or_else(|| {
                        format!(
                            "Gradle projectDir is not a supported literal assignment for {project}"
                        )
                    })?;
                root.join(literal.as_str())
            }
            _ => {
                return Err(format!(
                    "Gradle projectDir is declared more than once: {project}"
                ));
            }
        };
        let Some(path) = path.canonicalize().ok() else {
            return Err(format!(
                "Gradle project directory does not exist: {project}"
            ));
        };
        if !path.starts_with(root) {
            return Err(format!(
                "Gradle project path escapes project root: {project}"
            ));
        }
        if !modules.contains(&path) {
            modules.push(path);
        } else {
            return Err(format!("Gradle project declared more than once: {project}"));
        }
    }
    Ok(modules)
}

#[cfg(test)]
mod tests {
    use super::ModuleCatalog;
    use std::{fs, path::Path};

    #[test]
    fn falls_back_to_the_project_root_without_declared_metadata() {
        let directory = tempfile::tempdir().expect("temporary project root exists");
        let root = directory.path().canonicalize().expect("root canonicalizes");
        let catalog = ModuleCatalog::load(&root);

        assert_eq!(
            catalog.owner_for("src/main/java/example/Service.java"),
            root
        );
        assert_eq!(catalog.descendants_of(&root), vec![root.clone()]);
        assert!(catalog.diagnostic().is_some());
    }

    #[test]
    fn assigns_a_path_to_its_deepest_declared_module() {
        let directory = tempfile::tempdir().expect("temporary project root exists");
        let root = directory.path().canonicalize().expect("root canonicalizes");
        let orders = root.join("orders");
        fs::create_dir_all(&orders).expect("declared module exists");
        let catalog = ModuleCatalog::complete(root.clone(), vec![root.clone(), orders.clone()]);

        assert_eq!(
            catalog.owner_for("orders/src/main/java/example/Order.java"),
            orders
        );
        assert_eq!(catalog.descendants_of(&root), vec![root, orders]);
        assert!(catalog.diagnostic().is_none());
    }

    #[test]
    fn normalizes_candidate_paths_before_matching_declared_modules() {
        let directory = tempfile::tempdir().expect("temporary project root exists");
        let root = directory.path().canonicalize().expect("root canonicalizes");
        let orders = root.join("orders");
        let billing = root.join("billing");
        fs::create_dir_all(&orders).expect("orders module exists");
        fs::create_dir_all(&billing).expect("billing module exists");
        let catalog =
            ModuleCatalog::complete(root.clone(), vec![root.clone(), orders, billing.clone()]);

        assert_eq!(
            catalog.owner_for("orders/../billing/src/main/java/example/Billing.java"),
            billing
        );
        assert_eq!(catalog.owner_for("../outside/Service.java"), root);
    }

    #[test]
    fn maven_catalog_uses_only_declared_reactor_modules() {
        let directory = tempfile::tempdir().expect("temporary project root exists");
        let root = directory.path().canonicalize().expect("root canonicalizes");
        write_pom(&root, "<modules><module>orders</module></modules>");
        write_pom(&root.join("orders"), "");
        write_pom(&root.join("unlisted"), "");

        let catalog = ModuleCatalog::load(&root);

        assert_eq!(
            catalog.owner_for("orders/src/main/java/example/Order.java"),
            root.join("orders")
        );
        assert_eq!(
            catalog.descendants_of(&root),
            vec![root.clone(), root.join("orders")]
        );
        assert_eq!(
            catalog.owner_for("unlisted/src/main/java/example/Hidden.java"),
            root
        );
        assert!(catalog.diagnostic().is_none());
    }

    #[test]
    fn maven_catalog_retains_unconditional_modules_when_profiles_are_present() {
        let directory = tempfile::tempdir().expect("temporary project root exists");
        let root = directory.path().canonicalize().expect("root canonicalizes");
        write_pom(
            &root,
            "<modules><module>orders</module></modules><profiles><profile><modules><module>experimental</module></modules></profile></profiles><build><plugins><plugin><modules><module>ignored</module></modules></plugin></plugins></build>",
        );
        write_pom(
            &root.join("orders"),
            "<modules><module>billing</module></modules><profiles><profile><modules><module>orders-experimental</module></modules></profile></profiles>",
        );
        write_pom(&root.join("orders/billing"), "");
        write_pom(&root.join("orders/orders-experimental"), "");
        write_pom(&root.join("experimental"), "");
        write_pom(&root.join("ignored"), "");

        let catalog = ModuleCatalog::load(&root);

        assert_eq!(
            catalog.owner_for("orders/billing/src/main/java/example/Billing.java"),
            root.join("orders/billing")
        );
        assert_eq!(
            catalog.owner_for("experimental/src/main/java/example/Experiment.java"),
            root
        );
        assert!(
            catalog
                .diagnostic()
                .is_some_and(|diagnostic| diagnostic.contains("profile"))
        );
        assert!(!catalog.is_complete());
    }

    #[test]
    fn maven_catalog_rejects_property_and_escaping_module_paths() {
        for module in ["${child}", "../outside"] {
            let directory = tempfile::tempdir().expect("temporary project root exists");
            let root = directory.path().canonicalize().expect("root canonicalizes");
            write_pom(
                &root,
                &format!("<modules><module>{module}</module></modules>"),
            );

            let catalog = ModuleCatalog::load(&root);

            assert!(catalog.diagnostic().is_some(), "{module}");
            assert_eq!(
                catalog.owner_for("orders/src/main/java/example/Order.java"),
                root
            );
        }
    }

    #[test]
    fn maven_catalog_reads_cdata_module_paths() {
        let directory = tempfile::tempdir().expect("temporary project root exists");
        let root = directory.path().canonicalize().expect("root canonicalizes");
        write_pom(
            &root,
            "<modules><module><![CDATA[orders]]></module></modules>",
        );
        write_pom(&root.join("orders"), "");
        assert_eq!(
            ModuleCatalog::load(&root).owner_for("orders/src/main/java/Order.java"),
            root.join("orders")
        );
    }

    #[test]
    fn maven_catalog_ignores_nested_plugin_modules() {
        let directory = tempfile::tempdir().expect("temporary project root exists");
        let root = directory.path().canonicalize().expect("root canonicalizes");
        write_pom(
            &root,
            "<build><plugins><plugin><modules><module>unlisted</module></modules></plugin></plugins></build>",
        );
        write_pom(&root.join("unlisted"), "");
        let catalog = ModuleCatalog::load(&root);
        assert_eq!(catalog.descendants_of(&root), vec![root]);
    }

    #[test]
    fn gradle_catalog_reads_literal_groovy_and_kotlin_projects() {
        let directory = tempfile::tempdir().expect("temporary project root exists");
        let root = directory.path().canonicalize().expect("root canonicalizes");
        write_gradle_project(&root.join("services/orders"));
        fs::write(root.join("settings.gradle"), "include ':services:orders'")
            .expect("settings writes");
        assert_eq!(
            ModuleCatalog::load(&root).owner_for("services/orders/src/main/java/Order.java"),
            root.join("services/orders")
        );

        let directory = tempfile::tempdir().expect("temporary project root exists");
        let root = directory.path().canonicalize().expect("root canonicalizes");
        write_gradle_project(&root.join("custom/payment"));
        fs::write(
            root.join("settings.gradle.kts"),
            "include(\":payment\")\nproject(\":payment\").projectDir = file(\"custom/payment\")",
        )
        .expect("settings writes");
        assert_eq!(
            ModuleCatalog::load(&root).owner_for("custom/payment/src/main/java/Payment.java"),
            root.join("custom/payment")
        );
    }

    #[test]
    fn gradle_catalog_reads_literal_root_dir_project_mappings() {
        let directory = tempfile::tempdir().expect("temporary project root exists");
        let root = directory.path().canonicalize().expect("root canonicalizes");
        write_gradle_project(&root.join("xds"));
        fs::write(
            root.join("settings.gradle"),
            "include ':grpc-xds'\nproject(':grpc-xds').projectDir = \"$rootDir/xds\" as File",
        )
        .expect("settings writes");

        assert_eq!(
            ModuleCatalog::load(&root).owner_for("xds/src/main/java/Xds.java"),
            root.join("xds")
        );
    }

    #[test]
    fn gradle_catalog_accepts_a_lexically_aliased_project_root() {
        let directory = tempfile::tempdir().expect("temporary project root exists");
        let root = directory.path().canonicalize().expect("root canonicalizes");
        fs::create_dir(root.join("alias")).expect("alias segment exists");
        write_gradle_project(&root.join("xds"));
        fs::write(
            root.join("settings.gradle"),
            "include ':grpc-xds'\nproject(':grpc-xds').projectDir = \"$rootDir/xds\" as File",
        )
        .expect("settings writes");

        assert_eq!(
            ModuleCatalog::load(&root.join("alias").join(".."))
                .owner_for("xds/src/main/java/Xds.java"),
            root.join("xds")
        );
    }

    #[test]
    fn gradle_catalog_rejects_dynamic_project_dir_mappings() {
        let directory = tempfile::tempdir().expect("temporary project root exists");
        let root = directory.path().canonicalize().expect("root canonicalizes");
        write_gradle_project(&root.join("$moduleDirectory"));
        fs::write(
            root.join("settings.gradle"),
            r#"include ':grpc-xds'
project(':grpc-xds').projectDir = "$rootDir/$moduleDirectory" as File"#,
        )
        .expect("settings writes");

        let catalog = ModuleCatalog::load(&root);
        assert!(catalog.diagnostic().is_some());
        assert_eq!(
            catalog.owner_for("$moduleDirectory/src/main/java/Xds.java"),
            root
        );
    }

    #[test]
    fn gradle_catalog_rejects_multiline_dynamic_project_dir_mappings() {
        let directory = tempfile::tempdir().expect("temporary project root exists");
        let root = directory.path().canonicalize().expect("root canonicalizes");
        write_gradle_project(&root.join("grpc-xds"));
        fs::write(
            root.join("settings.gradle"),
            "include ':grpc-xds'\nproject(':grpc-xds').projectDir =\n    dynamicPath",
        )
        .expect("settings writes");

        let catalog = ModuleCatalog::load(&root);
        assert!(catalog.diagnostic().is_some());
        assert_eq!(catalog.owner_for("grpc-xds/src/main/java/Xds.java"), root);
    }

    #[test]
    fn gradle_catalog_rejects_root_dir_mappings_that_escape_the_project() {
        let directory = tempfile::tempdir().expect("temporary project root exists");
        let root = directory.path().canonicalize().expect("root canonicalizes");
        let project = root.join("project");
        write_gradle_project(&project);
        write_gradle_project(&root.join("outside"));
        fs::write(
            project.join("settings.gradle"),
            "include ':outside'\nproject(':outside').projectDir = \"$rootDir/../outside\" as File",
        )
        .expect("settings writes");

        let catalog = ModuleCatalog::load(&project);
        assert!(catalog.diagnostic().is_some());
        assert_eq!(catalog.owner_for("src/main/java/Project.java"), project);
    }

    #[test]
    fn gradle_catalog_rejects_dynamic_includes() {
        let directory = tempfile::tempdir().expect("temporary project root exists");
        let root = directory.path().canonicalize().expect("root canonicalizes");
        fs::write(root.join("settings.gradle"), "include projectName").expect("settings writes");
        let catalog = ModuleCatalog::load(&root);
        assert!(catalog.diagnostic().is_some());
        assert_eq!(catalog.owner_for("orders/src/main/java/Order.java"), root);
    }

    fn write_pom(directory: &Path, body: &str) {
        fs::create_dir_all(directory).expect("module directory exists");
        fs::write(
            directory.join("pom.xml"),
            format!("<project><modelVersion>4.0.0</modelVersion>{body}</project>"),
        )
        .expect("POM writes");
    }

    fn write_gradle_project(directory: &Path) {
        fs::create_dir_all(directory).expect("module directory exists");
        fs::write(directory.join("build.gradle"), "plugins { id 'java' }")
            .expect("build file writes");
    }
}
