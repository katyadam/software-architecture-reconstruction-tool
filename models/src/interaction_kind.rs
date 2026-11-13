//! Which architectural view a service interaction belongs to.
//! Classification rules live in synthesizer's `interaction_kind` module.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// What kind of interaction this is -- i.e. which architectural view it belongs to.
///
/// **Declaration order is the specification.** It carries two rules at once:
///
/// 1. Per-request precedence when a request matches more than one non-business
///    rule: `TestOrigin` > `Reflexive` > `HealthInfra`. A probe defined inside a
///    test is first a test artifact; a self-probe is first a self-call.
/// 2. Connection rollup via `min()`: `Business` wins any tie, so one real
///    business request keeps the whole edge in the business view. This is the
///    RTS-safe direction.
///
/// Reordering these variants changes the `min()` rollup. Per-request
/// precedence is a second, independent encoding -- the order of the
/// early returns in synthesizer's `classify` -- and must be kept consistent
/// with this ordering by hand; reordering here does not itself change it.
#[derive(
    Debug, Serialize, Deserialize, ToSchema, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default,
)]
pub enum InteractionKind {
    /// A real cross-service business dependency. The only kind that is scored.
    #[default]
    Business,
    /// The call site lives in test code.
    TestOrigin,
    /// A self-call -- localhost or the caller's own configured host.
    Reflexive,
    /// A liveness or health probe.
    HealthInfra,
}

impl InteractionKind {
    /// Roll a connection's request kinds up to a single edge kind.
    /// `Business` wins any tie, so one real business request keeps the whole
    /// edge in the business view -- the RTS-safe direction.
    pub fn rollup(kinds: impl Iterator<Item = Self>) -> Self {
        kinds.min().unwrap_or_default()
    }
}
