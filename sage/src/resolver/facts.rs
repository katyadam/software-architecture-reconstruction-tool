use std::collections::HashMap;

use models::ir::language::Framework;

use crate::resolver::{code::CodeSnippet, messages::Message};

pub struct FactBundle {
    pub sites: Vec<CodeSnippet>,
    pub frameworks: Vec<Framework>,
    pub scraped_variables: HashMap<String, String>,
    pub others: Vec<Message>,
}
