//! Classification of a single service interaction into an architectural view.
//!
//! The SDG feeds change-impact analysis and regression test selection, where a
//! missing edge is unsafe and a spurious edge is merely costly. So an edge that
//! does not belong in the *business* view is never deleted -- it is tagged and
//! excluded from business scoring while remaining in the graph.

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
/// Reordering these variants silently changes both rules.
#[derive(
    Debug, Serialize, Deserialize, ToSchema, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default,
)]
pub enum InteractionKind {
    /// A real cross-service business dependency. The only kind that is scored.
    #[default]
    Business,
    /// The call site lives in test code.
    TestOrigin,
    /// A self-call -- localhost or the caller's own configured host. source == target.
    Reflexive,
    /// A liveness or health probe.
    HealthInfra,
}
