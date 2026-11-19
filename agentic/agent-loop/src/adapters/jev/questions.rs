//! Builds one request's questions: `answer`, plus `enough` and `next`.

use std::collections::{BTreeMap, BTreeSet};

use clients::typesafe::question::{MAX_CHOICE_OPTIONS, Question};

use super::{ANSWER, ENOUGH, JevWording, NEXT, NONE, STOP};
use crate::adapters::{Answers, NONE_DESCRIPTION};
use crate::protocol::ToolOption;

/// `answer` always; `enough` and `next` only while tool options are offered.
/// `next` keeps the best `MAX_CHOICE_OPTIONS - 1` options, leaving room for `stop`.
pub(in crate::adapters) fn questions<A>(
    wording: &JevWording,
    answers: &Answers<A>,
    options: &[ToolOption],
) -> Result<BTreeMap<String, Question>, String> {
    let labels = answers.options.iter().map(|o| (&o.label, &o.description));
    let labels = distinct(labels, NONE, NONE_DESCRIPTION)?;
    let mut q = BTreeMap::from([(ANSWER.to_string(), choice(&wording.answer, labels)?)]);
    if options.is_empty() {
        return Ok(q);
    }
    let best = &options[..options.len().min(MAX_CHOICE_OPTIONS - 1)];
    let ids = best.iter().map(|o| (&o.id, &o.description));
    let next = distinct(ids, STOP, "Stop: no listed fact would help.")?;
    let enough = Question::Noul {
        instructions: wording.enough.clone(),
        criteria: Some(wording.enough_criteria.clone()),
    };
    q.insert(ENOUGH.to_string(), enough);
    q.insert(NEXT.to_string(), choice(&wording.next, next)?);
    Ok(q)
}

/// Choice entries plus the reserved one. Jev keys choices by id, so a
/// duplicate or reserved id would silently overwrite another entry.
fn distinct<'s>(
    entries: impl Iterator<Item = (&'s String, &'s String)>,
    reserved: &str,
    reserved_description: &str,
) -> Result<Vec<(String, String)>, String> {
    let mut seen = BTreeSet::from([reserved]);
    let mut out = vec![];
    for (id, description) in entries {
        if !seen.insert(id.as_str()) {
            return Err(format!("jev: choice id {id} is duplicate or reserved"));
        }
        out.push((id.clone(), description.clone()));
    }
    out.push((reserved.to_string(), reserved_description.to_string()));
    Ok(out)
}

fn choice(instructions: &str, options: Vec<(String, String)>) -> Result<Question, String> {
    Question::choice(instructions, options).map_err(|e| format!("jev: {e}"))
}
