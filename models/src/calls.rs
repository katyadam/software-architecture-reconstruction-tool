use crate::Argument;

#[derive(Debug, PartialEq, Eq)]
pub struct CallStatement {
    pub function_name: String,
    pub arguments: Vec<Argument>,
    pub enclosing_function_name: Option<String>,
    pub enclosing_class_name: Option<String>,
}
