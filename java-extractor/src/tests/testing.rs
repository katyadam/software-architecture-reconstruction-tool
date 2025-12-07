use models::Entity;

use crate::{
    extraction::{
        entities::{self, extractor::EntitiesExtractor},
        extractor::Extractor,
    },
    utils::{get_tree, load_file},
};

#[test]
fn base_test() {
    let filename = "./examples/AllFieldClass.java";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let mut entities = EntitiesExtractor.extract(&code, &tree, &filename);
    println!("{:#?}", entities);
}
