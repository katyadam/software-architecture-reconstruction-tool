use extractor_runtime::impact::ModuleDependencyGraph;
use std::{fs, path::Path};

fn write(root: &Path, path: &str, content: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

#[test]
fn maven_consumers_include_transitive_dependencies_but_not_independent_modules() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    write(
        &root,
        "pom.xml",
        "<project><groupId>control</groupId><artifactId>root</artifactId><version>1</version><modules><module>lib</module><module>middle</module><module>app</module><module>other</module></modules></project>",
    );
    for (name, dependency) in [
        ("lib", ""),
        ("middle", "lib"),
        ("app", "middle"),
        ("other", ""),
    ] {
        let dependencies = if dependency.is_empty() {
            String::new()
        } else {
            format!(
                "<dependencies><dependency><groupId>control</groupId><artifactId>{dependency}</artifactId><version>${{project.version}}</version></dependency></dependencies>"
            )
        };
        write(
            &root,
            &format!("{name}/pom.xml"),
            &format!(
                "<project><parent><groupId>control</groupId><artifactId>root</artifactId><version>1</version></parent><artifactId>{name}</artifactId>{dependencies}</project>"
            ),
        );
    }
    let graph = ModuleDependencyGraph::load(&root).unwrap();
    assert_eq!(
        graph.consumers_of(&root.join("lib")),
        [root.join("app"), root.join("middle")]
            .into_iter()
            .collect()
    );
    assert!(graph.consumers_of(&root.join("other")).is_empty());
    assert!(
        graph
            .upstream_of(&root.join("app"))
            .contains(&root.join("lib"))
    );
}

#[test]
fn gradle_literal_project_dependencies_and_cycles_are_resolved() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    write(
        &root,
        "settings.gradle.kts",
        "include(\":lib\", \":app\", \":other\")",
    );
    write(&root, "build.gradle.kts", "");
    write(
        &root,
        "lib/build.gradle.kts",
        "plugins { java }\ndependencies { implementation(project(\":app\")) }",
    );
    write(
        &root,
        "app/build.gradle.kts",
        "plugins { java }\ndependencies { implementation(project(\":lib\")) }",
    );
    write(&root, "other/build.gradle.kts", "plugins { java }");
    let graph = ModuleDependencyGraph::load(&root).unwrap();
    assert_eq!(
        graph.consumers_of(&root.join("lib")),
        [root.join("app")].into_iter().collect()
    );
}

#[test]
fn dynamic_gradle_project_dependency_is_uncertain() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    write(&root, "settings.gradle", "include ':lib', ':app'");
    write(&root, "build.gradle", "");
    write(&root, "lib/build.gradle", "plugins { id 'java' }");
    write(
        &root,
        "app/build.gradle",
        "dependencies { implementation project(chosenModule) }",
    );
    assert!(ModuleDependencyGraph::load(&root).is_err());
}

#[test]
fn maven_profiles_and_unresolved_properties_do_not_claim_independence() {
    for extra in [
        "<profiles><profile><dependencies/></profile></profiles>",
        "<dependencyManagement><dependencies/></dependencyManagement>",
        "<dependencies><dependency><groupId>control</groupId><artifactId>lib</artifactId><version>${unknown}</version></dependency></dependencies>",
        "<dependencies><dependency><groupId>control</groupId><artifactId>lib</artifactId><version>[1,2)</version></dependency></dependencies>",
    ] {
        let temp = tempfile::tempdir().unwrap();
        write(
            temp.path(),
            "pom.xml",
            &format!(
                "<project><groupId>control</groupId><artifactId>root</artifactId><version>1</version>{extra}</project>"
            ),
        );
        assert!(ModuleDependencyGraph::load(temp.path()).is_err(), "{extra}");
    }
}

#[test]
fn gradle_shared_or_indirect_build_logic_does_not_claim_independence() {
    for logic in [
        "apply from: 'dependencies.gradle'",
        "subprojects { dependencies { implementation project(':lib') } }",
        "configureDependencies()",
        "dependencies { implementation(findProject(\":lib\")) }",
    ] {
        let temp = tempfile::tempdir().unwrap();
        write(temp.path(), "settings.gradle", "include ':lib', ':app'");
        write(temp.path(), "build.gradle", logic);
        write(temp.path(), "lib/build.gradle", "plugins { id 'java' }");
        write(temp.path(), "app/build.gradle", "plugins { id 'java' }");
        assert!(ModuleDependencyGraph::load(temp.path()).is_err(), "{logic}");
    }
}
