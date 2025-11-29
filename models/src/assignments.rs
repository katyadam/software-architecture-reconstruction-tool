#[derive(Debug, Clone)]
pub struct Assignment {
    pub variable_name: String,
    pub variable_type: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Scope {
    Global,
    Function(String),
    Class(String),
}

impl Scope {
    pub fn from_enclosings(
        enclosing_function: Option<String>,
        enclosing_class: Option<String>,
    ) -> Scope {
        match (enclosing_function, enclosing_class) {
            (None, None) => Scope::Global,
            (None, Some(class)) => Scope::Class(class),
            (Some(function), None) => Scope::Function(function),
            (Some(function), Some(_)) => Scope::Function(function),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssignmentKey {
    pub scope: Scope,
    pub variable_name: String,
}
