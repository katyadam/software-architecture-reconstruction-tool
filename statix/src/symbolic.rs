use std::collections::HashMap;

use crate::{
    ast::{Expr, MethodAst, Stmt},
    error::EvalError,
};

#[derive(Debug, Clone)]
pub enum SymVal {
    Literal(String),
    Var(String),
    Concat(Vec<SymVal>),
    Empty,
}

impl SymVal {
    pub fn new_concat(parts: Vec<SymVal>) -> SymVal {
        let mut flat = Vec::new();
        for p in parts {
            match p {
                SymVal::Concat(inner) => flat.extend(inner),
                other => flat.push(other),
            }
        }
        SymVal::Concat(flat)
    }
}

type VarName = String;
type VarType = String;
type Env = HashMap<VarName, (VarType, SymVal)>;

pub fn eval_method(
    method_name: &str,
    methods: &HashMap<String, MethodAst>,
) -> Result<Env, EvalError> {
    let mut env: Env = HashMap::new();
    let method = methods.get(method_name).expect("method should be in map");
    for stmt in &method.body {
        eval_stmt(stmt, &mut env, &methods)?;
    }

    Ok(env)
}

fn eval_stmt(
    stmt: &Stmt,
    env: &mut Env,
    methods: &HashMap<String, MethodAst>,
) -> Result<(), EvalError> {
    match stmt {
        Stmt::Declaration {
            name,
            datatype,
            value,
        } => {
            let evaluated = eval_expr(value, env, methods)?;
            env.insert(name.to_string(), (datatype.to_string(), evaluated.1));
        }
        _ => {}
    };

    Ok(())
}

fn eval_expr(
    expr: &Expr,
    env: &mut Env,
    methods: &HashMap<String, MethodAst>,
) -> Result<(VarType, SymVal), EvalError> {
    match expr {
        Expr::Literal(lit) => Ok(("String".to_string(), SymVal::Literal(lit.clone()))),
        Expr::Var(name) => env.get(name).cloned().ok_or(EvalError::NonSenseEvaluation(
            "variable not found in environment".to_string(),
        )),
        Expr::Concat(l, r) => {
            let (left_type, left) = eval_expr(l, env, methods)?;
            let (right_type, right) = eval_expr(r, env, methods)?;

            Ok((
                decide_concat_datatype(&left_type, &right_type),
                SymVal::new_concat(vec![left, right]),
            ))
        }
        Expr::Call { name, args } => {
            if let Some(method) = methods.get(name) {
                inline_method(method, args, env, methods)
            } else {
                Ok(("void".to_string(), SymVal::Empty))
            }
        }
        Expr::Empty => Ok(("void".to_string(), SymVal::Empty)),
    }
}

fn inline_method(
    method: &MethodAst,
    args: &[Expr],
    caller_env: &mut Env,
    methods: &HashMap<String, MethodAst>,
) -> Result<(String, SymVal), EvalError> {
    let mut local_env = Env::new();

    for (param, arg) in method.params.iter().zip(args) {
        let evaluated_param = eval_expr(arg, caller_env, methods)?;
        local_env.insert(param.clone(), evaluated_param);
    }

    for stmt in &method.body {
        match stmt {
            Stmt::Declaration {
                name,
                datatype,
                value,
            } => {
                let evaluated_expr = eval_expr(value, &mut local_env, methods)?;
                local_env.insert(name.clone(), (datatype.to_string(), evaluated_expr.1));
            }
            Stmt::Return(expr) => {
                return eval_expr(expr, &mut local_env, methods);
            }
            _ => {}
        }
    }

    Ok(("void".to_string(), SymVal::Empty))
}

fn decide_concat_datatype(left: &str, right: &str) -> String {
    if left == "String" || right == "String" {
        return "String".to_string();
    }

    left.to_string()
}
