#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Literal(String),
    Var(String),
    Concat(Box<Expr>, Box<Expr>),
    Call {
        name: String,
        args: Vec<Expr>,
    },
    Empty,
    ITE {
        cond: Box<Expr>,
        then_val: Box<Expr>,
        else_val: Box<Expr>,
    },
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Declaration {
        name: String,
        dtype: String,
        value: Expr,
    },
    Assignment {
        name: String,
        value: Expr,
    },
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
    },
    Return(Expr),
    Empty,
}

#[derive(Clone, Debug)]
pub struct Parameter {
    pub name: String,
    pub datatype: String,
}

impl Parameter {
    pub fn new(name: String, datatype: String) -> Self {
        Self { name, datatype }
    }
}

#[derive(Clone, Debug)]
pub struct MethodAst {
    pub return_type: String,
    pub header: String,
    pub params: Vec<Parameter>,
    pub body: Vec<Stmt>,
}
