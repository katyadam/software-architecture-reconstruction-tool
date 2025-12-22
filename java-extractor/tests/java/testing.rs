use java_extractor::extraction::{
    endpoints::extractor::EndpointsExtractor, extractor::Extractor,
    restcalls::extractor::RestCallsExtractor,
};

use crate::java::utils::{get_tree, load_file};

#[test]
fn base_test_record() {
    let filename = "./examples/CancelServiceImpl.java";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let mut restcalls = RestCallsExtractor.extract(&code, &tree, &filename);
    println!("{:#?}", restcalls);
}
