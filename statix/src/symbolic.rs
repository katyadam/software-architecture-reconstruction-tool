use std::collections::HashMap;

use crate::{
    ast::{Expr, MethodAst, Stmt},
    error::EvalError,
    method_match::find_closest_method,
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

pub type VarName = String;
pub type VarType = String;
type Env = HashMap<VarName, (VarType, SymVal)>;

fn update_env(env: &mut Env, name: &VarName, new_val: SymVal) -> Result<(), EvalError> {
    if let Some((_type, old_val)) = env.get_mut(name) {
        *old_val = new_val;
        Ok(())
    } else {
        Err(EvalError::NonSenseEvaluation(
            "variable that should be updated by assignment, was not declared before".to_string(),
        ))
    }
}

pub fn eval_method(
    method_name: &str,
    methods: &HashMap<String, MethodAst>,
) -> Result<Env, EvalError> {
    let mut env: Env = HashMap::new();
    let method = methods.get(method_name).expect("method should be in map");

    for param in method.params.iter() {
        env.insert(param.name.clone(), (param.datatype.clone(), SymVal::Empty));
    }
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
        Stmt::Assignment { name, value } => {
            let evaluated = eval_expr(value, env, methods)?;
            update_env(env, name, evaluated.1)?;
        }
        _ => (),
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
        Expr::Var(name) => env
            .get(name)
            .cloned()
            .ok_or(EvalError::NonSenseEvaluation(format!(
                "variable not found in environment {name}"
            ))),
        Expr::Concat(l, r) => {
            let (left_type, left) = eval_expr(l, env, methods)?;
            let (right_type, right) = eval_expr(r, env, methods)?;

            Ok((
                decide_concat_datatype(&left_type, &right_type),
                SymVal::new_concat(vec![left, right]),
            ))
        }
        Expr::Call { name, args } => {
            let param_types: Vec<VarType> = args
                .iter()
                .map(|p| eval_expr(p, env, methods).map(|(t, _v)| t))
                .collect::<Result<Vec<VarType>, EvalError>>()?;
            let closest = find_closest_method(methods, &name, &param_types);
            if let Some(method) = closest
                && let Some(method_ast) = methods.get(&method)
            {
                inline_method(method_ast, args, env, methods)
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
) -> Result<(VarType, SymVal), EvalError> {
    let mut local_env = Env::new();

    for (param, arg) in method.params.iter().zip(args) {
        let evaluated_param = eval_expr(arg, caller_env, methods)?;
        local_env.insert(param.name.clone(), evaluated_param);
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
