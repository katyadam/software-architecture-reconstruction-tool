use std::collections::HashMap;

use crate::{
    ast::{Expr, MethodAst, Stmt},
    error::EvalError,
    method_match::find_closest_method,
    visitor::Visitor,
};

pub type VarName = String;
pub type VarType = String;
type Env = HashMap<VarName, (VarType, Expr)>;

pub struct AnalysisResult {
    pub return_value: Expr,
    pub final_env: Env,
}

#[derive(Clone)]
pub struct SymbolicEvaluator<'a> {
    env: Env,
    methods: &'a HashMap<String, MethodAst>,
}

impl<'a> SymbolicEvaluator<'a> {
    pub fn new(env: Env, methods: &'a HashMap<String, MethodAst>) -> Self {
        Self { env, methods }
    }

    pub fn eval_method(
        method_name: &str,
        methods: &'a HashMap<String, MethodAst>,
    ) -> Result<AnalysisResult, EvalError> {
        let method = methods.get(method_name).ok_or_else(|| {
            EvalError::NonSenseEvaluation(format!("Method {} not found", method_name))
        })?;

        let mut env = HashMap::new();
        for param in &method.params {
            env.insert(
                param.name.clone(),
                (param.datatype.clone(), Expr::Var(param.name.clone())),
            );
        }

        let mut evaluator = Self::new(env, methods);
        let result = evaluator.visit_statements(&method.body)?;

        Ok(AnalysisResult {
            return_value: result.unwrap_or(Expr::Empty),
            final_env: evaluator.env,
        })
    }
}

impl<'a> Visitor for SymbolicEvaluator<'a> {
    type Out = (VarType, Expr);

    type Error = EvalError;

    fn join(&mut self, cond: &Expr, then: &Expr, els: &Expr) -> Result<Self::Out, Self::Error> {
        match cond {
            Expr::Literal(lit) => {
                if lit == "true" {
                    Ok(("any".to_owned(), then.clone()))
                } else if lit == "false" {
                    Ok(("any".to_owned(), els.clone()))
                } else {
                    Ok((
                        "any".to_owned(),
                        Expr::Joined {
                            vals: vec![then.clone(), els.clone()],
                        },
                    ))
                }
            }
            _ => Ok((
                "any".to_owned(),
                Expr::Joined {
                    vals: vec![then.clone(), els.clone()],
                },
            )),
        }
    }

    fn visit_expr(&mut self, expr: &Expr) -> Result<Self::Out, Self::Error> {
        match expr {
            Expr::Literal(lit) => self.visit_literal(lit),
            Expr::Var(name) => self.visit_var(name),
            Expr::Concat(left, right) => self.visit_concat(left, right),
            Expr::Call { name, args } => self.visit_call(name, args),
            Expr::Empty => Ok(("void".to_string(), Expr::Empty)),
            Expr::Joined { vals } => Ok(("any".to_string(), Expr::Joined { vals: vals.clone() })),
        }
    }

    fn visit_literal(&mut self, lit: &str) -> Result<Self::Out, Self::Error> {
        Ok(("String".to_string(), Expr::Literal(lit.to_owned())))
    }

    fn visit_var(&mut self, name: &str) -> Result<Self::Out, Self::Error> {
        self.env
            .get(name)
            .cloned()
            .ok_or(EvalError::NonSenseEvaluation(format!(
                "variable not found in environment {name}"
            )))
    }

    fn visit_concat(&mut self, left: &Expr, right: &Expr) -> Result<Self::Out, Self::Error> {
        let (left_type, left) = self.visit_expr(left)?;
        let (right_type, right) = self.visit_expr(right)?;

        let concat_datatype = decide_concat_datatype(&left_type, &right_type);

        if concat_datatype == "String" {
            if let (Expr::Literal(ls), Expr::Literal(rs)) = (&left, &right) {
                return Ok((concat_datatype, Expr::Literal(format!("{}{}", ls, rs))));
            }
        }

        Ok((
            concat_datatype,
            Expr::Concat(Box::new(left), Box::new(right)),
        ))
    }

    fn visit_call(&mut self, name: &str, args: &[Expr]) -> Result<Self::Out, Self::Error> {
        let mut evaluated_args = Vec::new();
        let mut arg_types = Vec::new();
        for arg in args {
            let (t, v) = self.visit_expr(arg)?;
            arg_types.push(t);
            evaluated_args.push(v);
        }

        let closest = find_closest_method(self.methods, name, &arg_types);
        if let Some(m_name) = closest
            && let Some(method_ast) = self.methods.get(&m_name)
        {
            let mut local_evaluator = SymbolicEvaluator {
                env: HashMap::new(),
                methods: self.methods,
            };

            for (param, val) in method_ast.params.iter().zip(evaluated_args) {
                local_evaluator
                    .env
                    .insert(param.name.clone(), (param.datatype.clone(), val));
            }
            let result = local_evaluator.visit_statements(&method_ast.body)?;

            Ok((
                method_ast.return_type.clone(),
                result.unwrap_or(Expr::Empty),
            ))
        } else {
            Ok(("void".to_string(), Expr::Empty))
        }
    }

    fn visit_statements(&mut self, stmts: &[Stmt]) -> Result<Option<Expr>, EvalError> {
        for stmt in stmts {
            match stmt {
                Stmt::Return(e) => {
                    let (_, val) = self.visit_expr(e)?;
                    return Ok(Some(val));
                }
                Stmt::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    if let Some(ret) = self.visit_if(condition, then_branch, else_branch)? {
                        return Ok(Some(ret));
                    }
                }
                _ => self.visit_stmt(stmt)?,
            }
        }
        Ok(None)
    }

    fn visit_stmt(&mut self, stmt: &Stmt) -> Result<(), EvalError> {
        match stmt {
            Stmt::Declaration { name, dtype, value } => self.visit_declaration(name, dtype, value),
            Stmt::Assignment { name, value } => self.visit_assignment(name, value),
            _ => Ok(()),
        }
    }

    fn visit_declaration(
        &mut self,
        name: &str,
        dtype: &str,
        value: &Expr,
    ) -> Result<(), EvalError> {
        let evaluated = self.visit_expr(value)?;
        self.env
            .insert(name.to_string(), (dtype.to_string(), evaluated.1));
        Ok(())
    }

    fn visit_assignment(&mut self, name: &str, value: &Expr) -> Result<(), EvalError> {
        let evaluated = self.visit_expr(value)?;
        update_env(&mut self.env, name, evaluated.1)?;
        Ok(())
    }

    fn visit_if(
        &mut self,
        cond: &Expr,
        then_b: &[Stmt],
        else_b: &Option<Vec<Stmt>>,
    ) -> Result<Option<Expr>, EvalError> {
        let (_, sym_cond) = self.visit_expr(cond)?;

        let mut then_evaluator = self.clone();
        let mut else_evaluator = self.clone();

        let then_ret = then_evaluator.visit_statements(then_b)?;

        let else_ret = if let Some(stmts) = else_b {
            else_evaluator.visit_statements(stmts)?
        } else {
            None
        };

        let keys: Vec<String> = self.env.keys().cloned().collect();
        for key in keys {
            let (_, t_val) = then_evaluator.env.get(&key).unwrap();
            let (_, e_val) = else_evaluator.env.get(&key).unwrap();

            if t_val != e_val {
                let (_, evaluated_ite) = self.join(&sym_cond, t_val, e_val)?;
                self.env.get_mut(&key).unwrap().1 = evaluated_ite;
            }
        }

        // 5. JOIN Return Values: If branches return, the IF block returns symbolically
        if then_ret.is_some() || else_ret.is_some() {
            let (_, evaluated_ite) = self.join(
                &sym_cond,
                &then_ret.unwrap_or(Expr::Empty),
                &else_ret.unwrap_or(Expr::Empty),
            )?;
            return Ok(Some(evaluated_ite));
        }

        Ok(None)
    }
}

fn update_env(env: &mut Env, name: &str, new_val: Expr) -> Result<(), EvalError> {
    if let Some((_type, old_val)) = env.get_mut(name) {
        *old_val = new_val;
        Ok(())
    } else {
        Err(EvalError::NonSenseEvaluation(
            "variable that should be updated by assignment, was not declared before".to_string(),
        ))
    }
}

fn decide_concat_datatype(left: &str, right: &str) -> String {
    if left == "String" || right == "String" {
        return "String".to_string();
    }

    left.to_string()
}
