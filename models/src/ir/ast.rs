// TODO: Add more valuetypes - Boolean, Number,... - Now Python takes "true" as correct boolean and Java takes "True" as correct boolean - both are wrong
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Literal(String),
    Var(String),
    Concat(Box<Expr>, Box<Expr>),
    StructLiteral {
        type_name: Option<String>,
        fields: Vec<(String, Expr)>,
    },
    Call {
        name: String,
        receiver: Option<Box<Expr>>,
        args: Vec<Expr>,
    },
    Empty,
    Joined {
        vals: Vec<Expr>,
    },
    /// Attribute access: `object.field` (e.g. `settings.as_url`).
    Attr {
        object: Box<Expr>,
        field: String,
    },
}

/// A reference from an outer callable to a nested callable defined in its body.
/// Used to propagate outer-scope variables (captured closure env) into the inner
/// callable during symbolic evaluation.
#[derive(Clone, Debug)]
pub struct NestedRef {
    /// Mangled key of the inner callable in the callables map.
    pub key: String,
    /// SHA-256 hex hash of the inner function's full text.
    /// Matches `RestCall::function_hash` so captured scopes can be looked up
    /// by hash rather than mangled name, avoiding key collisions between sibling
    /// inner functions that share a name but have different bodies.
    pub hash: String,
    /// Names of variables from the outer scope that the inner callable reads.
    pub captured: Vec<String>,
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
    /// Nested callables defined inside this callable's body.
    pub nested: Vec<NestedRef>,
}
