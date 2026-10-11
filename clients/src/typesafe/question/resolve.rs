use std::collections::BTreeMap;

use crate::{
    error::TypeSafeError,
    typesafe::question::{NoulCriteria, Question},
};

/// Answer key of the target-service Choice.
pub const TARGET: &str = "target";
/// Answer key of the "facts are enough" Noul.
pub const ENOUGH: &str = "enough";
/// Answer key of the next-action Choice.
pub const NEXT: &str = "next";
/// `target` option: no listed service.
pub const NONE: &str = "none";
/// `next` option: stop acquiring.
pub const STOP: &str = "stop";

/// `(id, description)` of one Choice option.
pub type Opt = (String, String);

/// Candidate services + `none`.
pub fn target(services: &[Opt]) -> Result<Question, TypeSafeError> {
    let none = (
        NONE.to_string(),
        "None of these: external API, or the facts do not show it.".to_string(),
    );

    Question::choice(
        "Which service does this call send its request to?",
        services.iter().cloned().chain([none]),
    )
}

pub fn enough() -> Question {
    Question::Noul {
        instructions: "Do the call and its facts identify the service the URL points to?"
            .to_string(),
        criteria: Some(NoulCriteria {
            yes: "The URL's base (host, setting, constant or env var) is traced to one service."
                .to_string(),
            no: "The URL's base is still unknown; another fact is needed.".to_string(),
        }),
    }
}

/// Available actions + `stop`.
pub fn next(actions: &[Opt]) -> Result<Question, TypeSafeError> {
    let stop = (
        STOP.to_string(),
        "Stop: no listed fact would help.".to_string(),
    );
    Question::choice(
        "Which fact would best reveal where the URL's base comes from?",
        actions.iter().cloned().chain([stop]),
    )
}

/// One acquisition step; `enough` / `next` only when actions remain.
pub fn acquire_step(
    services: &[Opt],
    actions: &[Opt],
) -> Result<BTreeMap<String, Question>, TypeSafeError> {
    let mut questions = BTreeMap::from([(TARGET.to_string(), target(services)?)]);
    if !actions.is_empty() {
        questions.insert(ENOUGH.to_string(), enough());
        questions.insert(NEXT.to_string(), next(actions)?);
    }
    Ok(questions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typesafe::question::MAX_CHOICE_OPTIONS;

    fn opts(n: usize) -> Vec<Opt> {
        (0..n)
            .map(|i| (format!("o{i}"), format!("option {i}")))
            .collect()
    }

    #[test]
    fn acquire_step_asks_all_three() {
        let q = acquire_step(&opts(2), &opts(3)).expect("fits");
        assert_eq!(
            q.keys().map(String::as_str).collect::<Vec<_>>(),
            vec![ENOUGH, NEXT, TARGET]
        );
        let Question::Choice { criteria, .. } = &q[TARGET] else {
            panic!("target is a choice")
        };
        assert_eq!(criteria.len(), 3);
        assert!(criteria.contains_key(NONE));
        let Question::Choice { criteria, .. } = &q[NEXT] else {
            panic!("next is a choice")
        };
        assert_eq!(criteria.len(), 4);
        assert!(criteria.contains_key(STOP));
        assert!(matches!(q[ENOUGH], Question::Noul { .. }));
    }

    #[test]
    fn no_actions_asks_target_only() {
        let q = acquire_step(&opts(2), &[]).expect("fits");
        assert_eq!(
            q.keys().map(String::as_str).collect::<Vec<_>>(),
            vec![TARGET]
        );
        assert_eq!(q[TARGET], target(&opts(2)).expect("fits"));
    }

    #[test]
    fn too_many_options_error_instead_of_dropping() {
        // 254 + `none` / `stop` = 255 fits; one more must not be silently dropped.
        assert!(target(&opts(MAX_CHOICE_OPTIONS - 1)).is_ok());
        assert!(next(&opts(MAX_CHOICE_OPTIONS - 1)).is_ok());
        assert!(matches!(
            target(&opts(MAX_CHOICE_OPTIONS)),
            Err(TypeSafeError::TooManyOptions { got: 256 })
        ));
        assert!(matches!(
            next(&opts(MAX_CHOICE_OPTIONS)),
            Err(TypeSafeError::TooManyOptions { got: 256 })
        ));
    }
}
