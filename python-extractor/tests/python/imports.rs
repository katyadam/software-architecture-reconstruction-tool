use models::Import;
use python_extractor::{
    extraction::{
        extractor::{ExtractParams, Extractor},
        imports::extractor::ImportsExtractor,
    },
    utils::load_file,
};

use crate::python::utils::get_tree;

#[test]
fn abstract_imports() {
    let filename = "./examples/python/abstract-imports.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let imports = ImportsExtractor.extract(ExtractParams::new(&tree, &code));
    let expected = vec![
        Import::from_parts("os", "", "os", "", "os"),
        Import::from_parts("math.sqrt", "", "math.sqrt", "", "math.sqrt"),
        Import::from_parts("numpy", "", "np", "", "np"),
        Import::from_parts("mypackage.utils", "", "utils", "", "utils"),
    ];
    assert_eq!(imports, expected);
}

#[test]
fn concrete_imports() {
    let filename = "./examples/python/concrete-imports.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let imports = ImportsExtractor.extract(ExtractParams::new(&tree, &code));
    let expected = vec![
        Import::from_parts("math", "pi", "math", "pi", "pi"),
        Import::from_parts("collections", "deque", "collections", "deque", "deque"),
        Import::from_parts(
            "collections",
            "defaultdict",
            "collections",
            "defaultdict",
            "defaultdict",
        ),
        Import::from_parts("os.path", "join", "os.path", "path_join", "path_join"),
        Import::from_parts(
            "os.path",
            "dirname",
            "os.path",
            "dirname_alias",
            "dirname_alias",
        ),
        Import::from_parts(
            "package.module",
            "something",
            "package.module",
            "alias",
            "alias",
        ),
    ];

    assert_eq!(imports, expected);
}
