//! Maps one System One reply to a step.

use clients::typesafe::response::{Answer as JevAnswer, SystemOneResponse};

use super::{ANSWER, ENOUGH, JevLimits, NEXT, NONE, STOP};
use crate::adapters::Answers;
use crate::protocol::{Answer, Step};

/// Calls `next` unless no options were offered, `enough` passed, or `next`
/// is `stop` or missing; then answers.
pub(in crate::adapters) fn step_from<A: Clone>(
    response: &SystemOneResponse,
    answers: &Answers<A>,
    limits: JevLimits,
    offered: bool,
) -> Result<Step<A>, String> {
    match next_call(response, limits, offered) {
        Some(id) => Ok(Step::Call(id)),
        None => final_answer(response, answers, limits).map(Step::Final),
    }
}

fn next_call(response: &SystemOneResponse, limits: JevLimits, offered: bool) -> Option<String> {
    let enough = matches!(
        response.answers.get(ENOUGH),
        Some(JevAnswer::Noul { noul }) if *noul >= limits.enough
    );
    if !offered || enough {
        return None;
    }
    match response.answers.get(NEXT) {
        Some(JevAnswer::Choice { choice, .. }) if choice != STOP => Some(choice.clone()),
        _ => None,
    }
}

/// The picked label's value with its probability. Below the threshold:
/// `none`, with the weak pick kept in the note for later tuning.
fn final_answer<A: Clone>(
    response: &SystemOneResponse,
    answers: &Answers<A>,
    limits: JevLimits,
) -> Result<Answer<A>, String> {
    let Some(JevAnswer::Choice {
        choice,
        probabilities,
        ..
    }) = response.answers.get(ANSWER)
    else {
        return Err("jev: reply has no answer choice".to_string());
    };
    let none = |note| Answer {
        value: answers.none.clone(),
        confidence: None,
        note,
    };
    if choice == NONE {
        return Ok(none(None));
    }
    let value = answers
        .value_of(choice)
        .ok_or_else(|| format!("jev: answer {choice} was not offered"))?;
    let p = probabilities.get(choice).copied().unwrap_or(0.0);
    if p < limits.answer_min {
        let min = limits.answer_min;
        return Ok(none(Some(format!("below {min}: {choice} {p}"))));
    }
    Ok(Answer {
        value,
        confidence: Some(p),
        note: None,
    })
}
