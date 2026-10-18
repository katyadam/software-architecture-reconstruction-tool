use super::*;
use models::{go_impact::*, go_package_impact::*};
use std::collections::BTreeMap;

/// Only the validator can construct authority to apply package exclusions.
#[derive(Debug)]
pub struct ValidatedGoPackageInventory {
    evidence: GoPackageInventoryOutcome,
    inventory: GoInventoryOutcome,
    owners: BTreeMap<String, BTreeSet<String>>,
}
impl ValidatedGoPackageInventory {
    pub(crate) fn matches_inventory(&self, inventory: &GoInventoryOutcome) -> bool {
        &self.inventory == inventory
    }
    pub fn evidence(&self) -> &GoPackageInventoryOutcome {
        &self.evidence
    }
    pub fn owners(&self) -> &BTreeMap<String, BTreeSet<String>> {
        &self.owners
    }
}

impl GoNativeProvider {
    pub fn discover_packages(&self, root: &Path) -> GoPackageInventoryOutcome {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let failed = |message: String| GoPackageInventoryOutcome {
            schema_version: 1,
            provider_id: PROVIDER_ID.into(),
            status: GoStageStatus::Failed,
            source_root: root.to_string_lossy().into(),
            scope: None,
            effective_build_context: None,
            complete: false,
            packages: vec![],
            execution_packages: vec![],
            blockers: vec![],
            diagnostics: vec![message],
        };
        let output = match self.metadata("package-inventory", Some(&root), None) {
            Ok(o) => o,
            Err(e) => return failed(format!("Go package inventory: {e}")),
        };
        let parsed = serde_json::from_slice::<GoPackageInventoryOutcome>(&output.stdout)
            .map_err(|e| e.to_string())
            .and_then(|out| {
                if out.schema_version != 1
                    || out.provider_id != PROVIDER_ID
                    || out.source_root != root.to_string_lossy()
                    || !output.status.success()
                    || out.status != GoStageStatus::Ok
                    || !out.complete
                {
                    return Err(format!(
                        "unsuccessful/incomplete package capability: {:?}",
                        out.diagnostics
                    ));
                }
                let scope = out.scope.as_ref().ok_or("missing package scope")?;
                if scope.build_tags != self.build_tags {
                    return Err("package build tags differ from request".into());
                }
                Ok(out)
            });
        parsed.unwrap_or_else(|e| {
            failed(format!(
                "Go package inventory protocol: {e}; stderr: {}",
                diagnostic_tail(&String::from_utf8_lossy(&output.stderr))
            ))
        })
    }
}

fn exact_directory(root: &Path, relative: &str) -> Result<PathBuf, String> {
    if relative.is_empty()
        || relative.contains('\\')
        || relative.contains("...")
        || relative.starts_with('/')
        || relative != "."
            && relative
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err("unrepresentable exact execution directory".into());
    }
    let path = root.join(relative);
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("execution directory: {e}"))?;
    if !canonical.is_dir() || !canonical.starts_with(root) || canonical != path && relative != "." {
        return Err("foreign or aliased execution directory".into());
    }
    Ok(canonical)
}

fn normalized_tags(tags: &str) -> String {
    tags.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(",")
}

fn validate_context(scope: &GoAnalysisScope, c: &GoEffectiveBuildContext) -> Result<(), String> {
    if scope.workspace_mode != "off"
        || scope.go_version.is_empty()
        || c.go_version != scope.go_version
        || c.goos != scope.goos
        || c.goarch != scope.goarch
        || c.build_tags != normalized_tags(&scope.build_tags)
        || !matches!(c.cgo_enabled.as_str(), "0" | "1")
    {
        return Err("effective driver/build scope mismatch".into());
    }
    for (key, value) in [
        ("GOENV", "off"),
        ("GOWORK", "off"),
        ("GOTOOLCHAIN", "local"),
        ("GOPROXY", "off"),
        ("GOSUMDB", "off"),
        ("GOFLAGS", ""),
        ("GOOS", c.goos.as_str()),
        ("GOARCH", c.goarch.as_str()),
        ("CGO_ENABLED", c.cgo_enabled.as_str()),
        ("GOEXPERIMENT", c.goexperiment.as_str()),
    ] {
        if c.environment.get(key).map(String::as_str) != Some(value) {
            return Err(format!("effective execution environment mismatch: {key}"));
        }
    }
    let required_feature = match c.goarch.as_str() {
        "386" => Some("GO386"),
        "amd64" => Some("GOAMD64"),
        "arm" => Some("GOARM"),
        "arm64" => Some("GOARM64"),
        "mips" | "mipsle" => Some("GOMIPS"),
        "mips64" | "mips64le" => Some("GOMIPS64"),
        "ppc64" | "ppc64le" => Some("GOPPC64"),
        "riscv64" => Some("GORISCV64"),
        _ => None,
    };
    if required_feature.is_some_and(|key| !c.architecture_features.contains_key(key)) {
        return Err("missing effective architecture feature setting".into());
    }
    for (key, value) in &c.architecture_features {
        if c.environment.get(key) != Some(value)
            || value.is_empty()
            || ![
                "GO386",
                "GOAMD64",
                "GOARM",
                "GOARM64",
                "GOMIPS",
                "GOMIPS64",
                "GOPPC64",
                "GORISCV64",
                "GOWASM",
            ]
            .contains(&key.as_str())
        {
            return Err("architecture feature context mismatch".into());
        }
    }
    let mut flags = vec!["-mod=readonly".to_string()];
    if !c.build_tags.is_empty() {
        flags.push(format!("-tags={}", c.build_tags));
    }
    if flags != c.build_flags {
        return Err("unmodeled execution build flags".into());
    }
    Ok(())
}

pub fn validate_package_inventory(
    root: &Path,
    inventory: &GoInventoryOutcome,
    evidence: &GoPackageInventoryOutcome,
) -> Result<ValidatedGoPackageInventory, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    if evidence.schema_version != 1
        || evidence.provider_id != PROVIDER_ID
        || evidence.source_root != root.to_string_lossy()
        || inventory.source_root != evidence.source_root
        || inventory.status != GoStageStatus::Ok
        || !inventory.test_inventory_complete
        || evidence.status != GoStageStatus::Ok
        || !evidence.complete
        || !evidence.blockers.is_empty()
    {
        return Err("package capability unavailable, incomplete, blocked or inconsistent".into());
    }
    let scope = inventory
        .scope
        .as_ref()
        .ok_or("missing ordinary inventory scope")?;
    if evidence.scope.as_ref() != Some(scope) {
        return Err("package/ordinary inventory scope drift".into());
    }
    validate_context(
        scope,
        evidence
            .effective_build_context
            .as_ref()
            .ok_or("missing effective driver context")?,
    )?;
    let mut nodes = BTreeMap::new();
    let mut owners: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for node in &evidence.packages {
        if node.id.is_empty()
            || node.import_path.is_empty()
            || nodes.insert(node.id.as_str(), node).is_some()
        {
            return Err("missing/duplicate package identity".into());
        }
        let directory = Path::new(&node.provenance.directory);
        if !directory.is_absolute() || !directory.is_dir() {
            return Err("missing package provenance directory".into());
        }
        let local = matches!(
            node.provenance.kind,
            GoPackageProvenanceKind::VerifiedModule | GoPackageProvenanceKind::LocalReplacement
        );
        if local
            && (directory
                .canonicalize()
                .ok()
                .is_none_or(|d| !d.starts_with(&root))
                || node
                    .provenance
                    .module_path
                    .as_ref()
                    .is_none_or(String::is_empty))
        {
            return Err("foreign/missing local module provenance".into());
        }
        if !local && (!node.source_files.is_empty() || !node.test_files.is_empty()) {
            return Err("nonlocal provenance claims committed local source".into());
        }
        let mut seen = BTreeSet::new();
        for file in &node.source_files {
            let path = Path::new(file);
            let canonical = path
                .canonicalize()
                .map_err(|e| format!("local source {file}: {e}"))?;
            if !local
                || !path.is_absolute()
                || canonical != path
                || !path.is_file()
                || !canonical.starts_with(&root)
                || canonical.parent() != Some(directory)
                || !seen.insert(file)
            {
                return Err("foreign, aliased, duplicate or contradictory source ownership".into());
            }
            owners
                .entry(file.clone())
                .or_default()
                .insert(node.id.clone());
        }
        let expected: BTreeSet<_> = node
            .source_files
            .iter()
            .filter(|f| f.ends_with("_test.go"))
            .collect();
        let actual: BTreeSet<_> = node.test_files.iter().collect();
        if actual != expected
            || actual.len() != node.test_files.len()
            || !actual.is_empty() && node.for_test.is_empty()
        {
            return Err("test-file ownership contradicts package instance".into());
        }
    }
    for node in &evidence.packages {
        let mut seen = BTreeSet::new();
        if node
            .imports
            .iter()
            .any(|id| !nodes.contains_key(id.as_str()) || !seen.insert(id))
        {
            return Err("dangling/duplicate package import".into());
        }
    }
    let ordinary: BTreeSet<_> = inventory.source_files.iter().collect();
    let active_go: BTreeSet<_> = owners.keys().filter(|f| f.ends_with(".go")).collect();
    if ordinary != active_go {
        return Err("active ordinary/package source set drift".into());
    }
    let mut binaries = BTreeMap::new();
    let mut dirs = BTreeSet::new();
    let mut rooted_tests = BTreeSet::new();
    for binary in &evidence.execution_packages {
        let directory = exact_directory(&root, &binary.relative_directory)?;
        if binary.import_path.is_empty()
            || binary.roots.is_empty()
            || binaries
                .insert(binary.import_path.as_str(), binary)
                .is_some()
            || !dirs.insert(directory.clone())
        {
            return Err("missing/duplicate execution package".into());
        }
        let mut roots = BTreeSet::new();
        let mut test_root = false;
        for id in &binary.roots {
            let node = nodes
                .get(id.as_str())
                .ok_or("execution root not in package graph")?;
            if !roots.insert(id)
                || node.source_files.is_empty()
                || node.provenance.directory != directory.to_string_lossy()
                || node.for_test != binary.import_path
                    && !(node.for_test.is_empty()
                        && node.import_path == binary.import_path
                        && node.test_files.is_empty())
            {
                return Err("execution root has inconsistent source grouping".into());
            }
            if !node.test_files.is_empty() {
                test_root = true;
                rooted_tests.insert(node.id.as_str());
            }
        }
        if !test_root {
            return Err("binary lacks source-backed active tests".into());
        }
        // The generated harness initializes the package under test even when
        // its external tests never import it. Prefer its augmented instance;
        // a test-only directory has no source-backed production instance.
        let production: Vec<_> = evidence
            .packages
            .iter()
            .filter(|node| {
                node.import_path == binary.import_path
                    && node.provenance.directory == directory.to_string_lossy()
                    && !node.source_files.is_empty()
                    && (node.for_test.is_empty() || node.for_test == binary.import_path)
            })
            .collect();
        let augmented = production
            .iter()
            .any(|node| node.for_test == binary.import_path);
        if production.iter().any(|node| {
            (!augmented || node.for_test == binary.import_path) && !binary.roots.contains(&node.id)
        }) {
            return Err("source-backed production initialization root omitted".into());
        }
    }
    for node in &evidence.packages {
        if !node.test_files.is_empty() && !rooted_tests.contains(node.id.as_str()) {
            return Err("active test-file group omitted from execution inventory".into());
        }
    }
    for test in &inventory.tests {
        let binary = binaries
            .get(test.execution_package.as_str())
            .ok_or("ordinary TestX has no execution binary")?;
        let file = test
            .method
            .source_path
            .as_ref()
            .ok_or("TestX has no source")?;
        let owner = owners.get(file).ok_or("TestX has no package owner")?;
        let matched: Vec<_> = owner
            .iter()
            .filter(|id| nodes[id.as_str()].import_path == test.method.declaring_type)
            .collect();
        if matched.is_empty() || matched.iter().any(|id| !binary.roots.contains(id)) {
            return Err("TestX source-owning instance omitted from binary roots".into());
        }
    }
    Ok(ValidatedGoPackageInventory {
        evidence: evidence.clone(),
        inventory: inventory.clone(),
        owners,
    })
}
