// TODO: Add more valuetypes - Boolean, Number,... - Now Python takes "true" as correct boolean and Java takes "True" as correct boolean - both are wrong
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Literal(String),
    Var(String),
    Concat(Box<Expr>, Box<Expr>),
    Call { name: String, args: Vec<Expr> },
    Empty,
    Joined { vals: Vec<Expr> },
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Declaration {
        name: String,
        dtype: Option<String>,
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
    TryCatch {
        try_branch: Vec<Stmt>,
        catch_branch: Vec<Stmt>,
    },
    Return(Expr),
    Empty,
}

#[derive(Clone, Debug)]
pub struct CallableAst {
    pub statements: Vec<Stmt>,
}
