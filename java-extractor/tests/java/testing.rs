use java_extractor::extraction::{endpoints::extractor::EndpointsExtractor, extractor::Extractor};

use crate::java::utils::{get_tree, load_file};

#[test]
fn base_test_record() {
    let filename = "./examples/CallGraphController.java";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let mut endpoints = EndpointsExtractor.extract(&code, &tree, &filename);
    println!("{:#?}", endpoints);
}
