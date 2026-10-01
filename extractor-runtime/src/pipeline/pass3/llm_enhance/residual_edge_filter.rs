//! Gate for residual REST calls: resolved, empty, or needing resolution.

use crate::pipeline::pass3::restcalls::{EvalState, is_restcall_evaluated_enough};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(super) enum ResidualTriage {
    Resolved,
    Empty,
    NeedsResolution,
}

pub(super) fn triage(rc: &models::RestCall) -> ResidualTriage {
    match is_restcall_evaluated_enough(rc) {
        EvalState::ResolvedURL => ResidualTriage::Resolved,
        EvalState::Junk => ResidualTriage::Empty,
        EvalState::NeedsResolution => ResidualTriage::NeedsResolution,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::{RestCall, source_code::SourceSpan};

    fn restcall(target_uri: &str) -> RestCall {
        RestCall {
            function_name: "do_call".to_string(),
            function_hash: "h1".to_string(),
            call_arguments: vec![],
            http_method: Default::default(),
            target_uri: target_uri.to_string(),
            file_path: "/proj/caller/client.py".to_string(),
            source_span: SourceSpan::new(0, 0),
        }
    }

    #[test]
    fn triage_http_target_is_resolved() {
        let rc = restcall("http://medical-data-service:8000/x");
        assert_eq!(triage(&rc), ResidualTriage::Resolved);
    }

    #[test]
    fn triage_empty_target_is_empty() {
        assert_eq!(triage(&restcall("")), ResidualTriage::Empty);
    }

    #[test]
    fn triage_residual_needs_resolution() {
        let rc = restcall("self._mds_url + url");
        assert_eq!(triage(&rc), ResidualTriage::NeedsResolution);
    }
}
