//! Real-provider acceptance controls; no build descriptors exist in the fixture.
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn write(root: &Path, path: &str, code: &str) {
    let p = root.join(path);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, code).unwrap();
}
fn command(command: &mut Command) -> Output {
    let out = command.output().unwrap();
    assert!(
        out.status.success(),
        "{:?}\n{}\n{}",
        command,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}
fn git(root: &Path, args: &[&str]) -> String {
    String::from_utf8(command(Command::new("git").current_dir(root).args(args)).stdout)
        .unwrap()
        .trim()
        .into()
}
fn sources(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    for e in fs::read_dir(root).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            out.extend(sources(&p));
        } else if p.extension().is_some_and(|e| e == "java") {
            out.push(p);
        }
    }
    out.sort();
    out
}
fn compile(source: &Path, output: &Path, classpath: &[PathBuf]) {
    fs::create_dir_all(output).unwrap();
    let mut cmd = Command::new("javac");
    cmd.args(["--release", "8", "-d"]).arg(output);
    if !classpath.is_empty() {
        cmd.arg("-cp").arg(std::env::join_paths(classpath).unwrap());
    }
    command(cmd.args(sources(source)));
}

struct Fixture {
    directory: tempfile::TempDir,
    root: PathBuf,
    baseline: String,
    candidate: String,
    manifest: Value,
    cross: bool,
}
impl Fixture {
    fn new(cross: bool) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("project");
        fs::create_dir(&root).unwrap();
        git(&root, &["init", "-q"]);
        git(&root, &["config", "user.name", "Fixture"]);
        git(&root, &["config", "user.email", "fixture@example.invalid"]);
        write(&root, ".gitignore", "**/compiled/\ndependencies/\n");
        write(
            &root,
            "app/source/java/fixture/Service.java",
            if cross {
                "package fixture; public class Service { public static int value(){return Shared.value();} public static int stable(){return 7;} }"
            } else {
                "package fixture; public class Service { public static int value(){return 1;} public static int stable(){return 7;} }"
            },
        );
        if cross {
            write(
                &root,
                "library/source/java/fixture/Shared.java",
                "package fixture; public class Shared { public static int value(){return 1;} }",
            );
        }
        write(
            &root,
            "app/checks/java/fixture/ServiceTest.java",
            "package fixture; public class ServiceTest { @org.junit.jupiter.api.Test public void coversChanged(){if(Service.value()!=1) throw new AssertionError();} @org.junit.jupiter.api.Test public void coversUnchanged(){if(Service.stable()!=7) throw new AssertionError();} }",
        );
        if cross {
            write(
                &root,
                "independent/checks/java/fixture/IndependentTest.java",
                "package fixture; public class IndependentTest { @org.junit.jupiter.api.Test public void independent(){if(2+2!=4) throw new AssertionError();} }",
            );
        }
        git(&root, &["add", "."]);
        git(&root, &["commit", "-qm", "baseline"]);
        let baseline = git(&root, &["rev-parse", "HEAD"]);
        write(
            directory.path(),
            "stubs/org/junit/jupiter/api/Test.java",
            "package org.junit.jupiter.api; public @interface Test {}",
        );
        compile(
            &directory.path().join("stubs"),
            &root.join("dependencies/junit"),
            &[],
        );
        let mut f = Self {
            directory,
            root,
            baseline,
            candidate: String::new(),
            manifest: Value::Null,
            cross,
        };
        f.build();
        assert!(f.run_suite().status.success());
        let changed = if cross {
            "library/source/java/fixture/Shared.java"
        } else {
            "app/source/java/fixture/Service.java"
        };
        let code = fs::read_to_string(f.root.join(changed))
            .unwrap()
            .replace("return 1;", "return 2;");
        write(&f.root, changed, &code);
        git(&f.root, &["add", "."]);
        git(&f.root, &["commit", "-qm", "candidate"]);
        f.candidate = git(&f.root, &["rev-parse", "HEAD"]);
        f.build();
        let actual = f.run_suite();
        assert!(!actual.status.success());
        assert_eq!(
            String::from_utf8(actual.stdout).unwrap().trim(),
            "coversChanged"
        );
        let mut modules = vec![f.module("app", if cross { vec!["library"] } else { vec![] })];
        if cross {
            modules.push(f.module("library", vec![]));
            modules.push(f.module("independent", vec![]));
        }
        f.manifest = json!({"schema_version":1,"candidate_revision":f.candidate,"build_context":{"tool":"maven","requested_profiles":[],"active_profiles":[],"java_version":"17"},"module_inventory_complete":true,"modules":modules});
        write(f.directory.path(),"config.json",&json!({"service_descriptions":[{"name":"fixture","base_dir_path":".","urls":[],"java_analysis":{"artifacts_file":f.directory.path().join("inputs.json")}}]}).to_string());
        f
    }
    fn build(&self) {
        let mut cp = vec![self.root.join("dependencies/junit")];
        if self.cross {
            compile(
                &self.root.join("library/source/java"),
                &self.root.join("library/compiled/main"),
                &cp,
            );
            cp.push(self.root.join("library/compiled/main"));
        }
        compile(
            &self.root.join("app/source/java"),
            &self.root.join("app/compiled/main"),
            &cp,
        );
        cp.push(self.root.join("app/compiled/main"));
        compile(
            &self.root.join("app/checks/java"),
            &self.root.join("app/compiled/tests"),
            &cp,
        );
        cp.push(self.root.join("app/compiled/tests"));
        if self.cross {
            compile(
                &self.root.join("independent/checks/java"),
                &self.root.join("independent/compiled/tests"),
                &cp,
            );
            cp.push(self.root.join("independent/compiled/tests"));
        }
        write(
            self.directory.path(),
            "runner/Runner.java",
            &format!(
                "public class Runner {{public static void main(String[] args) {{boolean failed=false;try{{new fixture.ServiceTest().coversChanged();}}catch(Throwable t){{System.out.println(\"coversChanged\");failed=true;}}try{{new fixture.ServiceTest().coversUnchanged();}}catch(Throwable t){{System.out.println(\"coversUnchanged\");failed=true;}}{}if(failed)System.exit(1);}}}}",
                if self.cross {
                    "try{new fixture.IndependentTest().independent();}catch(Throwable t){System.out.println(\"independent\");failed=true;}"
                } else {
                    ""
                }
            ),
        );
        compile(
            &self.directory.path().join("runner"),
            &self.directory.path().join("runner-classes"),
            &cp,
        );
    }
    fn run_suite(&self) -> Output {
        let mut cp = vec![
            self.directory.path().join("runner-classes"),
            self.root.join("dependencies/junit"),
            self.root.join("app/compiled/main"),
            self.root.join("app/compiled/tests"),
        ];
        if self.cross {
            cp.extend([
                self.root.join("library/compiled/main"),
                self.root.join("independent/compiled/tests"),
            ]);
        }
        Command::new("java")
            .arg("-cp")
            .arg(std::env::join_paths(cp).unwrap())
            .arg("Runner")
            .output()
            .unwrap()
    }
    fn module(&self, name: &str, deps: Vec<&str>) -> Value {
        let root = self.root.join(name);
        let source = root.join("source/java");
        let checks = root.join("checks/java");
        let tests = if checks.is_dir() {
            extractor_runtime::impact::discover_java_tests_in_roots(&root, &[checks]).unwrap()
        } else {
            vec![]
        };
        json!({"root":name,"source_roots":if source.is_dir(){vec!["source/java"]}else{vec![]},"test_source_roots":if root.join("checks/java").is_dir(){vec!["checks/java"]}else{vec![]},"application_class_dirs":if root.join("compiled/main").is_dir(){vec!["compiled/main"]}else{vec![]},"test_class_dirs":if root.join("compiled/tests").is_dir(){vec!["compiled/tests"]}else{vec![]},"classpath":[self.root.join("dependencies/junit")],"dependencies":deps,"dependency_inventory_complete":true,"test_inventory_complete":true,"tests":tests})
    }
    fn analyze(&self, provider: &str) -> Value {
        write(
            self.directory.path(),
            "inputs.json",
            &self.manifest.to_string(),
        );
        let tool = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let output = self.directory.path().join(format!("{provider}.json"));
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_test-impact"));
        cmd.arg("--project-dir")
            .arg(&self.root)
            .args(["--baseline-revision", &self.baseline, "--config-file"])
            .arg(self.directory.path().join("config.json"))
            .arg("--output")
            .arg(&output)
            .arg("--quiet");
        if provider == "wala" {
            cmd.arg("--wala-adapter-jar")
                .arg(tool.join("wala-callgraph/target/wala-callgraph-all.jar"));
        } else {
            cmd.args([
                "--java-callgraph-provider",
                "sootup",
                "--java-callgraph-adapter-jar",
            ])
            .arg(tool.join("sootup-callgraph/target/sootup-callgraph-all.jar"));
        }
        command(&mut cmd);
        serde_json::from_slice(&fs::read(output).unwrap()).unwrap()
    }
}

#[test]
fn both_providers_select_the_mutation_failure_using_precompiled_nonstandard_roots() {
    let f = Fixture::new(false);
    for provider in ["wala", "sootup"] {
        let result = f.analyze(provider);
        assert_eq!(
            result["selected_tests"].as_array().unwrap().len(),
            1,
            "{provider}: {}",
            result["diagnostics"]
        );
        assert_eq!(
            result["selected_tests"][0]["test"]["method_name"],
            "coversChanged"
        );
        assert_eq!(result["unselected_tests"].as_array().unwrap().len(), 1);
        assert!(result["diagnostics"].as_array().unwrap().iter().any(|d| {
            d.as_str()
                .unwrap()
                .starts_with("artifact preparation skipped:")
        }));
        assert_eq!(result["analysis_contexts"].as_array().unwrap().len(), 1);
    }
}

#[test]
fn both_providers_retain_the_failing_consumer_and_require_full_run_for_incomplete_scope() {
    let mut f = Fixture::new(true);
    for provider in ["wala", "sootup"] {
        let result = f.analyze(provider);
        assert_eq!(
            result["selected_tests"].as_array().unwrap().len(),
            1,
            "{provider}: {}",
            result["diagnostics"]
        );
        assert_eq!(
            result["selected_tests"][0]["test"]["method_name"],
            "coversChanged"
        );
        assert_eq!(result["unselected_tests"].as_array().unwrap().len(), 2);
    }
    for flag in ["dependency_inventory_complete", "test_inventory_complete"] {
        f.manifest["modules"][0][flag] = json!(false);
        let result = f.analyze("wala");
        assert_eq!(result["requires_full_test_run"], true);
        assert_eq!(result["unselected_tests"], json!([]));
        f.manifest["modules"][0][flag] = json!(true);
    }
}
