//! Conservative project dependency metadata. Unsupported expressions return uncertainty.
use super::ModuleCatalog;
use quick_xml::{Reader, events::Event};
use regex::Regex;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

pub struct ModuleDependencyGraph {
    dependencies: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
}

impl ModuleDependencyGraph {
    pub fn load(root: &Path) -> Result<Self, String> {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let catalog = ModuleCatalog::load(&root);
        if let Some(reason) = catalog.diagnostic() {
            return Err(reason.into());
        }
        let modules = catalog.descendants_of(&root);
        let dependencies = if root.join("pom.xml").is_file() {
            maven_dependencies(&modules)?
        } else {
            gradle_dependencies(&root, &modules)?
        };
        Ok(Self { dependencies })
    }

    pub fn load_with_configured_inputs(
        root: &Path,
        inputs: &crate::java_analysis::ConfiguredJavaInputs,
    ) -> Result<Self, String> {
        if inputs.modules.is_empty() {
            return Self::load(root);
        }
        if !inputs.module_inventory_complete
            || inputs
                .modules
                .values()
                .any(|m| !m.dependency_inventory_complete)
        {
            return Err("configured module/dependency inventory incomplete".into());
        }
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let automatic = Self::load(&root);
        let mut dependencies = automatic
            .as_ref()
            .map(|g| g.dependencies.clone())
            .unwrap_or_default();
        // When the automatic graph succeeded, check for on-disk build modules that are
        // neither in the manifest nor already claimed by the reactor. A module the automatic
        // graph knows about can be intentionally excluded (manifest is authoritative);
        // a module it never claimed is an independent build that still blocks independence.
        // When the automatic graph failed (unsupported syntax etc.), skip the check and
        // trust the manifest's module_inventory_complete=true claim.
        if let Ok(auto) = &automatic {
            let auto_modules: BTreeSet<_> = auto.dependencies.keys().collect();
            let known = super::java_tests::checkout_modules(&root).map_err(|e| e.to_string())?;
            for module in known {
                let build = module.join("pom.xml").is_file()
                    || module.join("build.gradle").is_file()
                    || module.join("build.gradle.kts").is_file()
                    || module.join("settings.gradle").is_file()
                    || module.join("settings.gradle.kts").is_file();
                if build
                    && !inputs.modules.contains_key(&module)
                    && !dependencies.contains_key(&module)
                    && !auto_modules.contains(&module)
                {
                    return Err(format!(
                        "consumer dependency scope unresolved: {}",
                        module.display()
                    ));
                }
            }
        }
        for service in &inputs.services {
            if service.manifest_path.is_none() && !dependencies.contains_key(&service.root) {
                return Err(format!(
                    "unconfigured service dependencies: {}",
                    service.root.display()
                ));
            }
        }
        for module in inputs.modules.values() {
            dependencies.insert(module.root.clone(), module.dependencies.clone());
        }
        Ok(Self { dependencies })
    }

    pub fn modules(&self) -> impl Iterator<Item = &PathBuf> {
        self.dependencies.keys()
    }

    /// Returns the set of module paths that the reactor (settings.gradle / root pom.xml)
    /// claims, regardless of whether a full dependency analysis succeeded.  Returns an
    /// empty set when no supported reactor file is found.
    pub fn reactor_claimed_modules(root: &Path) -> std::collections::BTreeSet<PathBuf> {
        let Ok(root) = root.canonicalize() else {
            return Default::default();
        };
        if root.join("pom.xml").is_file() {
            reactor_modules_maven(&root).unwrap_or_default()
        } else {
            reactor_modules_gradle(&root).unwrap_or_default()
        }
    }

    pub fn consumers_of(&self, module: &Path) -> BTreeSet<PathBuf> {
        let mut seen = BTreeSet::from([module.to_path_buf()]);
        let mut pending = vec![module.to_path_buf()];
        while let Some(current) = pending.pop() {
            for (consumer, dependencies) in &self.dependencies {
                if dependencies.contains(&current) && seen.insert(consumer.clone()) {
                    pending.push(consumer.clone());
                }
            }
        }
        seen.remove(module);
        seen
    }

    pub fn upstream_of(&self, module: &Path) -> BTreeSet<PathBuf> {
        let mut seen = BTreeSet::from([module.to_path_buf()]);
        let mut pending = vec![module.to_path_buf()];
        while let Some(current) = pending.pop() {
            for upstream in self.dependencies.get(&current).into_iter().flatten() {
                if seen.insert(upstream.clone()) {
                    pending.push(upstream.clone());
                }
            }
        }
        seen.remove(module);
        seen
    }
}

#[derive(Clone, Default)]
struct Pom {
    values: BTreeMap<String, String>,
    dependencies: Vec<BTreeMap<String, String>>,
}

fn parse_pom(path: &Path) -> Result<Pom, String> {
    let source = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut reader = Reader::from_str(&source);
    reader.config_mut().trim_text(true);
    let mut stack = Vec::<String>::new();
    let mut pom = Pom::default();
    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) => {
                let name = String::from_utf8_lossy(tag.local_name().as_ref()).into_owned();
                if matches!(name.as_str(), "profiles" | "dependencyManagement") {
                    return Err(format!(
                        "Maven {name} requires effective dependency metadata: {}",
                        path.display()
                    ));
                }
                stack.push(name);
                if stack.join("/") == "project/dependencies/dependency" {
                    pom.dependencies.push(BTreeMap::new());
                }
            }
            Ok(Event::Text(value)) => {
                let value = value.decode().map_err(|e| e.to_string())?.to_string();
                if stack.len() == 4 && stack[..3] == ["project", "dependencies", "dependency"] {
                    pom.dependencies
                        .last_mut()
                        .ok_or("invalid dependency")?
                        .entry(stack[3].clone())
                        .or_default()
                        .push_str(&value);
                } else {
                    pom.values
                        .entry(stack.join("/"))
                        .or_default()
                        .push_str(&value);
                }
            }
            Ok(Event::Empty(tag)) => {
                if tag.local_name().as_ref() == b"relativePath"
                    && stack.as_slice() == ["project", "parent"]
                {
                    return Err("Maven external parent needs effective metadata".into());
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Unable to read {}: {e}", path.display())),
            _ => {}
        }
    }
    Ok(pom)
}

fn expand(value: &str, properties: &BTreeMap<String, String>) -> Result<String, String> {
    let mut value = value.to_string();
    for _ in 0..32 {
        let Some(start) = value.find("${") else {
            return Ok(value);
        };
        let end = start + value[start..].find('}').ok_or("invalid Maven property")?;
        let key = &value[start + 2..end];
        let replacement = properties
            .get(key)
            .ok_or_else(|| format!("unresolved Maven dependency property: {key}"))?;
        value.replace_range(start..=end, replacement);
    }
    Err("cyclic Maven properties".into())
}

fn effective_pom(
    module: &Path,
    poms: &BTreeMap<PathBuf, Pom>,
    visiting: &mut BTreeSet<PathBuf>,
) -> Result<Pom, String> {
    if !visiting.insert(module.to_path_buf()) {
        return Err("cyclic Maven parent POMs".into());
    }
    let own = poms
        .get(module)
        .ok_or("parent POM is outside the declared reactor")?;
    let parent = if own.values.contains_key("project/parent/artifactId") {
        let relative = own
            .values
            .get("project/parent/relativePath")
            .map(String::as_str)
            .unwrap_or("../pom.xml");
        let parent = module
            .join(relative)
            .canonicalize()
            .map_err(|_| "Maven parent metadata is not locally available")?;
        Some(effective_pom(
            parent.parent().ok_or("invalid Maven parent")?,
            poms,
            visiting,
        )?)
    } else {
        None
    };
    let mut result = parent.clone().unwrap_or_default();
    result
        .values
        .retain(|key, _| key.starts_with("project/properties/"));
    result.values.extend(own.values.clone());
    for name in ["groupId", "version"] {
        let key = format!("project/{name}");
        if !result.values.contains_key(&key) {
            let inherited = own
                .values
                .get(&format!("project/parent/{name}"))
                .ok_or_else(|| format!("missing Maven {name}"))?;
            result.values.insert(key, inherited.clone());
        }
    }
    let (_, _, _, properties) = coordinates(&result)?;
    if let Some(parent) = &parent {
        let (group, artifact, version, _) = coordinates(parent)?;
        for (name, expected) in [
            ("groupId", group),
            ("artifactId", artifact),
            ("version", version),
        ] {
            let declared = own
                .values
                .get(&format!("project/parent/{name}"))
                .ok_or("missing Maven parent coordinate")?;
            if expand(declared, &properties)? != expected {
                return Err("Maven relative parent coordinates do not match".into());
            }
        }
    }
    result.dependencies.extend(own.dependencies.clone());
    // Resolve inherited declarations in their declaring POM's property context.
    for dependency in &mut result.dependencies {
        for value in dependency.values_mut() {
            *value = expand(value, &properties)?;
        }
    }
    visiting.remove(module);
    Ok(result)
}

fn coordinates(pom: &Pom) -> Result<(String, String, String, BTreeMap<String, String>), String> {
    let mut properties: BTreeMap<_, _> = pom
        .values
        .iter()
        .filter_map(|(key, value)| {
            key.strip_prefix("project/properties/")
                .map(|key| (key.to_string(), value.clone()))
        })
        .collect();
    for name in ["groupId", "artifactId", "version"] {
        let value = pom
            .values
            .get(&format!("project/{name}"))
            .ok_or_else(|| format!("missing Maven {name}"))?;
        properties.insert(format!("project.{name}"), value.clone());
        properties.insert(format!("pom.{name}"), value.clone());
    }
    Ok((
        expand(&properties["project.groupId"], &properties)?,
        expand(&properties["project.artifactId"], &properties)?,
        expand(&properties["project.version"], &properties)?,
        properties,
    ))
}

fn maven_dependencies(modules: &[PathBuf]) -> Result<BTreeMap<PathBuf, BTreeSet<PathBuf>>, String> {
    let poms: BTreeMap<_, _> = modules
        .iter()
        .map(|module| Ok((module.clone(), parse_pom(&module.join("pom.xml"))?)))
        .collect::<Result<_, String>>()?;
    let mut resolved = BTreeMap::new();
    let mut owners = BTreeMap::new();
    for module in modules {
        let pom = effective_pom(module, &poms, &mut BTreeSet::new())?;
        let (group, artifact, version, properties) = coordinates(&pom)?;
        if owners
            .insert((group, artifact, version), module.clone())
            .is_some()
        {
            return Err("duplicate reactor artifact coordinates".into());
        }
        resolved.insert(module.clone(), (pom, properties));
    }
    let mut graph = BTreeMap::new();
    for (module, (pom, properties)) in resolved {
        let mut dependencies = BTreeSet::new();
        for dependency in pom.dependencies {
            let get = |key: &str| {
                expand(
                    dependency
                        .get(key)
                        .ok_or_else(|| format!("missing dependency {key}"))?,
                    &properties,
                )
            };
            let key = (get("groupId")?, get("artifactId")?, get("version")?);
            if key.2.contains(['[', ']', '(', ')'])
                || matches!(key.2.as_str(), "LATEST" | "RELEASE")
            {
                return Err("Maven dynamic dependency version needs effective metadata".into());
            }
            if let Some(owner) = owners.get(&key) {
                dependencies.insert(owner.clone());
            }
        }
        graph.insert(module, dependencies);
    }
    Ok(graph)
}

/// Extracts declared submodule paths from a root pom.xml <modules> block.
fn reactor_modules_maven(root: &Path) -> Result<std::collections::BTreeSet<PathBuf>, String> {
    let source = fs::read_to_string(root.join("pom.xml")).map_err(|e| e.to_string())?;
    let mut reader = Reader::from_str(&source);
    reader.config_mut().trim_text(true);
    let mut stack = Vec::<String>::new();
    let mut modules = std::collections::BTreeSet::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) => {
                stack.push(String::from_utf8_lossy(tag.local_name().as_ref()).into_owned());
            }
            Ok(Event::Text(value)) => {
                if stack == ["project", "modules", "module"] {
                    let name = value.decode().map_err(|e| e.to_string())?.trim().to_string();
                    if !name.is_empty() {
                        if let Ok(p) = root.join(&name).canonicalize() {
                            modules.insert(p);
                        }
                    }
                }
            }
            Ok(Event::End(_)) => { stack.pop(); }
            Ok(Event::Eof) => break,
            _ => {}
        }
    }
    Ok(modules)
}

/// Extracts declared submodule paths from a Gradle settings file.
/// Accepts both `include ':module'` (colon prefix) and `include 'module'` (bare name).
fn reactor_modules_gradle(root: &Path) -> Result<std::collections::BTreeSet<PathBuf>, String> {
    let settings = ["settings.gradle", "settings.gradle.kts"]
        .into_iter()
        .find_map(|name| fs::read_to_string(root.join(name)).ok())
        .ok_or("no Gradle settings")?;
    // Match both `include(':name')` and `include('name')`, with or without colon.
    let re = Regex::new(r#"include\s*\(?\s*["']:?([^"'$:]+)["']"#).unwrap();
    let mut modules = std::collections::BTreeSet::new();
    for cap in re.captures_iter(&settings) {
        let name = cap[1].trim();
        if !name.is_empty() {
            // Gradle uses colon as path separator for nested subprojects; convert to filesystem path.
            let rel_path = name.replace(':', std::path::MAIN_SEPARATOR_STR);
            if let Ok(p) = root.join(&rel_path).canonicalize() {
                modules.insert(p);
            }
        }
    }
    Ok(modules)
}

fn gradle_dependencies(
    root: &Path,
    modules: &[PathBuf],
) -> Result<BTreeMap<PathBuf, BTreeSet<PathBuf>>, String> {
    let settings = ["settings.gradle", "settings.gradle.kts"]
        .into_iter()
        .find_map(|name| fs::read_to_string(root.join(name)).ok())
        .ok_or("missing Gradle settings")?;
    if [
        "projectDir",
        "includeBuild",
        "dependencySubstitution",
        "pluginManagement",
    ]
    .iter()
    .any(|token| settings.contains(token))
    {
        return Err("Gradle custom settings require evaluated dependency metadata".into());
    }
    let settings_line = Regex::new(r#"^(?:include\s*\(?\s*["']:[^"'$]+["'](?:\s*,\s*["']:[^"'$]+["'])*\s*\)?|rootProject\.name\s*=\s*["'][^"'$]+["'])\s*;?$"#).unwrap();
    if settings
        .lines()
        .any(|line| !line.trim().is_empty() && !settings_line.is_match(line.trim()))
    {
        return Err("Gradle settings are outside the supported literal subset".into());
    }
    let projects: BTreeMap<_, _> = modules
        .iter()
        .map(|path| {
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, ":");
            (format!(":{relative}"), path.clone())
        })
        .collect();
    let literal = Regex::new(r#"\bproject\s*\(\s*["'](:[^"'$]+)["']\s*\)"#).unwrap();
    let plugins = Regex::new(r#"plugins\s*\{\s*(?:(?:java|`java-library`|application|id\s*\(?\s*["'](?:java|java-library|application)["']\s*\)?)\s*;?\s*)*\}"#).unwrap();
    let dependency_block = Regex::new(r"dependencies\s*\{([^{}]*)\}").unwrap();
    let repositories =
        Regex::new(r"repositories\s*\{\s*(?:(?:mavenCentral|mavenLocal)\(\)\s*;?\s*)*\}").unwrap();
    let test_task = Regex::new(r"test\s*\{\s*useJUnitPlatform\(\)\s*;?\s*\}").unwrap();
    let declaration = Regex::new(r#"\b(?:api|implementation|compileOnly|runtimeOnly|testImplementation|testRuntimeOnly|testCompileOnly)\s*\(?\s*(?:project\s*\(\s*["']:[^"'$]+["']\s*\)|["'][^"'$]+["'])\s*\)?\s*;?"#).unwrap();
    let mut graph = BTreeMap::new();
    for module in modules {
        let source = ["build.gradle", "build.gradle.kts"]
            .into_iter()
            .find_map(|name| fs::read_to_string(module.join(name)).ok())
            .unwrap_or_default();
        if [
            "subprojects",
            "allprojects",
            "apply",
            "includeBuild",
            "libs.",
            "buildSrc",
        ]
        .iter()
        .any(|token| source.contains(token))
            || root.join("buildSrc").exists()
            || root.join("build-logic").exists()
        {
            return Err(
                "Gradle convention or shared dependency metadata needs evaluated build metadata"
                    .into(),
            );
        }
        // Accept only the intentionally supported literal build subset. Anything
        // executable outside it may add dependencies indirectly and must widen scope.
        let without_plugins = plugins.replace_all(&source, "");
        let without_repositories = repositories.replace_all(&without_plugins, "");
        let without_plugins = test_task.replace_all(&without_repositories, "");
        for block in dependency_block.captures_iter(&without_plugins) {
            if !declaration.replace_all(&block[1], "").trim().is_empty() {
                return Err(
                    "Gradle dependency declaration requires evaluated build metadata".into(),
                );
            }
        }
        if !dependency_block
            .replace_all(&without_plugins, "")
            .trim()
            .is_empty()
        {
            return Err("Gradle build is outside the supported literal dependency subset".into());
        }
        let mut dependencies = BTreeSet::new();
        for capture in literal.captures_iter(&source) {
            dependencies.insert(
                projects
                    .get(&capture[1])
                    .ok_or("unknown Gradle project dependency")?
                    .clone(),
            );
        }
        graph.insert(module.clone(), dependencies);
    }
    Ok(graph)
}
