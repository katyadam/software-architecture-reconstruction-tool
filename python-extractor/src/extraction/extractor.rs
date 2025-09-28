use tree_sitter::Tree;

pub struct ExtractParams<'a> {
    pub tree: &'a Tree,
    pub code: &'a str,
    pub file_name: Option<&'a str>,
    pub service_name: Option<&'a str>,
}

impl<'a> ExtractParams<'a> {
    pub fn new(tree: &'a Tree, code: &'a str) -> Self {
        Self {
            tree,
            code,
            file_name: None,
            service_name: None,
        }
    }

    pub fn file_name(mut self, file_name: &'a str) -> Self {
        self.file_name = Some(file_name);
        self
    }

    pub fn service_name(mut self, service_name: &'a str) -> Self {
        self.service_name = Some(service_name);
        self
    }
}

pub trait Extractor<T> {
    fn extract(&self, params: ExtractParams) -> Vec<T>;
}
