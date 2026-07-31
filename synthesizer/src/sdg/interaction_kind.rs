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
/// Reordering these variants changes the rollup (rule 2). Per-request
/// precedence (rule 1) is a second, independent encoding -- the order of the
/// early returns in [`classify`] -- and must be kept consistent with this
/// ordering by hand; reordering here does not itself change `classify`.
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

impl InteractionKind {
    /// Roll a connection's request kinds up to a single edge kind.
    /// `Business` wins any tie, so one real business request keeps the whole
    /// edge in the business view -- the RTS-safe direction.
    pub fn rollup(kinds: impl Iterator<Item = Self>) -> Self {
        kinds.min().unwrap_or_default()
    }
}

use models::ir::language::Language;

/// Hosts that always mean "the caller itself".
///
/// No bare `"::1"` here: `host_of` splits an unbracketed authority at the
/// first colon, so a bare `::1` can never come out of it -- only the
/// bracketed `"[::1]"` form is reachable. Adding `"::1"` back would be dead.
const REFLEXIVE_HOSTS: &[&str] = &["localhost", "127.0.0.1", "0.0.0.0", "[::1]"];

/// Final path segments that mean a liveness or health probe.
const HEALTH_SEGMENTS: &[&str] = &[
    "alive",
    "health",
    "healthz",
    "ready",
    "readiness",
    "live",
    "liveness",
    "ping",
];

/// The evidence `classify` decides on.
///
/// Deliberately `&str`s rather than `&RestCall` and `&Endpoint`: a new
/// protocol (gRPC, message queues) types its interactions by filling this in
/// from its own element types, with no edit to this module. gRPC puts its method
/// in `target_path` (`/grpc.health.v1.Health/Check` types as `HealthInfra` under
/// the existing rule); a queue puts the topic there and supplies a URI-shaped
/// target (`amqp://broker:5672`) in `target_uri`.
pub struct InteractionSignals<'a> {
    /// Path of the file the call site lives in.
    pub caller_file: &'a str,
    /// Resolved path, method, or topic. May be a full URL when no path is known.
    pub target_path: &'a str,
    /// The raw, unparsed target URI. May be relative, in which case it carries
    /// no host or port and is never reflexive.
    pub target_uri: &'a str,
}

/// Classify one interaction.
///
/// `own_urls` is the *calling* service's configured URL list verbatim (e.g.
/// `http://mds:8000`); `all_urls` is every service's configured URL list
/// flattened together. Both are passed through to `is_reflexive` verbatim --
/// this module stays the only place that knows how to parse them.
///
/// The early returns are in precedence order; see [`InteractionKind`].
pub fn classify(
    s: &InteractionSignals,
    own_urls: &[String],
    all_urls: &[String],
) -> InteractionKind {
    if is_test_path(Language::from_path(s.caller_file), s.caller_file) {
        return InteractionKind::TestOrigin;
    }
    if is_reflexive(s.target_uri, own_urls, all_urls) {
        return InteractionKind::Reflexive;
    }
    if is_health_path(s.target_path) {
        return InteractionKind::HealthInfra;
    }
    InteractionKind::Business
}

/// Does `target_uri` resolve to the caller itself?
///
/// Public because the builder needs this *before* matching, to decide whether a
/// restcall may match its own service's endpoints.
///
/// Ownership first, loopback as fallback -- a bare loopback host is not
/// enough, because a *different* service can be configured on `localhost` at
/// its own port (e.g. `loadtus-service` at `http://localhost:10005`), and
/// blanket-matching loopback would swallow that real cross-service edge:
///
/// 1. `target_uri`'s authority (`host:port`) matches one of `own_urls` -> reflexive.
/// 2. Else it matches an authority in `all_urls` (some other service owns it) -> not reflexive.
/// 3. Else the host is a bare loopback address -> reflexive.
/// 4. Else -> not reflexive.
pub fn is_reflexive(target_uri: &str, own_urls: &[String], all_urls: &[String]) -> bool {
    let authority = authority_of(target_uri);
    if authority.is_empty() {
        return false;
    }
    let authority = authority.to_ascii_lowercase();

    if own_urls
        .iter()
        .any(|url| authority_of(url).to_ascii_lowercase() == authority)
    {
        return true;
    }
    if all_urls
        .iter()
        .any(|url| authority_of(url).to_ascii_lowercase() == authority)
    {
        return false;
    }
    REFLEXIVE_HOSTS.contains(&host_of(target_uri).to_ascii_lowercase().as_str())
}

/// The `host:port` authority of a URI, with userinfo stripped, before the
/// host/port split -- shared by [`host_of`] and [`authority_of`].
///
/// Returns `""` for a relative URI: only an authority-looking string (one
/// with a scheme) can carry a host.
fn authority_str(uri: &str) -> &str {
    let after_scheme = match uri.split_once("://") {
        Some((_, rest)) => rest,
        None => return "",
    };

    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    match authority.rsplit_once('@') {
        Some((_userinfo, host_port)) => host_port,
        None => authority,
    }
}

/// Extract the host from a URI, without the scheme, userinfo, port, or path.
///
/// Returns `""` for a relative URI, which is never reflexive -- so relative
/// targets keep behaving exactly as they do today.
pub fn host_of(uri: &str) -> &str {
    let host_port = authority_str(uri);

    // An IPv6 literal is bracketed, and its colons are not port separators.
    if host_port.starts_with('[') {
        return match host_port.find(']') {
            Some(end) => &host_port[..=end],
            None => host_port,
        };
    }

    match host_port.split_once(':') {
        Some((host, _port)) => host,
        None => host_port,
    }
}

/// Extract the `host:port` authority from a URI (or just the host, when the
/// URI carries no port), without the scheme, userinfo, or path.
///
/// Returns `""` for a relative URI, same as [`host_of`].
pub fn authority_of(uri: &str) -> &str {
    authority_str(uri)
}

/// Does this call site live in test code?
///
/// The only rule that varies by language. Adding a language means adding one
/// arm here and one arm in [`Language::from_path`] -- e.g. Go would match
/// `*_test.go`, TypeScript `*.spec.ts` and a `__tests__` segment.
fn is_test_path(lang: Language, path: &str) -> bool {
    let file_name = path.rsplit('/').next().unwrap_or(path);

    match lang {
        Language::Java => {
            path.contains("/src/test/")
                || file_name.ends_with("Test.java")
                || file_name.ends_with("Tests.java")
                || file_name.ends_with("IT.java")
        }
        Language::Python => {
            has_test_segment(path)
                || file_name == "conftest.py"
                || (file_name.starts_with("test_") && file_name.ends_with(".py"))
                || file_name.ends_with("_test.py")
        }
        // A language whose arm has not been added yet still gets the safe,
        // convention-independent part of the rule.
        Language::Unknown => has_test_segment(path),
    }
}

/// A whole path segment equal to `test` or `tests` -- so `latest/` does not match.
fn has_test_segment(path: &str) -> bool {
    path.split('/').any(|seg| seg == "test" || seg == "tests")
}

/// Is this a liveness or health probe?
///
/// Language-agnostic. Works on a bare path or a full URL, since it looks at the
/// last segment either way.
fn is_health_path(path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    if path.split('/').any(|seg| seg == "actuator") {
        return true;
    }
    let last = path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default();
    HEALTH_SEGMENTS.contains(&last)
}
