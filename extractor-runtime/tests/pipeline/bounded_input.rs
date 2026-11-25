//! Crafted input must not crash or stall extraction (#70).

use extractor_runtime::pipeline::{dispatch_syntactic, pass1::MAX_SOURCE_BYTES};

type Shape = (&'static str, fn(usize) -> String);

/// Nesting shapes that overflowed the stack below 1000 levels.
const SHAPES: [Shape; 5] = [
    ("a.go", |n| {
        format!(
            "package main\nfunc f() int {{ return {}1{} }}\n",
            "(".repeat(n),
            ")".repeat(n)
        )
    }),
    ("a.py", |n| {
        format!("x = {}1{}\n", "(".repeat(n), ")".repeat(n))
    }),
    ("b.py", |n| {
        let mut s = String::new();
        for i in 0..n {
            s += &format!("{}if x:\n", " ".repeat(i));
        }
        s + &" ".repeat(n) + "pass\n"
    }),
    ("A.java", |n| {
        format!(
            "class A {{ int f() {{ return {}1{}; }} }}\n",
            "(".repeat(n),
            ")".repeat(n)
        )
    }),
    ("B.java", |n| {
        format!(
            "class B {{ int f() {{ return 1{}; }} }}\n",
            " + 1".repeat(n)
        )
    }),
];

/// Runs extraction on a fresh thread with the default 2 MiB stack. Java and
/// Python walk on rayon pool threads, which also default to 2 MiB.
fn extract(name: &'static str, source: String) -> Result<bool, String> {
    std::thread::spawn(move || {
        dispatch_syntactic(&source, name)
            .map(|record| record.is_some())
            .map_err(|e| e.to_string())
    })
    .join()
    .expect("extraction thread panicked")
}

#[test]
fn deeply_nested_source_is_rejected_without_crashing() {
    for (name, source) in SHAPES {
        let result = extract(name, source(1000));
        assert!(
            result.is_err(),
            "{name}: expected depth error, got {result:?}"
        );
    }
}

#[test]
fn source_just_under_depth_cap_is_extracted() {
    for (name, source) in SHAPES {
        // Rejection is monotonic in nesting, so binary-search the deepest
        // accepted input; every probe below the cap must not crash either.
        let (mut ok, mut rejected) = (1, 1000);
        while rejected - ok > 1 {
            let mid = (ok + rejected) / 2;
            match extract(name, source(mid)) {
                Ok(_) => ok = mid,
                Err(_) => rejected = mid,
            }
        }
        assert_eq!(extract(name, source(ok)), Ok(true), "{name}");
    }
}

#[test]
fn oversized_file_is_skipped() {
    let source = format!("x = 1\n{}", "#".repeat(MAX_SOURCE_BYTES));
    assert!(matches!(dispatch_syntactic(&source, "big.py"), Ok(None)));
}
