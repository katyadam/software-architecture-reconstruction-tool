use super::ImpactError;
use models::JavaTestCase;
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Parser};

const TEST_ANNOTATIONS: &[&str] = &["Test", "ParameterizedTest", "RepeatedTest", "TestFactory"];

/// Discovers JUnit 4/5 test methods below a conventional Maven test-source directory.
pub fn discover_java_tests(module_root: &Path) -> Result<Vec<JavaTestCase>, ImpactError> {
    let source_root = module_root.join("src/test/java");
    let mut tests = Vec::new();
    for source_path in java_sources(&source_root)? {
        tests.extend(discover_file(module_root, &source_path)?);
    }
    Ok(tests)
}

/// Recursively collects Java source files below one Maven test-source directory.
fn java_sources(root: &Path) -> Result<Vec<PathBuf>, ImpactError> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut sources = Vec::new();
    for entry in std::fs::read_dir(root).map_err(|error| ImpactError::Source(error.to_string()))? {
        let path = entry
            .map_err(|error| ImpactError::Source(error.to_string()))?
            .path();
        if path.is_dir() {
            sources.extend(java_sources(&path)?);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "java")
        {
            sources.push(path);
        }
    }
    Ok(sources)
}

/// Extracts annotated JUnit methods from one Java source file.
fn discover_file(module_root: &Path, source_path: &Path) -> Result<Vec<JavaTestCase>, ImpactError> {
    let source = std::fs::read_to_string(source_path)
        .map_err(|error| ImpactError::Source(error.to_string()))?;
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .expect("Java grammar is valid");
    let tree = parser
        .parse(&source, None)
        .expect("Java source parses into a tree");
    let package = package_name(tree.root_node(), &source);
    let mut tests = Vec::new();
    collect_methods(
        tree.root_node(),
        &source,
        &package,
        module_root,
        source_path,
        &mut tests,
    );
    Ok(tests)
}

/// Recursively collects test methods while retaining Java nesting through their parent declarations.
fn collect_methods(
    node: Node<'_>,
    source: &str,
    package: &str,
    module_root: &Path,
    source_path: &Path,
    tests: &mut Vec<JavaTestCase>,
) {
    if node.kind() == "method_declaration" && is_test_method(node, source) {
        if let Some(test) = test_case(node, source, package, module_root, source_path) {
            tests.push(test);
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_methods(child, source, package, module_root, source_path, tests);
    }
}

/// Returns whether one method has a supported JUnit test annotation.
fn is_test_method(method: Node<'_>, source: &str) -> bool {
    modifiers(method).is_some_and(|modifiers| {
        annotations(modifiers, source)
            .iter()
            .any(|name| TEST_ANNOTATIONS.contains(&name.as_str()))
    })
}

/// Returns the modifiers child of one Java method declaration.
fn modifiers(method: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = method.walk();
    method
        .children(&mut cursor)
        .find(|node| node.kind() == "modifiers")
}

/// Builds a stable JVM-level test identity when its declaration can be represented safely.
fn test_case(
    method: Node<'_>,
    source: &str,
    package: &str,
    module_root: &Path,
    source_path: &Path,
) -> Option<JavaTestCase> {
    let class_name = enclosing_class_name(method, source, package)?;
    let method_name = text(method.child_by_field_name("name")?, source)?.to_owned();
    let descriptor = method_descriptor(method, source)?;
    Some(JavaTestCase {
        module_root: module_root.to_string_lossy().into_owned(),
        callable_signature: format!(
            "{}.{}{}",
            class_name.trim_start_matches('L').rsplit('/').next()?,
            method_name,
            descriptor
        ),
        class_name,
        method_name,
        descriptor,
        source_path: source_path.to_string_lossy().into_owned(),
    })
}

/// Returns annotation terminal names from one Java modifiers node.
fn annotations(modifiers: Node<'_>, source: &str) -> Vec<String> {
    let mut cursor = modifiers.walk();
    modifiers
        .children(&mut cursor)
        .filter(|node| matches!(node.kind(), "annotation" | "marker_annotation"))
        .filter_map(|node| text(node, source))
        .filter_map(|annotation| annotation.trim_start_matches('@').split(['(', ' ']).next())
        .filter_map(|name| name.rsplit('.').next())
        .map(str::to_owned)
        .collect()
}

/// Returns the declared package name or an empty package for default-package tests.
fn package_name(root: Node<'_>, source: &str) -> String {
    let mut cursor = root.walk();
    root.children(&mut cursor)
        .find(|node| node.kind() == "package_declaration")
        .and_then(|node| text(node, source))
        .map(|value| {
            value
                .trim_start_matches("package")
                .trim_end_matches(';')
                .trim()
                .to_owned()
        })
        .unwrap_or_default()
}

/// Builds the JVM internal name of the enclosing top-level or nested class.
fn enclosing_class_name(method: Node<'_>, source: &str, package: &str) -> Option<String> {
    let mut classes = Vec::new();
    let mut current = method.parent();
    while let Some(node) = current {
        if matches!(node.kind(), "class_declaration" | "interface_declaration") {
            classes.push(text(node.child_by_field_name("name")?, source)?.to_owned());
        }
        current = node.parent();
    }
    classes.reverse();
    (!classes.is_empty()).then(|| {
        let path = if package.is_empty() {
            classes.join("$")
        } else {
            format!("{}/{}", package.replace('.', "/"), classes.join("$"))
        };
        format!("L{path}")
    })
}

/// Converts one Java method declaration to its JVM parameter-and-return descriptor.
fn method_descriptor(method: Node<'_>, source: &str) -> Option<String> {
    let parameters = method.child_by_field_name("parameters")?;
    let mut cursor = parameters.walk();
    let descriptors: Option<Vec<_>> = parameters
        .children(&mut cursor)
        .filter(|node| matches!(node.kind(), "formal_parameter" | "spread_parameter"))
        .map(|parameter| {
            parameter
                .child_by_field_name("type")
                .and_then(|node| text(node, source))
                .and_then(java_type_descriptor)
        })
        .collect();
    let return_type = method
        .child_by_field_name("type")
        .and_then(|node| text(node, source))
        .and_then(java_type_descriptor)?;
    Some(format!("({}){return_type}", descriptors?.join("")))
}

/// Converts a Java source type into a best-effort JVM descriptor for WALA selector matching.
fn java_type_descriptor(value: &str) -> Option<String> {
    let value = value.trim();
    let (arrays, base) = value.chars().fold((0, value), |(count, remainder), _| {
        if remainder.ends_with("[]") {
            (count + 1, &remainder[..remainder.len() - 2])
        } else {
            (count, remainder)
        }
    });
    let descriptor = match base.trim() {
        "void" => "V".to_owned(),
        "boolean" => "Z".to_owned(),
        "byte" => "B".to_owned(),
        "char" => "C".to_owned(),
        "short" => "S".to_owned(),
        "int" => "I".to_owned(),
        "long" => "J".to_owned(),
        "float" => "F".to_owned(),
        "double" => "D".to_owned(),
        "String" | "java.lang.String" => "Ljava/lang/String;".to_owned(),
        value if !value.is_empty() => format!("L{};", value.split('<').next()?.replace('.', "/")),
        _ => return None,
    };
    Some(format!("{}{}", "[".repeat(arrays), descriptor))
}

/// Reads source text for one Tree-sitter node.
fn text<'a>(node: Node<'_>, source: &'a str) -> Option<&'a str> {
    node.utf8_text(source.as_bytes()).ok()
}
