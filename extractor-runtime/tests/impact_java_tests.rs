use extractor_runtime::impact::discover_java_tests;
use std::{fs, path::Path};

#[test]
fn discovers_junit_entrypoints_and_preserves_overload_descriptors() {
    let module = tempfile::tempdir().expect("temporary module is created");
    write(
        module.path(),
        "src/test/java/example/JupiterTest.java",
        r#"
            package example;
            import org.junit.jupiter.api.Test;
            class JupiterTest {
              @Test void unit() {}
              @org.junit.jupiter.params.ParameterizedTest void parameterized(String name, int count) {}
              @org.junit.jupiter.api.RepeatedTest(2) void repeated() {}
              @org.junit.jupiter.api.TestFactory Object factory() { return null; }
              void helper() {}
            }
        "#,
    );
    write(
        module.path(),
        "src/test/java/example/LegacyTest.java",
        r#"
            package example;
            class LegacyTest {
              @org.junit.Test public void overloaded() {}
              @org.junit.Test public void overloaded(String value) {}
            }
        "#,
    );

    let tests = discover_java_tests(module.path()).expect("test methods are discovered");

    assert_eq!(tests.len(), 6);
    assert!(tests.iter().any(|test| {
        test.class_name == "Lexample/JupiterTest"
            && test.method_name == "parameterized"
            && test.descriptor == "(Ljava/lang/String;I)V"
    }));
    assert!(tests.iter().any(|test| {
        test.class_name == "Lexample/LegacyTest"
            && test.method_name == "overloaded"
            && test.descriptor == "()V"
    }));
    assert!(tests.iter().any(|test| {
        test.class_name == "Lexample/LegacyTest"
            && test.method_name == "overloaded"
            && test.descriptor == "(Ljava/lang/String;)V"
    }));
    assert!(tests.iter().all(|test| test.method_name != "helper"));
}

/// Writes one test fixture source file below the temporary Maven module.
fn write(module_root: &Path, relative_path: &str, content: &str) {
    let path = module_root.join(relative_path);
    fs::create_dir_all(path.parent().expect("fixture path has parent"))
        .expect("fixture directory is created");
    fs::write(path, content).expect("fixture source is written");
}
