//! What a fact lookup is about: one call site, of a REST call or a message edge.

use std::collections::BTreeSet;

use models::{Argument, MessageEdge, RestCall, callables::Namespace, ir::project::ProjectIR};

use super::define::symbols;
use super::site::{read_span, source_call, strip_args};
use crate::pipeline::pass3::OperandBindings;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Subject {
    pub file: String,
    pub function_hash: String,
    /// Enclosing callable's signature; option text for `callers`.
    pub enclosing_function: String,
    pub enclosing_class: Option<String>,
    /// Inferred type of the object the call is made on.
    pub receiver_type: Option<String>,
    /// Names worth `define:` and `constant:`.
    pub symbols: BTreeSet<String>,
    /// Text an import codeword may appear in; mentioned imports are offered first.
    pub mentions: Vec<String>,
}

impl Subject {
    pub fn rest_call(rc: &RestCall, bindings: &OperandBindings, ir: &ProjectIR) -> Self {
        let site = Site::find(&rc.file_path, &rc.function_hash, &rc.call_arguments, ir);
        let values = bindings.values().map(String::as_str);
        let mut mentions = vec![rc.target_uri.clone()];
        mentions.extend(site.snippet.clone());
        mentions.extend(bindings.values().cloned());
        site.subject(&rc.function_name, symbols(values), mentions)
    }

    /// Symbols come from the raw destination: `Queues.queueName` -> `queueName`.
    pub fn message(edge: &MessageEdge, ir: &ProjectIR) -> Self {
        let site = Site::find(
            &edge.file_path,
            &edge.function_hash,
            &edge.call_arguments,
            ir,
        );
        let mut mentions = vec![edge.destination.clone()];
        mentions.extend(site.snippet.clone());
        let symbols = symbols([edge.destination.as_str()]);
        site.subject(&edge.function_name, symbols, mentions)
    }
}

/// What the IR knows about one call site.
struct Site {
    file: String,
    function_hash: String,
    signature: Option<String>,
    class: Option<String>,
    receiver_type: Option<String>,
    snippet: Option<String>,
}

impl Site {
    fn find(file: &str, hash: &str, args: &[Argument], ir: &ProjectIR) -> Self {
        let callable = ir.enclosing_callable(file, hash).map(|c| &c.metadata);
        let parsed = ir
            .files
            .iter()
            .find(|f| f.file_path == file)
            .and_then(|f| source_call(hash, args, &f.call_statements));
        Self {
            file: file.to_string(),
            function_hash: hash.to_string(),
            signature: callable.map(|c| c.signature.clone()),
            class: callable.and_then(|c| match &c.namespace {
                Namespace::Class(name) => Some(name.clone()),
                Namespace::Module(_) => None,
            }),
            // As DDFA's `receiver`: only a dotted callee has a receiver.
            receiver_type: parsed
                .map(|c| &c.metadata)
                .filter(|c| strip_args(&c.function_name).contains('.'))
                .and_then(|c| c.invoked_on.clone()),
            snippet: parsed
                .and_then(|c| c.function_span.as_ref())
                .and_then(|span| read_span(file, span)),
        }
    }

    fn subject(self, name: &str, symbols: BTreeSet<String>, mentions: Vec<String>) -> Subject {
        Subject {
            file: self.file,
            function_hash: self.function_hash,
            enclosing_function: self.signature.unwrap_or_else(|| name.to_string()),
            enclosing_class: self.class,
            receiver_type: self.receiver_type,
            symbols,
            mentions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::facts::test_fixtures::world;
    use models::{CommunicationProtocol, MessageDestinationKind, MessageRole};

    #[test]
    fn message_symbols_come_from_destination() {
        let edge = MessageEdge {
            protocol: CommunicationProtocol::RabbitMq,
            role: MessageRole::Producer,
            destination_kind: MessageDestinationKind::Queue,
            destination: "Queues.queueName".to_string(),
            exchange: None,
            routing_key: None,
            queue: None,
            topic: None,
            handler: None,
            function_name: "send".to_string(),
            function_hash: "h7".to_string(),
            call_arguments: vec![],
            file_path: "/proj/caller/sender.py".to_string(),
        };
        let s = Subject::message(&edge, &world().ir);
        assert_eq!(s.symbols, BTreeSet::from(["queueName".to_string()]));
        assert_eq!(s.enclosing_function, "send");
        assert_eq!(s.mentions, vec!["Queues.queueName".to_string()]);
    }
}
