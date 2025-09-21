#[derive(Debug, Clone)]
pub struct Assignment {
    pub variable_name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Scope {
    Global,
    Function(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssignmentKey {
    pub scope: Scope,
    pub variable_name: String,
}
