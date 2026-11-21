//! Passes 1-3 with no LLM, as the baseline `cli` runs them.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use extractor_runtime::pipeline::pass3::{pass_attr, pass_module};
use extractor_runtime::pipeline::{
    build_project_ir, collect_files, collect_syntactic_records, evaluate,
};
use models::ir::evaluted::EvaluatedIR;

pub fn extract(project: &Path, constants: &HashMap<String, String>) -> Result<EvaluatedIR> {
    let records = collect_syntactic_records(collect_files(project)?, false)?;
    let ir = build_project_ir(records);
    let attrs = pass_attr::resolve_all(&ir, constants);
    let module_consts = pass_module::resolve_all(&ir, constants, &attrs);
    Ok(evaluate(ir, constants, &attrs, &module_consts))
}
