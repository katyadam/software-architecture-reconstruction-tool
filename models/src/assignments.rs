#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub variable_name: String,
    pub variable_type: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VariableAddress {
    pub microservice: String,
    pub file: String,
    pub key: AssignmentKey,
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
            (Some(function), _) => Scope::Function(function),
        }
    }

    pub fn from_enclosing_function(enclosing_function: Option<String>) -> Scope {
        enclosing_function
            .map(Scope::Function)
            .unwrap_or(Scope::Global)
    }

    pub fn from_enclosing_class(enclosing_class: Option<String>) -> Scope {
        enclosing_class.map(Scope::Class).unwrap_or(Scope::Global)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssignmentKey {
    pub scope: Scope,
    pub variable_name: String,
}
