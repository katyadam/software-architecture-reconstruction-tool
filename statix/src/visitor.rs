use models::ir::ast::{Expr, Stmt};

use crate::error::EvalError;

pub trait Visitor {
    type Out;
    type Error;
    // Join operation
    fn join(&mut self, cond: &Expr, then: &Expr, els: &Expr) -> Result<Self::Out, Self::Error>;

    // Expression visits
    fn visit_expr(&mut self, expr: &Expr) -> Result<Self::Out, Self::Error>;
    fn visit_literal(&mut self, lit: &str) -> Result<Self::Out, Self::Error>;
    fn visit_var(&mut self, name: &str) -> Result<Self::Out, Self::Error>;
    fn visit_concat(&mut self, left: &Expr, right: &Expr) -> Result<Self::Out, Self::Error>;
    fn visit_call(&mut self, name: &str, args: &[Expr]) -> Result<Self::Out, Self::Error>;

    // Statement visits
    fn visit_statements(&mut self, stmts: &[Stmt]) -> Result<Option<Expr>, EvalError>;
    fn visit_stmt(&mut self, stmt: &Stmt) -> Result<(), EvalError>;
    fn visit_declaration(
        &mut self,
        name: &str,
        dtype: &Option<String>,
        value: &Expr,
    ) -> Result<(), EvalError>;
    fn visit_assignment(&mut self, name: &str, value: &Expr) -> Result<(), EvalError>;
    fn visit_if(
        &mut self,
        cond: &Expr,
        then_b: &[Stmt],
        else_b: &Option<Vec<Stmt>>,
    ) -> Result<Option<Expr>, EvalError>;
    fn visit_try_catch(
        &mut self,
        try_branch: &[Stmt],
        catch_branch: &[Stmt],
    ) -> Result<Option<Expr>, EvalError>;
}
