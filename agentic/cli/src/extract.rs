//! Passes 1-3 with no LLM, as the baseline `cli` runs them. Keeps the
//! project and each call's operand bindings for the Jev tools.

use std::collections::HashMap;
use std::path::Path;

use agentic_models::{CodeElement, PointId};
use anyhow::Result;
use extractor_runtime::pipeline::pass3::{
    OperandBindings, evaluate_with_bindings, pass_attr, pass_module,
};
use extractor_runtime::pipeline::{build_project_ir, collect_files, collect_syntactic_records};
use models::RestCall;
use models::ir::evaluted::EvaluatedIR;
use models::ir::project::ProjectIR;

pub struct Extracted {
    pub project: ProjectIR,
    pub ir: EvaluatedIR,
    pub bindings: HashMap<PointId, OperandBindings>,
}

pub fn extract(project: &Path, constants: &HashMap<String, String>) -> Result<Extracted> {
    let records = collect_syntactic_records(collect_files(project)?, false)?;
    let project = build_project_ir(records);
    let attrs = pass_attr::resolve_all(&project, constants);
    let module_consts = pass_module::resolve_all(&project, constants, &attrs);
    let (ir, bindings) = evaluate_with_bindings(&project, constants, &attrs, &module_consts);
    let bindings = by_point(&ir.restcalls, bindings);
    Ok(Extracted {
        project,
        ir,
        bindings,
    })
}

/// `bindings[i]` belongs to `restcalls[i]`; empty ones are left out, equal ids keep the first.
fn by_point(
    restcalls: &[RestCall],
    bindings: Vec<OperandBindings>,
) -> HashMap<PointId, OperandBindings> {
    let mut out = HashMap::new();
    for (rc, b) in restcalls
        .iter()
        .zip(bindings)
        .filter(|(_, b)| !b.is_empty())
    {
        let id = CodeElement::RestCall(rc.clone()).point_id();
        out.entry(id).or_insert(b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_keyed_by_point_id() {
        let rc = |target: &str| RestCall {
            file_path: "a/client.py".into(),
            target_uri: target.into(),
            ..Default::default()
        };
        let calls = [rc("{x}/a"), rc("{y}/b")];
        let bound = |k: &str| OperandBindings::from([(k.to_string(), "settings.v".to_string())]);
        let map = by_point(&calls, vec![bound("x"), bound("y")]);
        let id = CodeElement::RestCall(calls[1].clone()).point_id();
        assert_eq!(map[&id], bound("y"));
    }
}
