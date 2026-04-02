use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use models::{
    ParsedCallable,
    ir::ast::{Expr, Stmt},
};

use crate::{error::EvalError, matcher::CallableMatcher, visitor::Visitor};

pub type VarName = String;
pub type VarType = Option<String>;
type Env = HashMap<VarName, (VarType, Expr)>;

#[derive(Debug)]
pub struct AnalysisResult {
    pub return_value: Expr,
    pub final_env: Env,
}

pub struct AnalysisContext<'a> {
    pub callables_map: &'a HashMap<String, ParsedCallable>,
    pub matcher: Arc<dyn CallableMatcher>,
}

impl<'a> AnalysisContext<'a> {
    pub fn new(
        callables: &'a HashMap<String, ParsedCallable>,
        matcher: Arc<dyn CallableMatcher>,
    ) -> Self {
        Self {
            callables_map: callables,
            matcher,
        }
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

    /// Merge variable changes from two branch evaluators back into `self`.
    ///
    /// When `sym_cond` is `Some`, joining is condition-aware (`join` is called, so
    /// `true`/`false` literals select a single branch while unknown conditions
    /// produce `Expr::Joined`). When `None` (try/catch), values always join.
    ///
    /// Three phases:
    /// 1. Update pre-existing variables that changed in either branch.
    /// 2. Bring variables first declared inside a branch into the parent env.
    /// 3. For variables declared in BOTH branches that did not exist before,
    ///    join differing values (phase 2 lets the first-branch value win otherwise).
    fn merge_branches(
        &mut self,
        branch_a: &Self,
        branch_b: &Self,
        sym_cond: Option<&Expr>,
    ) -> Result<(), EvalError> {
        // Phase 1: join differing values for variables that already existed.
        let pre_keys: Vec<String> = self.env.keys().cloned().collect();
        for key in &pre_keys {
            let (_, a_val) = branch_a.env.get(key).unwrap();
            let (_, b_val) = branch_b.env.get(key).unwrap();
            if a_val != b_val {
                let joined = if let Some(cond) = sym_cond {
                    self.join(cond, a_val, b_val)?.1
                } else {
                    Expr::Joined {
                        vals: vec![a_val.clone(), b_val.clone()],
                    }
                };
                self.env.get_mut(key).unwrap().1 = joined;
            }
        }

        // Phase 2: bring in variables first declared inside either branch.
        let pre_keys_set: HashSet<String> = pre_keys.into_iter().collect();
        self.merge_new_vars(branch_a);
        self.merge_new_vars(branch_b);

        // Phase 3: join variables declared in BOTH branches that were not pre-existing.
        // Collect first to avoid simultaneous borrows of branch_a.env and self.env.
        let new_both: Vec<(String, Option<String>, Expr, Expr)> = branch_a
            .env
            .iter()
            .filter(|(key, _)| !pre_keys_set.contains(*key))
            .filter_map(|(key, (dtype, a_val))| {
                branch_b
                    .env
                    .get(key)
                    .filter(|(_, b_val)| a_val != b_val)
                    .map(|(_, b_val)| (key.clone(), dtype.clone(), a_val.clone(), b_val.clone()))
            })
            .collect();

        for (key, dtype, a_val, b_val) in new_both {
            let joined = if let Some(cond) = sym_cond {
                self.join(cond, &a_val, &b_val)?.1
            } else {
                Expr::Joined {
                    vals: vec![a_val, b_val],
                }
            };
            self.env.insert(key, (dtype, joined));
        }

        Ok(())
    }

    pub fn eval_callable(
        callable_name: &str,
        ctx: &'a AnalysisContext<'a>,
    ) -> Result<AnalysisResult, EvalError> {
        let callable = ctx.callables_map.get(callable_name).ok_or_else(|| {
            EvalError::NonSenseEvaluation(format!("Method {} not found", callable_name))
        })?;

        let mut env = HashMap::new();
        for param in &callable.callable.parameters {
            let inserted_expr = if let Some(initial_value) = &param.initial_value {
                Expr::Literal(initial_value.to_string())
            } else {
                Expr::Var(param.name.clone())
            };
            env.insert(param.name.clone(), (param.datatype.clone(), inserted_expr));
        }

        let mut evaluator = Self { env, ctx };
        let result = evaluator.visit_statements(&callable.ast.statements)?;

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
                    Ok((None, then.clone()))
                } else if lit == "false" || lit == "False" {
                    Ok((None, els.clone()))
                } else {
                    Ok((
                        None,
                        Expr::Joined {
                            vals: vec![then.clone(), els.clone()],
                        },
                    ))
                }
            }
            _ => Ok((
                None,
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
            Expr::Empty => Ok((None, Expr::Empty)),
            Expr::Joined { vals } => Ok((None, Expr::Joined { vals: vals.clone() })),
        }
    }

    fn visit_literal(&mut self, lit: &str) -> Result<Self::Out, Self::Error> {
        Ok((Some("String".to_string()), Expr::Literal(lit.to_owned())))
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

        if concat_datatype == Some("String".to_string())
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

        let closest =
            self.ctx
                .matcher
                .find_closest_callable(self.ctx.callables_map, name, &arg_types);
        if let Some(m_name) = closest
            && let Some(parsed_callable) = self.ctx.callables_map.get(&m_name)
        {
            let mut local_evaluator = SymbolicEvaluator {
                env: HashMap::new(),
                ctx: self.ctx,
            };

            for (param, val) in parsed_callable
                .callable
                .parameters
                .iter()
                .zip(evaluated_args)
            {
                local_evaluator
                    .env
                    .insert(param.name.clone(), (param.datatype.clone(), val));
            }
            let result = local_evaluator.visit_statements(&parsed_callable.ast.statements)?;

            Ok((
                parsed_callable.callable.return_type.clone(),
                result.unwrap_or(Expr::Empty),
            ))
        } else {
            Ok((None, Expr::Empty))
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
        dtype: &Option<String>,
        value: &Expr,
    ) -> Result<(), EvalError> {
        let evaluated = self.visit_expr(value)?;
        self.env
            .insert(name.to_string(), (dtype.clone(), evaluated.1));
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

        self.merge_branches(&then_evaluator, &else_evaluator, Some(&sym_cond))?;

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

        // None = no condition, so merge_branches always produces Expr::Joined
        self.merge_branches(&try_evaluator, &catch_evaluator, None)?;

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

fn decide_concat_datatype(left: &Option<String>, right: &Option<String>) -> Option<String> {
    if *left == Some("String".to_string()) || *right == Some("String".to_string()) {
        return Some("String".to_string());
    }

    left.clone()
}
