use std::fmt;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub datatype: Option<String>,
    pub initial_value: Option<String>,
}

impl fmt::Display for Parameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let to_print = match (&self.datatype, &self.initial_value) {
            (None, None) => format!("{}", self.name),
            (Some(datatype), None) => format!("{}:{}", self.name, datatype),
            (None, Some(initial_value)) => format!("{}={}", self.name, initial_value),
            (Some(datatype), Some(initial_value)) => {
                format!("{}:{}={}", self.name, datatype, initial_value)
            }
        };
        write!(f, "{}", to_print)
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Clone)]
pub struct Argument {
    pub assigned_variable: String,
    pub value: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Namespace {
    Class(String),
    Module(String),
}

impl Namespace {
    pub fn get_signature(&self) -> String {
        match self {
            Namespace::Class(value) => format!("class:{}", value),
            Namespace::Module(value) => format!("module:{}", value),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Callable {
    pub signature: String,
    pub namespace: Namespace,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<String>,
    pub is_async: bool,
    pub hash: String,
}
