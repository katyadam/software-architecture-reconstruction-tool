use models::{Entity, Field};

use crate::{
    extraction::{
        entities::{self, extractor::EntitiesExtractor},
        extractor::Extractor,
    },
    s, strs,
    utils::{get_tree, load_file},
};

#[test]
fn base_test_record() {
    let filename = "./examples/AllFieldRecord.java";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let mut entities = EntitiesExtractor.extract(&code, &tree, &filename);
    println!("{:#?}", entities);
}
