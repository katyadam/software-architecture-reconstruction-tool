use std::collections::HashMap;

use crate::ast::{Expr, MethodAst, Stmt};

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

type Env = HashMap<String, SymVal>;

pub fn eval_method(method_name: &str, methods: &HashMap<String, MethodAst>) -> Env {
    let mut env: Env = HashMap::new();
    let method = methods.get(method_name).expect("method should be in map");
    for stmt in &method.body {
        eval_stmt(stmt, &mut env, &methods);
    }

    env
}

fn eval_stmt(stmt: &Stmt, env: &mut Env, methods: &HashMap<String, MethodAst>) {
    match stmt {
        Stmt::Assign { name, value } => {
            let evaluated = eval_expr(value, env, methods);
            env.insert(name.to_string(), evaluated);
        }
        _ => {}
    };
}

fn eval_expr(expr: &Expr, env: &mut Env, methods: &HashMap<String, MethodAst>) -> SymVal {
    match expr {
        Expr::Literal(s) => SymVal::Literal(s.clone()),
        Expr::Var(name) => env.get(name).cloned().unwrap_or(SymVal::Var(name.clone())),
        Expr::Concat(l, r) => {
            SymVal::new_concat(vec![eval_expr(l, env, methods), eval_expr(r, env, methods)])
        }
        Expr::Call { name, args } => {
            if let Some(method) = methods.get(name) {
                inline_method(method, args, env, methods)
            } else {
                SymVal::Empty
            }
        }
        Expr::Empty => SymVal::Empty,
    }
}

fn inline_method(
    method: &MethodAst,
    args: &[Expr],
    caller_env: &mut Env,
    methods: &HashMap<String, MethodAst>,
) -> SymVal {
    let mut local_env = Env::new();

    for (param, arg) in method.params.iter().zip(args) {
        local_env.insert(param.clone(), eval_expr(arg, caller_env, methods));
    }

    for stmt in &method.body {
        match stmt {
            Stmt::Assign { name, value } => {
                let v = eval_expr(value, &mut local_env, methods);
                local_env.insert(name.clone(), v);
            }
            Stmt::Return(expr) => {
                return eval_expr(expr, &mut local_env, methods);
            }
            _ => {}
        }
    }

    SymVal::Empty
}
