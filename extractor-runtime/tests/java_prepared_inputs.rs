use extractor_runtime::{
    call_graph::{java_adapter::JavaAdapterProvider, wala::WalaJavaProvider},
    java_analysis::JavaBytecodeInputs,
};
use models::{
    JavaTestCase,
    call_graph::{CallGraphRequest, CallGraphStatus, Language},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

#[test]
fn both_providers_use_prepared_paths_without_a_preparer_class() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("Main.java");
    fs::write(&source,r#"public class Main { public static void main(String[] args) {
      String root=""; for(int i=0;i<args.length-1;i++) if(args[i].equals("--source-dir")) root=args[i+1];
      System.out.print("{\"schema_version\":2,\"status\":\"ok\",\"provider_id\":\""+root.substring(1)+"\",\"source_root\":\""+root+"\",\"algorithm\":\"cha\",\"diagnostics\":[\"ARGS="+String.join("|",args)+"\"],\"reachability\":[],\"edges\":[]}");
    }}"#).unwrap();
    assert!(
        Command::new("javac")
            .arg(&source)
            .status()
            .unwrap()
            .success()
    );
    let jar = dir.path().join("adapter.jar");
    assert!(
        Command::new("jar")
            .args(["cfe"])
            .arg(&jar)
            .arg("Main")
            .arg("-C")
            .arg(dir.path())
            .arg("Main.class")
            .status()
            .unwrap()
            .success()
    );
    // The JAR deliberately contains no ArtifactPreparationMain. A successful
    // analysis therefore proves that the preparer was not invoked.
    let bytecode = JavaBytecodeInputs {
        application_class_dirs: vec![PathBuf::from("/classes one"), PathBuf::from("/classes-two")],
        test_class_dirs: vec!["/test classes".into()],
        classpath: vec!["/dependency/classes".into(), "/dependency.jar".into()],
    };
    let tests = vec![JavaTestCase {
        module_root: "/module".into(),
        class_name: "fixture.ServiceTest".into(),
        method_name: "works".into(),
        descriptor: "()V".into(),
        callable_signature: "works".into(),
        source_path: "/module/Test.java".into(),
    }];
    for provider_id in ["wala-java", "sootup-java"] {
        let root = PathBuf::from(format!("/{provider_id}"));
        let outcome = if provider_id == "wala-java" {
            WalaJavaProvider::with_timeout(&jar, Duration::from_secs(10))
                .analyze_prepared_test_roots(
                    &root,
                    &tests,
                    &["Lfixture/Service#changed/0".into()],
                    &bytecode,
                )
        } else {
            JavaAdapterProvider::new(provider_id, &jar, Duration::from_secs(10), "cha")
                .analyze_prepared_test_roots(
                    &root,
                    &tests,
                    &["Lfixture/Service#changed/0".into()],
                    &bytecode,
                )
        };
        assert_eq!(
            outcome.status,
            CallGraphStatus::Ok,
            "{:?}",
            outcome.diagnostics
        );
        let args = &outcome.diagnostics[0];
        assert!(
            args.contains("--classes-dir|/classes one|--classes-dir|/classes-two"),
            "{args}"
        );
        assert!(args.contains("--test-classes-dir|/test classes"));
        assert!(args.contains("/dependency/classes:/dependency.jar"));
        assert!(args.contains(
            "--entrypoint|fixture.ServiceTest#works()V|--target|Lfixture/Service#changed/0"
        ));
        let request = CallGraphRequest::new(root.to_string_lossy(), Language::Java);
        let ordinary = if provider_id == "wala-java" {
            WalaJavaProvider::new(&jar).analyze_prepared(&request, &bytecode)
        } else {
            JavaAdapterProvider::new(provider_id, &jar, Duration::from_secs(10), "cha")
                .analyze_prepared(&request, &bytecode)
        };
        assert_eq!(ordinary.status, CallGraphStatus::Ok);
    }
}

#[test]
fn merging_artifacts_keeps_dependency_directories_and_removes_application_duplicates() {
    let a = JavaBytecodeInputs {
        application_class_dirs: vec!["/project/lib/target/classes".into()],
        test_class_dirs: vec![],
        classpath: vec![
            "/project/lib/target/classes".into(),
            "/external/classes".into(),
            "/project/lib/build/libs/lib.jar".into(),
        ],
    };
    let result = JavaBytecodeInputs::merge(&[&a, &a], &[PathBuf::from("/project/lib")]);
    assert_eq!(
        result.application_class_dirs,
        [Path::new("/project/lib/target/classes")]
    );
    assert_eq!(result.classpath, [Path::new("/external/classes")]);
}
