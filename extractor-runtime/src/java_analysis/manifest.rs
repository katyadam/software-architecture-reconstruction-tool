use super::*;
use models::{configuration::ServiceDescription, java_analysis::JavaArtifactManifest};

pub(super) fn directory(path: &Path, scope: Option<&Path>) -> Result<PathBuf, JavaInputsError> {
    let path = canonical(path, scope)?;
    if !path.is_dir() {
        return Err(JavaInputsError::Path(format!(
            "not a directory: {}",
            path.display()
        )));
    }
    Ok(path)
}

pub(super) fn file(path: &Path, scope: Option<&Path>) -> Result<PathBuf, JavaInputsError> {
    let path = canonical(path, scope)?;
    if !path.is_file() {
        return Err(JavaInputsError::Path(format!(
            "not a file: {}",
            path.display()
        )));
    }
    Ok(path)
}

fn canonical(path: &Path, scope: Option<&Path>) -> Result<PathBuf, JavaInputsError> {
    let canonical = path
        .canonicalize()
        .map_err(|e| JavaInputsError::Path(format!("{}: {e}", path.display())))?;
    if scope.is_some_and(|scope| !canonical.starts_with(scope)) {
        return Err(JavaInputsError::Path(format!(
            "path escapes scope: {}",
            path.display()
        )));
    }
    if canonical.to_str().is_none() {
        return Err(JavaInputsError::Path(
            "non-UTF8 paths are unsupported".into(),
        ));
    }
    Ok(canonical)
}

fn directories(
    root: &Path,
    paths: &[String],
    checkout: &Path,
) -> Result<Vec<PathBuf>, JavaInputsError> {
    let mut result = paths
        .iter()
        .map(|p| directory(&root.join(p), Some(checkout)))
        .collect::<Result<Vec<_>, _>>()?;
    result.sort();
    result.dedup();
    Ok(result)
}

fn profiles(values: &[String]) -> Result<BTreeSet<&str>, JavaInputsError> {
    if values.iter().any(|p| {
        p.is_empty() || p.trim() != p || p.starts_with('-') || p.contains([',', '\n', '\r'])
    }) {
        return Err(JavaInputsError::Configuration(
            "expected profile IDs, not Maven command text".into(),
        ));
    }
    Ok(values.iter().map(String::as_str).collect())
}

pub(super) fn load(
    checkout: &Path,
    service_root: &Path,
    service: &ServiceDescription,
    path: &Path,
    revision: &str,
    inputs: &mut ConfiguredJavaInputs,
) -> Result<(), JavaInputsError> {
    let raw =
        std::fs::read_to_string(path).map_err(|e| JavaInputsError::Manifest(e.to_string()))?;
    let manifest: JavaArtifactManifest =
        serde_json::from_str(&raw).map_err(|e| JavaInputsError::Manifest(e.to_string()))?;
    if manifest.schema_version != 1
        || manifest.candidate_revision != revision
        || revision.len() != 40
        || !revision.bytes().all(|c| c.is_ascii_hexdigit())
    {
        return Err(JavaInputsError::Manifest(
            "wrong schema version or candidate revision".into(),
        ));
    }
    let settings = service
        .java_analysis
        .as_ref()
        .expect("manifest needs settings");
    let mut context = manifest.build_context;
    if !matches!(context.tool.as_str(), "maven" | "gradle")
        || context.java_version.trim().is_empty()
    {
        return Err(JavaInputsError::Manifest(
            "missing/unsupported build tool or Java version".into(),
        ));
    }
    if profiles(&settings.maven_profiles)? != profiles(&context.requested_profiles)?
        || (context.tool != "maven" && !settings.maven_profiles.is_empty())
    {
        return Err(JavaInputsError::Manifest(
            "requested profile mismatch".into(),
        ));
    }
    let active = profiles(&context.active_profiles)?;
    if !profiles(&context.requested_profiles)?.is_subset(&active) {
        return Err(JavaInputsError::Manifest(
            "requested profiles are not active in the supplied build".into(),
        ));
    }
    context.requested_profiles.sort();
    context.requested_profiles.dedup();
    context.active_profiles.sort();
    context.active_profiles.dedup();
    let mut roots = Vec::new();
    let mut sources = Vec::new();
    let mut tests = Vec::new();
    let mut local_roots = BTreeSet::new();
    inputs.module_inventory_complete &= manifest.module_inventory_complete;
    for module in manifest.modules {
        let root = directory(&service_root.join(&module.root), Some(checkout))?;
        if !local_roots.insert(root.clone()) {
            return Err(JavaInputsError::Manifest(format!(
                "duplicate module: {}",
                root.display()
            )));
        }
        let source_roots = directories(&root, &module.source_roots, checkout)?;
        let test_source_roots = directories(&root, &module.test_source_roots, checkout)?;
        let mut classpath = module
            .classpath
            .iter()
            .map(|p| canonical(&root.join(p), None))
            .collect::<Result<Vec<_>, _>>()?;
        if classpath.iter().any(|p| !p.is_dir() && !p.is_file()) {
            return Err(JavaInputsError::Manifest("invalid classpath entry".into()));
        }
        classpath.sort();
        classpath.dedup();
        let bytecode = JavaBytecodeInputs {
            application_class_dirs: directories(&root, &module.application_class_dirs, checkout)?,
            test_class_dirs: directories(&root, &module.test_class_dirs, checkout)?,
            classpath,
        };
        let dependencies = module
            .dependencies
            .iter()
            .map(|p| directory(&service_root.join(p), Some(checkout)))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let mut discovered = Vec::new();
        let mut identities = BTreeSet::new();
        for mut test in module.tests {
            let class = if test.class_name.contains('.') {
                format!("L{}", test.class_name.replace('.', "/"))
            } else if test.class_name.starts_with('L') {
                test.class_name.clone()
            } else {
                format!("L{}", test.class_name)
            };
            if !class
                .strip_prefix('L')
                .is_some_and(|c| c.split('/').all(identifier))
            {
                return Err(JavaInputsError::Manifest(
                    "invalid test class identity".into(),
                ));
            }
            test.class_name = class;
            let owner = directory(&root.join(&test.module_root), Some(checkout))?;
            let source = file(&root.join(&test.source_path), Some(checkout))?;
            if owner != root
                || !test_source_roots.iter().any(|p| source.starts_with(p))
                || !source.extension().is_some_and(|e| e == "java")
                || !identifier(&test.method_name)
                || !descriptor(&test.descriptor)
                || test.callable_signature.trim().is_empty()
                || !identities.insert((
                    test.class_name.clone(),
                    test.method_name.clone(),
                    test.descriptor.clone(),
                ))
            {
                return Err(JavaInputsError::Manifest(format!(
                    "invalid or duplicate test identity: {}#{}",
                    test.class_name, test.method_name
                )));
            }
            test.module_root = root.to_string_lossy().into();
            test.source_path = source.to_string_lossy().into();
            discovered.push(test);
        }
        if !discovered.is_empty() && bytecode.test_class_dirs.is_empty() {
            return Err(JavaInputsError::Manifest(
                "tests supplied without test class directories".into(),
            ));
        }
        discovered.sort_by(|a, b| {
            (&a.class_name, &a.method_name, &a.descriptor).cmp(&(
                &b.class_name,
                &b.method_name,
                &b.descriptor,
            ))
        });
        let resolved = ResolvedJavaModule {
            root: root.clone(),
            source_roots: source_roots.clone(),
            test_source_roots: test_source_roots.clone(),
            bytecode,
            dependencies,
            dependency_inventory_complete: module.dependency_inventory_complete,
            test_inventory_complete: module.test_inventory_complete,
            tests: discovered,
        };
        if let Some(existing) = inputs.modules.get(&root) {
            if existing != &resolved
                || inputs.contexts.iter().any(|c| {
                    c.module_roots.contains(&root.to_string_lossy().to_string())
                        && c.build_context != context
                })
            {
                return Err(JavaInputsError::Manifest(format!(
                    "conflicting shared module/context: {}",
                    root.display()
                )));
            }
        } else {
            inputs.modules.insert(root.clone(), resolved);
        }
        roots.push(root.to_string_lossy().into());
        sources.extend(
            source_roots
                .iter()
                .map(|p| p.to_string_lossy().into_owned()),
        );
        tests.extend(
            test_source_roots
                .iter()
                .map(|p| p.to_string_lossy().into_owned()),
        );
    }
    for list in [&mut roots, &mut sources, &mut tests] {
        list.sort();
        list.dedup();
    }
    inputs.contexts.push(JavaAnalysisContextRecord {
        service_name: service.name.clone(),
        service_root: service_root.to_string_lossy().into(),
        manifest_path: path.to_string_lossy().into(),
        manifest_digest: statix::strings::hash_text(&raw),
        candidate_revision: revision.into(),
        build_context: context,
        module_roots: roots,
        source_roots: sources,
        test_source_roots: tests,
    });
    Ok(())
}

fn identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_alphabetic() || matches!(c, '_' | '$'))
        && chars.all(|c| c.is_alphanumeric() || matches!(c, '_' | '$'))
}

fn descriptor(s: &str) -> bool {
    fn ty(bytes: &[u8], index: &mut usize, allow_void: bool) -> bool {
        let Some(c) = bytes.get(*index).copied() else {
            return false;
        };
        *index += 1;
        match c {
            b'V' => allow_void,
            b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' => true,
            b'[' => ty(bytes, index, false),
            b'L' => {
                let start = *index;
                while bytes.get(*index).is_some_and(|c| *c != b';') {
                    *index += 1;
                }
                let valid = *index > start && bytes.get(*index) == Some(&b';');
                *index += 1;
                valid
            }
            _ => false,
        }
    }
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'(') {
        return false;
    }
    let mut index = 1;
    while bytes.get(index) != Some(&b')') {
        if !ty(bytes, &mut index, false) {
            return false;
        }
    }
    index += 1;
    ty(bytes, &mut index, true) && index == bytes.len()
}
