#[derive(Clone, Debug)]
pub enum Expr {
    Literal(String),
    Var(String),
    Concat(Box<Expr>, Box<Expr>),
    Call { name: String, args: Vec<Expr> },
    Empty,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Assign { name: String, value: Expr },
    Return(Expr),
    Empty,
}

#[derive(Clone, Debug)]
pub struct MethodAst {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}
