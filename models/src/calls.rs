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
    /// True when this call sits inside a decorator/annotation rather than
    /// executable code -- e.g. Python's `@app.get("/items")`. Such calls are
    /// route declarations, not outbound calls.
    ///
    /// `#[serde(default)]`: IMCG chunks persisted to MinIO before this field
    /// existed lack the key entirely. Without a default, deserializing those
    /// old blobs fails with "missing field is_decorator". Keep this even if
    /// the field looks unused elsewhere -- it is consumed entirely inside
    /// Pass 2 and never read downstream, but old blobs still need to load.
    #[serde(default)]
    pub is_decorator: bool,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// IMCG chunks persisted to MinIO before `is_decorator` existed lack the
    /// key entirely. `#[serde(default)]` must let those old blobs still
    /// deserialize, defaulting the missing field to `false`.
    #[test]
    fn call_statement_deserializes_without_is_decorator_field() {
        let json = r#"{
            "function_name": "get",
            "arguments": [],
            "enclosing_function_name": null,
            "enclosing_class_name": null,
            "enclosing_function_hash": null,
            "is_self_invoke": false,
            "is_super_invoke": false,
            "invoked_on": null
        }"#;

        let call: CallStatement =
            serde_json::from_str(json).expect("missing is_decorator must not fail deserialization");
        assert!(!call.is_decorator);
    }
}
