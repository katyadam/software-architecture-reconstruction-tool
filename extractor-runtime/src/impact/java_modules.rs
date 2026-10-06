use std::path::{Component, Path, PathBuf};

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
        Self::uncertain(
            normalized(project_root),
            "module metadata has not been interpreted".into(),
        )
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

#[cfg(test)]
mod tests {
    use super::ModuleCatalog;
    use std::fs;

    #[test]
    fn falls_back_to_the_project_root_without_declared_metadata() {
        let directory = tempfile::tempdir().expect("temporary project root exists");
        let root = directory.path().canonicalize().expect("root canonicalizes");
        fs::write(root.join("pom.xml"), "<project/>").expect("POM writes");

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
}
