use std::fmt::{self, Display};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Clone)]
pub struct Argument {
    pub assigned_variable: String,
    pub value: String,
    pub datatype: Option<String>,
}

#[derive(ToSchema, Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct CallStatement {
    pub function_name: String,
    pub arguments: Vec<Argument>,
    pub enclosing_function_name: Option<String>,
    pub enclosing_class_name: Option<String>,
    pub enclosing_function_hash: Option<String>,
    pub is_self_invoke: bool,
    pub is_super_invoke: bool,
    pub invoked_on: Option<String>,
}

impl Display for CallStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let args = self
            .arguments
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        let mut prefix = String::new();
        if let Some(cls) = &self.enclosing_class_name {
            prefix.push_str(cls);
        }
        if let Some(func) = &self.enclosing_function_name {
            if !prefix.is_empty() {
                prefix.push_str("::");
            }
            prefix.push_str(func);
        }

        if prefix.is_empty() {
            write!(f, "{}({})", self.function_name, args)
        } else {
            write!(
                f,
                "{} >>> {}({}) -- INVOKED ON: {}",
                prefix,
                self.function_name,
                args,
                self.invoked_on.clone().unwrap_or("".to_string())
            )
        }
    }
}

impl Display for Argument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.assigned_variable.is_empty() {
            write!(f, "{}", self.value)
        } else {
            write!(f, "{} = {}", self.assigned_variable, self.value)
        }
    }
}
