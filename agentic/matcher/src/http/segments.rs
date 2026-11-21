//! Path segment rules for candidate matching, copied from the synthesizer.

use std::collections::BTreeSet;

use super::MatchTag;

/// Tag for call segments `c` against endpoint segments `s`, `None` if they do not match.
pub(super) fn match_tag(
    c: &[&str],
    s: &[&str],
    leading_hole: bool,
    host_known: bool,
) -> Option<MatchTag> {
    if c.len() == s.len() && unifies(c, s) {
        if c.iter().chain(s).any(|x| is_var(x)) {
            Some(MatchTag::Template)
        } else {
            Some(MatchTag::Exact)
        }
    } else if (leading_hole && s.len() + 1 >= c.len() && unifies(&c[1..], tail(s, c.len() - 1)))
        // Lost router prefix, e.g. `app.mount("/v3")`.
        || (host_known && s.len() < c.len() && unifies(tail(c, s.len()), s))
    {
        Some(MatchTag::Suffix)
    } else {
        None
    }
}

/// True when some aligned position is literal on both sides; `{a}/{b}` unifies with anything.
pub(super) fn has_evidence(c: &[&str], s: &[&str]) -> bool {
    !literals(c, s).is_empty()
}

/// Path segments without scheme, authority, query or fragment.
pub(super) fn segments(uri: &str) -> Vec<&str> {
    let path = uri.split(['?', '#']).next().unwrap_or_default();
    let path = match path.split_once("://") {
        Some((_, rest)) => rest.split_once('/').map_or("", |(_, p)| p),
        None => path,
    };
    path.split('/').filter(|s| !s.is_empty()).collect()
}

pub(super) fn is_var(seg: &str) -> bool {
    seg.contains('{') || seg.starts_with(':') || seg == "*" || seg == "**"
}

pub(super) fn unifies(a: &[&str], b: &[&str]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| is_var(x) || is_var(y) || x == y)
}

pub(super) fn tail<'s, 'a>(segs: &'s [&'a str], n: usize) -> &'s [&'a str] {
    &segs[segs.len().saturating_sub(n)..]
}

/// Positions, counted from the end, where both `c` and `s` are literal.
pub(super) fn literals(c: &[&str], s: &[&str]) -> BTreeSet<usize> {
    c.iter()
        .rev()
        .zip(s.iter().rev())
        .enumerate()
        .filter(|(_, (x, y))| !is_var(x) && !is_var(y))
        .map(|(i, _)| i)
        .collect()
}
