use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{
    ast::{CallableAst, Expr, Stmt},
    error::EvalError,
    matcher::CallableMatcher,
    visitor::Visitor,
};

pub type VarName = String;
pub type VarType = String;
type Env = HashMap<VarName, (VarType, Expr)>;

#[derive(Debug)]
pub struct AnalysisResult {
    pub return_value: Expr,
    pub final_env: Env,
}

pub struct AnalysisContext<'a> {
    pub callables: &'a HashMap<String, CallableAst>,
    pub matcher: Arc<dyn CallableMatcher>,
}

impl<'a> AnalysisContext<'a> {
    pub fn new(
        callables: &'a HashMap<String, CallableAst>,
        matcher: Arc<dyn CallableMatcher>,
    ) -> Self {
        Self { callables, matcher }
    }
}

pub struct SymbolicEvaluator<'a> {
    pub env: Env,
    pub ctx: &'a AnalysisContext<'a>,
}

// TODO: Should also take class fields to environment!
// When trying to access variable that is from class field, then getting EvalError::NonSenseEvaluation(format!("variable: {name} -- not found in environment"))
// Or getting the same type of error when assigning to a class field variable

impl<'a> SymbolicEvaluator<'a> {
    pub fn new(env: Env, ctx: &'a AnalysisContext<'a>) -> Self {
        Self { env, ctx }
    }

    pub fn branch(&self) -> Self {
        Self {
            env: self.env.clone(),
            ctx: self.ctx,
        }
    }

    fn merge_new_vars(&mut self, branch_evaluator: &Self) {
        for (key, (dtype, val)) in &branch_evaluator.env {
            if !self.env.contains_key(key) {
                self.env.insert(key.clone(), (dtype.clone(), val.clone()));
            }
        }
    }

    pub fn eval_callable(
        callable_name: &str,
        ctx: &'a AnalysisContext<'a>,
    ) -> Result<AnalysisResult, EvalError> {
        let callable = ctx.callables.get(callable_name).ok_or_else(|| {
            EvalError::NonSenseEvaluation(format!("Method {} not found", callable_name))
        })?;

        let mut env = HashMap::new();
        for param in &callable.params {
            let inserted_expr = if let Some(default_value) = &param.default_value {
                Expr::Literal(default_value.to_string())
            } else {
                Expr::Var(param.name.clone())
            };
            env.insert(param.name.clone(), (param.datatype.clone(), inserted_expr));
        }

        let mut evaluator = Self { env, ctx };
        let result = evaluator.visit_statements(&callable.body)?;

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
                if lit == "true" || lit == "True" {
                    Ok(("any".to_owned(), then.clone()))
                } else if lit == "false" || lit == "False" {
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
                "variable: {name} not found in environment"
            )))
    }

    fn visit_concat(&mut self, left: &Expr, right: &Expr) -> Result<Self::Out, Self::Error> {
        let (left_type, left) = self.visit_expr(left)?;
        let (right_type, right) = self.visit_expr(right)?;

        let concat_datatype = decide_concat_datatype(&left_type, &right_type);

        if concat_datatype == "String"
            && let (Expr::Literal(ls), Expr::Literal(rs)) = (&left, &right)
        {
            return Ok((concat_datatype, Expr::Literal(format!("{}{}", ls, rs))));
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

        let closest = self
            .ctx
            .matcher
            .find_closest_callable(self.ctx.callables, name, &arg_types);
        if let Some(m_name) = closest
            && let Some(callable_ast) = self.ctx.callables.get(&m_name)
        {
            let mut local_evaluator = SymbolicEvaluator {
                env: HashMap::new(),
                ctx: self.ctx,
            };

            for (param, val) in callable_ast.params.iter().zip(evaluated_args) {
                local_evaluator
                    .env
                    .insert(param.name.clone(), (param.datatype.clone(), val));
            }
            let result = local_evaluator.visit_statements(&callable_ast.body)?;

            Ok((
                callable_ast.return_type.clone(),
                result.unwrap_or(Expr::Empty),
            ))
        } else {
            Ok(("void".to_string(), Expr::Empty))
        }
    }

    fn visit_statements(&mut self, stmts: &[Stmt]) -> Result<Option<Expr>, EvalError> {
        let mut collected_returns = Vec::new();
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
                        collected_returns.push(ret);
                    }
                }
                Stmt::TryCatch {
                    try_branch,
                    catch_branch,
                } => {
                    if let Some(ret) = self.visit_try_catch(try_branch, catch_branch)? {
                        collected_returns.push(ret);
                    }
                }
                _ => self.visit_stmt(stmt)?,
            }
        }

        match collected_returns.len() {
            0 => Ok(None),
            1 => Ok(collected_returns.into_iter().next()),
            _ => Ok(Some(Expr::Joined {
                vals: collected_returns,
            })),
        }
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

        let mut then_evaluator = self.branch();
        let mut else_evaluator = self.branch();

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

        // Snapshot which keys existed before the if so we can find branch-only vars.
        let pre_if_keys: HashSet<String> = self.env.keys().cloned().collect();

        self.merge_new_vars(&then_evaluator);
        self.merge_new_vars(&else_evaluator);

        // Join variables that were declared in BOTH branches but did not exist in
        // the parent env before the if (e.g. variables first declared inside
        // try/except or parallel if-branches without a pre-declaration).
        for (key, (dtype, t_val)) in &then_evaluator.env {
            if pre_if_keys.contains(key) {
                continue; // already handled by the per-key join loop above
            }
            if let Some((_, e_val)) = else_evaluator.env.get(key)
                && t_val != e_val
            {
                let (_, joined) = self.join(&sym_cond, t_val, e_val)?;
                self.env.insert(key.clone(), (dtype.clone(), joined));
            }
        }

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

    fn visit_try_catch(
        &mut self,
        try_branch: &[Stmt],
        catch_branch: &[Stmt],
    ) -> Result<Option<Expr>, EvalError> {
        let mut try_evaluator = self.branch();
        let mut catch_evaluator = self.branch();

        let try_ret = try_evaluator.visit_statements(try_branch)?;
        let catch_ret = catch_evaluator.visit_statements(catch_branch)?;

        // Merge variables from both branches into the parent env, joining values
        // that differ (same logic as visit_if but with no condition — always joins).
        let keys: Vec<String> = self.env.keys().cloned().collect();
        for key in keys {
            let (_, t_val) = try_evaluator.env.get(&key).unwrap();
            let (_, c_val) = catch_evaluator.env.get(&key).unwrap();
            if t_val != c_val {
                let joined = Expr::Joined {
                    vals: vec![t_val.clone(), c_val.clone()],
                };
                self.env.get_mut(&key).unwrap().1 = joined;
            }
        }

        let pre_keys: std::collections::HashSet<String> = self.env.keys().cloned().collect();
        self.merge_new_vars(&try_evaluator);
        self.merge_new_vars(&catch_evaluator);

        // Join variables declared in BOTH branches that did not exist before the try.
        for (key, (dtype, t_val)) in &try_evaluator.env {
            if pre_keys.contains(key) {
                continue;
            }
            if let Some((_, c_val)) = catch_evaluator.env.get(key)
                && t_val != c_val
            {
                let joined = Expr::Joined {
                    vals: vec![t_val.clone(), c_val.clone()],
                };
                self.env.insert(key.clone(), (dtype.clone(), joined));
            }
        }

        match (try_ret, catch_ret) {
            (None, None) => Ok(None),
            (Some(t), None) => Ok(Some(t)),
            (None, Some(c)) => Ok(Some(c)),
            (Some(t), Some(c)) => {
                if t == c {
                    Ok(Some(t))
                } else {
                    Ok(Some(Expr::Joined { vals: vec![t, c] }))
                }
            }
        }
    }
}

fn update_env(env: &mut Env, name: &str, new_val: Expr) -> Result<(), EvalError> {
    if let Some((_type, old_val)) = env.get_mut(name) {
        *old_val = new_val;
        Ok(())
    } else {
        Err(EvalError::NonSenseEvaluation(format!(
            "variable: {name} that should be updated to: {new_val:#?} by assignment, was not declared before -- env: {env:#?}"
        )))
    }
}

fn decide_concat_datatype(left: &str, right: &str) -> String {
    if left == "String" || right == "String" {
        return "String".to_string();
    }

    left.to_string()
}
