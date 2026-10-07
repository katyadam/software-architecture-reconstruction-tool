use quick_xml::{Reader, events::Event};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

/// Records declared Java build modules or a conservative reason that metadata is unusable.
pub struct ModuleCatalog {
    root: PathBuf,
    state: CatalogState,
}

enum CatalogState {
    Complete(Vec<PathBuf>),
    Uncertain(String),
}

impl ModuleCatalog {
    /// Loads Java build metadata; parsing is added by the Maven and Gradle catalog stages.
    pub fn load(project_root: &Path) -> Self {
        let root = normalized(project_root);
        if root.join("pom.xml").is_file() {
            return match maven_modules(&root) {
                Ok(modules) => Self::complete(root, modules),
                Err(diagnostic) => Self::uncertain(root, diagnostic),
            };
        }
        Self::uncertain(root, "no Maven or Gradle build metadata found".into())
    }

    /// Returns the deepest declared module containing a candidate-relative source path.
    pub fn owner_for(&self, source_path: &str) -> PathBuf {
        match &self.state {
            CatalogState::Complete(modules) => {
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
            CatalogState::Complete(modules) => modules
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
            CatalogState::Uncertain(diagnostic) => Some(diagnostic),
        }
    }

    pub(super) fn complete(root: PathBuf, modules: Vec<PathBuf>) -> Self {
        Self {
            root,
            state: CatalogState::Complete(modules),
        }
    }

    pub(super) fn uncertain(root: PathBuf, diagnostic: String) -> Self {
        Self {
            root,
            state: CatalogState::Uncertain(diagnostic),
        }
    }
}

/// Normalizes existing roots so path-prefix ownership checks are stable.
fn normalized(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
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
fn maven_modules(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut modules = vec![root.to_path_buf()];
    collect_maven_modules(root, root, &mut modules)?;
    Ok(modules)
}

fn collect_maven_modules(
    root: &Path,
    module: &Path,
    modules: &mut Vec<PathBuf>,
) -> Result<(), String> {
    for declared in maven_declared_modules(&module.join("pom.xml"))? {
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
        if modules.contains(&child) {
            return Err(format!(
                "Maven module is declared more than once: {declared}"
            ));
        }
        modules.push(child.clone());
        collect_maven_modules(root, &child, modules)?;
    }
    Ok(())
}

fn maven_declared_modules(pom: &Path) -> Result<Vec<String>, String> {
    let xml = fs::read_to_string(pom).map_err(|error| error.to_string())?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut elements: Vec<String> = Vec::new();
    let mut module_value: Option<String> = None;
    let mut modules = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                let name = String::from_utf8_lossy(element.name().as_ref()).into_owned();
                if name == "profile" {
                    return Err("Maven profiles are unsupported for module metadata".into());
                }
                if name == "module" && elements.as_slice() == ["project", "modules"] {
                    module_value = Some(String::new());
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
            Ok(Event::Eof) => return Ok(modules),
            Err(error) => return Err(format!("unable to parse Maven POM: {error}")),
            _ => {}
        }
    }
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

    fn write_pom(directory: &Path, body: &str) {
        fs::create_dir_all(directory).expect("module directory exists");
        fs::write(
            directory.join("pom.xml"),
            format!("<project><modelVersion>4.0.0</modelVersion>{body}</project>"),
        )
        .expect("POM writes");
    }
}
