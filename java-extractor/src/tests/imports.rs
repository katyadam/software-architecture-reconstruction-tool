use models::Import;

use crate::{
    extraction::{extractor::Extractor, imports::extractor::ImportsExtractor},
    utils::{get_tree, load_file},
};

#[test]
fn all_types_of_imports_test() {
    let filename = s!("./examples/AllImportsClass.java");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);

    let imports = ImportsExtractor.extract(&code, &tree, &filename);

    let expected = [
        Import {
            orig_module: s!("java.util.List"),
            orig_name: s!("List"),
            module_alias: s!(""),
            name_alias: s!(""),
            codeword: s!("List"),
        },
        Import {
            orig_module: s!("java.io.File"),
            orig_name: s!("File"),
            module_alias: s!(""),
            name_alias: s!(""),
            codeword: s!("File"),
        },
        Import {
            orig_module: s!("java.util"),
            orig_name: s!("*"),
            module_alias: s!(""),
            name_alias: s!(""),
            codeword: s!("*"),
        },
        Import {
            orig_module: s!("java.time"),
            orig_name: s!("*"),
            module_alias: s!(""),
            name_alias: s!(""),
            codeword: s!("*"),
        },
        Import {
            orig_module: s!("java.lang.Math.PI"),
            orig_name: s!("PI"),
            module_alias: s!(""),
            name_alias: s!(""),
            codeword: s!("PI"),
        },
        Import {
            orig_module: s!("java.lang.Math.sqrt"),
            orig_name: s!("sqrt"),
            module_alias: s!(""),
            name_alias: s!(""),
            codeword: s!("sqrt"),
        },
        Import {
            orig_module: s!("java.lang.Math"),
            orig_name: s!("*"),
            module_alias: s!(""),
            name_alias: s!(""),
            codeword: s!("*"),
        },
        Import {
            orig_module: s!("java.util.Map.Entry"),
            orig_name: s!("Entry"),
            module_alias: s!(""),
            name_alias: s!(""),
            codeword: s!("Entry"),
        },
    ];

    assert_eq!(imports, expected);
}
