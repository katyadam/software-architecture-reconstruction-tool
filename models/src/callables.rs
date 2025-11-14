use std::fmt::{self, Display};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(ToSchema, Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
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

#[derive(ToSchema, Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
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

#[derive(ToSchema, Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct Callable {
    pub name: String,
    pub signature: String,
    pub namespace: Namespace,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<String>,
    pub is_async: bool,
    pub is_constructor: bool,
    pub hash: String,
    pub file_path: String,
}

impl Display for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Namespace::Class(c) => write!(f, "class::{c}"),
            Namespace::Module(m) => write!(f, "module::{m}"),
        }
    }
}

impl Display for Callable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let params = self
            .parameters
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        write!(
            f,
            "\nFILE: {}\nNAMESPACE: {}\nSIGN: {}\nHASH: {}",
            self.file_path, self.namespace, self.signature, self.hash,
        )
    }
}
