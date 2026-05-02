//! Recursive project-root walker that classifies files by [`SourceKind`].
//!
//! Skipped directories: `.git`, `node_modules`, `target`, `__pycache__`.

use std::path::{Path, PathBuf};

use log::{debug, warn};
use walkdir::WalkDir;

use crate::parsers::SourceKind;

/// Default maximum directory depth for [`discover`].
pub const DEFAULT_MAX_DEPTH: usize = 20;

/// A file discovered during the walk, together with its classification.
#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub path: PathBuf,
    pub kind: SourceKind,
}

/// Walk `root` recursively (up to `max_depth` levels) and return all files
/// that match the requested `sources` filter.
///
/// Pass an empty `sources` slice to discover all supported kinds.
pub fn discover(root: &Path, sources: &[SourceKind], max_depth: usize) -> Vec<DiscoveredFile> {
    let mut found = Vec::new();

    let walker = WalkDir::new(root)
        .max_depth(max_depth)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path()));

    for entry in walker.filter_map(|e| match e {
        Ok(entry) => Some(entry),
        Err(e) => {
            warn!("walkdir error: {e}");
            None
        }
    }) {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        if let Some(kind) = classify(path)
            && (sources.is_empty() || sources.contains(&kind))
        {
            debug!("discovery: found {:?} {:?}", kind, path);
            found.push(DiscoveredFile {
                path: path.to_owned(),
                kind,
            });
        }
    }

    found
}

/// Returns `true` for directories (and their children) that should be skipped.
fn is_ignored(path: &Path) -> bool {
    path.components().any(|c| {
        matches!(
            c.as_os_str().to_str().unwrap_or(""),
            ".git" | "node_modules" | "target" | "__pycache__"
        )
    })
}

/// Attempt to classify a file path as a supported [`SourceKind`].
/// Returns `None` if the file is not recognised.
pub fn classify(path: &Path) -> Option<SourceKind> {
    let file_name = path.file_name()?.to_str()?;

    if is_docker_compose(file_name) {
        return Some(SourceKind::DockerCompose);
    }

    if is_dotenv(file_name) {
        return Some(SourceKind::DotEnv);
    }

    None
}

fn is_docker_compose(name: &str) -> bool {
    let lower = name.to_lowercase();
    (lower.starts_with("docker-compose") || lower.starts_with("compose"))
        && (lower.ends_with(".yml") || lower.ends_with(".yaml"))
}

fn is_dotenv(name: &str) -> bool {
    if name == ".env" || name.starts_with(".env.") {
        return true;
    }
    if name.ends_with(".env") {
        return true;
    }
    false
}

/// Compute the merge priority for a discovered dotenv file.
/// Returns the priority value matching the plan's conflict-resolution table.
pub fn dotenv_priority(path: &Path) -> u8 {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    match name {
        ".env" => 100,
        ".env.local" => 95,
        n if n.starts_with(".env.") => 90, // .env.production, .env.development, etc.
        _ => 60,                           // sample.env, development.env, production.env, *.env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_dotenv_exact() {
        assert_eq!(classify(Path::new("/a/.env")), Some(SourceKind::DotEnv));
    }

    #[test]
    fn classify_dotenv_qualified() {
        assert_eq!(
            classify(Path::new("/a/.env.production")),
            Some(SourceKind::DotEnv)
        );
    }

    #[test]
    fn classify_named_env() {
        assert_eq!(
            classify(Path::new("/a/sample.env")),
            Some(SourceKind::DotEnv)
        );
    }

    #[test]
    fn classify_docker_compose_yml() {
        assert_eq!(
            classify(Path::new("/a/docker-compose.yml")),
            Some(SourceKind::DockerCompose)
        );
    }

    #[test]
    fn classify_docker_compose_yaml() {
        assert_eq!(
            classify(Path::new("/a/docker-compose.override.yaml")),
            Some(SourceKind::DockerCompose)
        );
    }

    #[test]
    fn classify_compose_short() {
        assert_eq!(
            classify(Path::new("/a/compose.yml")),
            Some(SourceKind::DockerCompose)
        );
    }

    #[test]
    fn classify_rs_file_none() {
        assert_eq!(classify(Path::new("/a/main.rs")), None);
    }

    #[test]
    fn priority_env() {
        assert_eq!(dotenv_priority(Path::new("/a/.env")), 100);
    }

    #[test]
    fn priority_env_local() {
        assert_eq!(dotenv_priority(Path::new("/a/.env.local")), 95);
    }

    #[test]
    fn priority_env_production() {
        assert_eq!(dotenv_priority(Path::new("/a/.env.production")), 90);
    }

    #[test]
    fn priority_sample_env() {
        assert_eq!(dotenv_priority(Path::new("/a/sample.env")), 60);
    }

    #[test]
    fn ignored_git_dir() {
        assert!(is_ignored(Path::new("/project/.git/config")));
    }

    #[test]
    fn ignored_node_modules() {
        assert!(is_ignored(Path::new("/project/node_modules/pkg/.env")));
    }

    #[test]
    fn not_ignored_normal_path() {
        assert!(!is_ignored(Path::new("/project/services/api/.env")));
    }

    // -- additional edge cases --

    #[test]
    fn test_classify_dotenv_local() {
        // `.env.local` must classify as DotEnv
        assert_eq!(
            classify(Path::new("/svc/.env.local")),
            Some(SourceKind::DotEnv)
        );
    }

    #[test]
    fn test_classify_docker_compose_yaml_extension() {
        // `docker-compose.yaml` (not .yml) must classify
        assert_eq!(
            classify(Path::new("/svc/docker-compose.yaml")),
            Some(SourceKind::DockerCompose)
        );
    }

    #[test]
    fn test_classify_compose_yaml() {
        // `compose.yaml` short-form must classify
        assert_eq!(
            classify(Path::new("/svc/compose.yaml")),
            Some(SourceKind::DockerCompose)
        );
    }

    #[test]
    fn test_classify_compose_yml() {
        // `compose.yml` short-form must classify
        assert_eq!(
            classify(Path::new("/svc/compose.yml")),
            Some(SourceKind::DockerCompose)
        );
    }

    #[test]
    fn test_is_ignored_target_dir() {
        assert!(is_ignored(Path::new("/project/target/debug/build/.env")));
    }

    #[test]
    fn test_is_ignored_pycache_dir() {
        assert!(is_ignored(Path::new("/project/svc/__pycache__/module.pyc")));
    }

    #[test]
    fn test_is_ignored_git_nested() {
        assert!(is_ignored(Path::new("/project/sub/.git/hooks/pre-commit")));
    }

    #[test]
    fn test_is_ignored_node_modules_nested() {
        assert!(is_ignored(Path::new("/project/frontend/node_modules/.env")));
    }

    #[test]
    fn test_priority_env_local_is_95() {
        assert_eq!(dotenv_priority(Path::new("/svc/.env.local")), 95);
    }

    #[test]
    fn test_priority_env_development_is_90() {
        assert_eq!(dotenv_priority(Path::new("/svc/.env.development")), 90);
    }

    #[test]
    fn test_priority_named_dot_env_file_is_60() {
        // `production.env` is a named env file -> priority 60
        assert_eq!(dotenv_priority(Path::new("/svc/production.env")), 60);
    }

    #[test]
    fn test_discover_ignores_target_dir() {
        use std::fs;
        use tempfile::TempDir;

        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();

        // Create a real file inside `target/` — it must not appear in results.
        let target_dir = root.join("target/debug");
        fs::create_dir_all(&target_dir).unwrap();
        fs::write(target_dir.join(".env"), "SECRET=leaked").unwrap();

        // Create a real file outside ignored dirs — it must appear.
        let svc_dir = root.join("svc");
        fs::create_dir_all(&svc_dir).unwrap();
        fs::write(svc_dir.join("sample.env"), "KEY=val").unwrap();

        let found = discover(root, &[], DEFAULT_MAX_DEPTH);
        let paths: Vec<_> = found
            .iter()
            .map(|f| f.path.to_str().unwrap().to_owned())
            .collect();

        assert!(
            paths.iter().any(|p| p.contains("sample.env")),
            "sample.env must be discovered"
        );
        assert!(
            !paths.iter().any(|p| p.contains("target")),
            "files inside target/ must not be discovered"
        );
    }

    #[test]
    fn test_discover_source_filter_docker_compose_only() {
        use std::fs;
        use tempfile::TempDir;

        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        fs::write(root.join("sample.env"), "K=v").unwrap();
        fs::write(root.join("docker-compose.yml"), "services: {}").unwrap();

        let found = discover(root, &[SourceKind::DockerCompose], DEFAULT_MAX_DEPTH);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, SourceKind::DockerCompose);
    }
}
