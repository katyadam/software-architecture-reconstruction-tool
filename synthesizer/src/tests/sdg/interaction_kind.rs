#[cfg(test)]
mod tests {
    use crate::sdg::interaction_kind::InteractionKind;

    #[test]
    fn default_is_business() {
        assert_eq!(InteractionKind::default(), InteractionKind::Business);
    }

    #[test]
    fn ordering_encodes_precedence() {
        // TestOrigin > Reflexive > HealthInfra as a *precedence*, which as an
        // Ord means TestOrigin sorts first. Business sorts before all of them.
        assert!(InteractionKind::Business < InteractionKind::TestOrigin);
        assert!(InteractionKind::TestOrigin < InteractionKind::Reflexive);
        assert!(InteractionKind::Reflexive < InteractionKind::HealthInfra);
    }

    #[test]
    fn rollup_business_wins_any_tie() {
        let kinds = [InteractionKind::HealthInfra, InteractionKind::Business];
        assert_eq!(
            kinds.iter().copied().min(),
            Some(InteractionKind::Business),
            "one business request must keep the whole edge in the business view"
        );
    }

    #[test]
    fn rollup_of_only_non_business_picks_highest_precedence() {
        // The case the 2026-07-21 spec left undefined.
        let kinds = [InteractionKind::HealthInfra, InteractionKind::TestOrigin];
        assert_eq!(
            kinds.iter().copied().min(),
            Some(InteractionKind::TestOrigin)
        );
    }

    #[test]
    fn serialises_as_a_bare_name_and_defaults_when_absent() {
        let json = serde_json::to_string(&InteractionKind::HealthInfra)
            .expect("InteractionKind must serialise");
        assert_eq!(json, "\"HealthInfra\"");

        // A legacy sdg.json has no kind field at all.
        #[derive(serde::Deserialize)]
        struct Legacy {
            #[serde(default)]
            kind: InteractionKind,
        }
        let legacy: Legacy = serde_json::from_str("{}").expect("legacy JSON must load");
        assert_eq!(legacy.kind, InteractionKind::Business);
    }
}
