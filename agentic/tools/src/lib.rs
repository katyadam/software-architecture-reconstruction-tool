//! Read-only tools for one initiator point: code facts from the extracted
//! project, and the receivers it could reach. No model, no writes.

mod receivers;

pub use receivers::{address_key, row};

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};

use agent_loop::{ToolOption, Tools};
use agentic_models::{CodeElement, ConnectionPoint, Side};
use extractor_runtime::pipeline::facts::{self, Explored, FactCtx, Subject};
use extractor_runtime::pipeline::pass3::OperandBindings;
use models::configuration::assign_service;
use models::{
    ConfigurationData,
    ir::project::{ProjectIR, TypedFileRecord},
};

/// What every tool reads; shared by all points of a run.
pub struct ToolCtx<'a> {
    pub project: &'a ProjectIR,
    pub config: &'a ConfigurationData,
    /// Constants file, as fed to evaluation.
    pub constants: &'a HashMap<String, String>,
    /// Receivers of every protocol.
    pub receivers: &'a [&'a ConnectionPoint],
}

pub struct PointTools<'a> {
    ctx: &'a ToolCtx<'a>,
    point: &'a ConnectionPoint,
    subject: Subject,
    origin: &'a TypedFileRecord,
    service_dir: &'a str,
    /// Opening a file widens the `open:` options; `Tools::run` takes `&self`.
    explored: RefCell<Explored>,
}

impl<'a> PointTools<'a> {
    /// `None` for a receiver, or when the point's file is not in the project.
    pub fn new(
        ctx: &'a ToolCtx<'a>,
        point: &'a ConnectionPoint,
        bindings: Option<&OperandBindings>,
    ) -> Option<Self> {
        if point.side != Side::Initiator {
            return None;
        }
        let subject = match &point.element {
            CodeElement::RestCall(rc) => {
                let none = OperandBindings::new();
                Subject::rest_call(rc, bindings.unwrap_or(&none), ctx.project)
            }
            CodeElement::Message(edge) => Subject::message(edge, ctx.project),
            CodeElement::Endpoint(_) => return None,
        };
        let origin = ctx
            .project
            .files
            .iter()
            .find(|f| f.file_path == subject.file)?;
        let service_dir = assign_service(&subject.file, &ctx.config.service_descriptions)
            .map_or("", |sd| sd.base_dir_path.as_str());
        Some(Self {
            ctx,
            point,
            explored: RefCell::new(Explored::new(&origin.file_path)),
            subject,
            origin,
            service_dir,
        })
    }

    fn facts(&self) -> FactCtx<'_> {
        FactCtx {
            project_ir: self.ctx.project,
            config: self.ctx.config,
            external_constants: self.ctx.constants,
            subject: &self.subject,
            origin: self.origin,
            service_dir: self.service_dir,
        }
    }

    fn code_options(&self) -> Vec<ToolOption> {
        let fctx = self.facts();
        facts::available(&fctx, &self.explored.borrow())
            .iter()
            .map(|a| {
                let (id, description) = a.option(&fctx);
                option(&id, description)
            })
            .collect()
    }

    fn other_options(&self) -> Vec<ToolOption> {
        let protocol = self.point.protocol;
        let mut out = vec![
            option(
                "similar",
                "Closest receivers in other services, same protocol, and same method for HTTP."
                    .to_string(),
            ),
            option(
                "imports",
                "Files this file imports, and files importing it.".to_string(),
            ),
        ];
        // Configured services with a receiver of this protocol, own included.
        let names: BTreeSet<&str> = self
            .ctx
            .config
            .service_descriptions
            .iter()
            .map(|sd| sd.name.as_str())
            .filter(|name| {
                self.ctx
                    .receivers
                    .iter()
                    .any(|r| r.protocol == protocol && r.service.0 == *name)
            })
            .collect();
        out.extend(names.into_iter().map(|name| {
            let text = format!("All {protocol:?} receivers of service `{name}`.");
            option(&format!("service:{name}"), text)
        }));
        out
    }

    /// Runs a code fact; a stale id finds nothing.
    fn code(&self, id: &str) -> String {
        let fctx = self.facts();
        let mut explored = self.explored.borrow_mut();
        let action = facts::available(&fctx, &explored)
            .into_iter()
            .find(|a| a.option(&fctx).0 == id);
        match action {
            Some(a) => facts::fetch(&fctx, &a, &mut explored).1,
            None => facts::render(vec![]),
        }
    }
}

/// `tool` is the id up to the first `:`, `args` the rest.
fn option(id: &str, description: String) -> ToolOption {
    let (tool, args) = id.split_once(':').unwrap_or((id, ""));
    ToolOption {
        id: id.to_string(),
        tool: tool.to_string(),
        args: args.to_string(),
        description,
    }
}

impl Tools for PointTools<'_> {
    /// Code facts, then `similar`, `imports`, and `service:` by name.
    fn available(&self, taken: &BTreeSet<String>) -> Vec<ToolOption> {
        let mut out = self.code_options();
        out.extend(self.other_options());
        out.retain(|o| !taken.contains(&o.id));
        out
    }

    fn run(&self, option: &ToolOption) -> String {
        let lines = match option.tool.as_str() {
            "similar" => receivers::similar(self.point, self.ctx.receivers),
            "service" => receivers::of_service(self.point, self.ctx.receivers, &option.args),
            "imports" => facts::imports_lines(&self.facts()),
            _ => return self.code(&option.id),
        };
        facts::render(lines)
    }
}

#[cfg(test)]
mod tests;
