use std::collections::{HashMap, HashSet};

use models::{
    assignments::{Scope, VariableAddress},
    ir::language::Language,
};
use once_cell::sync::Lazy;
use regex::Regex;

/// Hostname/host:port regex used by [`looks_url_or_host`]. Compiled once.
static HOST_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]*(:[0-9]+)?(/.*)?$").expect("static regex compiles")
});

/// Rank `all` by relevance to `snippet_code` (considering soft locality on
/// `snippet_microservice` / `snippet_file`) and return the top `budget`
/// entries as an ordered `Vec`. The returned order -> highest score first,
/// ties broken by [`stable_key`] -> is preserved by downstream prompt
/// rendering, so identical inputs always produce identical prompts.
pub(super) fn rank_and_cap(
    all: &HashMap<VariableAddress, String>,
    snippet_code: &str,
    snippet_language: Language,
    snippet_microservice: &str,
    snippet_file: &str,
    budget: usize,
) -> Vec<(VariableAddress, String)> {
    let idents = extract_identifiers(snippet_code, snippet_language);
    let mut scored: Vec<(i32, &VariableAddress, &String)> = all
        .iter()
        .map(|(addr, value)| {
            (
                score(addr, value, &idents, snippet_microservice, snippet_file),
                addr,
                value,
            )
        })
        .collect();
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| stable_key(a.1).cmp(&stable_key(b.1)))
    });
    scored
        .into_iter()
        .take(budget)
        .map(|(_, addr, value)| (addr.clone(), value.clone()))
        .collect()
}

fn score(
    addr: &VariableAddress,
    value: &str,
    idents: &HashSet<String>,
    snippet_microservice: &str,
    snippet_file: &str,
) -> i32 {
    let mut s = 0;
    let var_name = &addr.key.variable_name;
    // `extract_identifiers` already inserts the leading-underscore-stripped
    // lowercase form of every token, so a single normalized lookup subsumes
    // the original raw-name check.
    let var_norm = var_name.trim_start_matches('_').to_lowercase();
    if idents.contains(&var_norm) {
        s += 100;
    }
    if idents.iter().any(|i| name_similar(i, var_name)) {
        s += 40;
    }
    if looks_url_or_host(value) {
        s += 30;
    }
    if name_hints_url(var_name) {
        s += 15;
    }
    if addr.microservice == snippet_microservice {
        s += 20;
    }
    if addr.file == snippet_file {
        s += 10;
    }
    if matches!(addr.key.scope, Scope::Global) {
        s += 5;
    }
    s
}

/// Extract identifier tokens from `code` using tree-sitter for `language`.
///
/// Tree-sitter sources the raw identifier set (skipping string literals and
/// comments — the regex predecessor matched those false positives). Each raw
/// token is then normalized the same way as the previous regex pipeline:
/// `self`/`this` are dropped, the raw form is kept, plus a lowercase form,
/// a leading-underscore-stripped lowercase form, and lowercase pieces from
/// snake_case and camelCase splits. Downstream scoring relies on these
/// normalized variants existing in the set.
fn extract_identifiers(code: &str, language: Language) -> HashSet<String> {
    let raw_tokens = statix::identifiers::identifiers_in_snippet(code, language);
    let mut out: HashSet<String> = HashSet::new();
    for raw in raw_tokens {
        if raw == "self" || raw == "this" {
            continue;
        }
        let lower = raw.to_lowercase();
        let stripped = lower.trim_start_matches('_').to_string();
        let snake_parts: Vec<String> = split_snake(&raw)
            .into_iter()
            .filter(|p| !p.is_empty())
            .map(|p| p.to_lowercase())
            .collect();
        let camel_parts: Vec<String> = split_camel(&raw)
            .into_iter()
            .filter(|p| !p.is_empty())
            .map(|p| p.to_lowercase())
            .collect();
        out.insert(raw);
        out.insert(lower);
        if !stripped.is_empty() {
            out.insert(stripped);
        }
        out.extend(snake_parts);
        out.extend(camel_parts);
    }
    out
}

fn split_snake(s: &str) -> Vec<String> {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect()
}

fn split_camel(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    for c in s.chars() {
        if c.is_uppercase() && !current.is_empty() {
            parts.push(std::mem::take(&mut current));
        }
        current.push(c);
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

/// Case-insensitive similarity after stripping leading underscores.
/// `_mds_url` is similar to `MDS_URL`.
fn name_similar(a: &str, b: &str) -> bool {
    let an = a.trim_start_matches('_').to_lowercase();
    let bn = b.trim_start_matches('_').to_lowercase();
    if an.is_empty() || bn.is_empty() {
        return false;
    }
    if an == bn {
        return true;
    }
    an.contains(&bn) || bn.contains(&an)
}

/// Heuristic: does `value` look like a URL or hostname?
fn looks_url_or_host(value: &str) -> bool {
    let v = value.trim().trim_matches('"').trim_matches('\'');
    if v.starts_with("http://") || v.starts_with("https://") {
        return true;
    }
    if v.contains("://") {
        return true;
    }
    HOST_RE.is_match(v) && (v.contains('.') || v.contains(':'))
}

/// Heuristic: does the uppercased variable name look URL-ish?
fn name_hints_url(name: &str) -> bool {
    let upper = name.to_uppercase();
    ["URL", "URI", "HOST", "ENDPOINT", "BASE", "PORT"]
        .iter()
        .any(|needle| upper.contains(needle))
}

/// Deterministic tie-breaker for [`rank_and_cap`].
fn stable_key(addr: &VariableAddress) -> (String, String, String, String) {
    let scope = match &addr.key.scope {
        Scope::Global => "Global".to_string(),
        Scope::Class(n) => format!("Class:{n}"),
        Scope::Function(n) => format!("Function:{n}"),
    };
    (
        addr.microservice.clone(),
        addr.file.clone(),
        scope,
        addr.key.variable_name.clone(),
    )
}

#[cfg(test)]
mod ranking_tests {
    use super::*;
    use models::assignments::AssignmentKey;

    fn addr(microservice: &str, file: &str, scope: Scope, name: &str) -> VariableAddress {
        VariableAddress {
            microservice: microservice.to_string(),
            file: file.to_string(),
            key: AssignmentKey {
                scope,
                variable_name: name.to_string(),
            },
        }
    }

    fn serialized_in_order(entries: &[(VariableAddress, String)]) -> Vec<String> {
        entries
            .iter()
            .map(|(a, v)| {
                let (m, f, s, n) = stable_key(a);
                format!("{m}|{f}|{s}|{n}={v}")
            })
            .collect()
    }

    #[test]
    fn empaia_style_cross_microservice_match_wins() {
        let snippet = "url = f\"{self._mds_url}/v3/foo\"\nreturn await self._http_client.get(url, headers=self._headers)";

        let mds_url = addr(
            "medical-data-service",
            "/p/mds/client.py",
            Scope::Global,
            "MDS_URL",
        );
        let base_url = addr("app-service", "/p/app/base.py", Scope::Global, "BASE_URL");

        let mut all: HashMap<VariableAddress, String> = HashMap::new();
        all.insert(
            mds_url.clone(),
            "http://medical-data-service:8000".to_string(),
        );
        all.insert(base_url, "http://app-service:7000".to_string());
        // pad with noise so locality alone cannot win
        for i in 0..200 {
            all.insert(
                addr(
                    "app-service",
                    "/p/app/noise.py",
                    Scope::Global,
                    &format!("NOISE_VAR_{i}"),
                ),
                format!("value_{i}"),
            );
        }

        let pruned = rank_and_cap(
            &all,
            snippet,
            Language::Python,
            "app-service",
            "/p/app/caller.py",
            10,
        );
        assert!(!pruned.is_empty());
        assert!(pruned.len() <= 10);
        // The first element of the returned ordered Vec must be MDS_URL —
        // proves production sort itself ranks it first, not just inclusion.
        assert_eq!(
            pruned[0].0.key.variable_name,
            "MDS_URL",
            "expected MDS_URL at position 0, got order: {:?}",
            pruned
                .iter()
                .map(|(a, _)| a.key.variable_name.clone())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn budget_cap_returns_exactly_budget_entries() {
        let mut all: HashMap<VariableAddress, String> = HashMap::new();
        for i in 0..1000 {
            all.insert(
                addr("svc-a", "/p/f.py", Scope::Global, &format!("VAR_{i}")),
                format!("value_{i}"),
            );
        }
        let pruned = rank_and_cap(&all, "foo()", Language::Python, "svc-a", "/p/f.py", 50);
        assert_eq!(pruned.len(), 50);
    }

    #[test]
    fn determinism_with_shuffled_input_ordering() {
        // Two semantically equal source maps built with different insertion order.
        let mut a: HashMap<VariableAddress, String> = HashMap::new();
        let mut b: HashMap<VariableAddress, String> = HashMap::new();
        let names: Vec<String> = (0..200).map(|i| format!("VAR_{i:03}")).collect();
        for n in &names {
            a.insert(addr("svc", "/p/f.py", Scope::Global, n), format!("v_{n}"));
        }
        for n in names.iter().rev() {
            b.insert(addr("svc", "/p/f.py", Scope::Global, n), format!("v_{n}"));
        }

        let pa = rank_and_cap(
            &a,
            "x = VAR_042 + 1",
            Language::Python,
            "svc",
            "/p/f.py",
            25,
        );
        let pb = rank_and_cap(
            &b,
            "x = VAR_042 + 1",
            Language::Python,
            "svc",
            "/p/f.py",
            25,
        );
        // Compare order-preserving — Vec equality covers both contents and order.
        assert_eq!(serialized_in_order(&pa), serialized_in_order(&pb));
    }

    #[test]
    fn empty_variables_map_returns_empty() {
        let all: HashMap<VariableAddress, String> = HashMap::new();
        let pruned = rank_and_cap(&all, "anything", Language::Python, "svc", "/p/f.py", 50);
        assert!(pruned.is_empty());
    }

    #[test]
    fn budget_zero_returns_empty_without_panic() {
        let mut all: HashMap<VariableAddress, String> = HashMap::new();
        for i in 0..10 {
            all.insert(
                addr("svc", "/p/f.py", Scope::Global, &format!("VAR_{i}")),
                format!("v{i}"),
            );
        }
        let pruned = rank_and_cap(&all, "x = VAR_1", Language::Python, "svc", "/p/f.py", 0);
        assert!(pruned.is_empty());
    }

    #[test]
    fn all_zero_scoring_returns_deterministic_top_n() {
        // Snippet has no alphabetic identifiers, microservice/file differ from
        // every entry, values are plain numbers, names contain no URL hints,
        // and scope is Function (no +5 Global bonus). All scores collapse to 0.
        let mut all: HashMap<VariableAddress, String> = HashMap::new();
        let names: Vec<String> = (0..20).map(|i| format!("VAR_{i:02}")).collect();
        for n in &names {
            all.insert(
                addr("svc-x", "/p/f.py", Scope::Function("fn".to_string()), n),
                "1".to_string(),
            );
        }
        let pruned = rank_and_cap(
            &all,
            "1 + 2 * 3",
            Language::Python,
            "svc-y",
            "/p/other.py",
            5,
        );
        assert_eq!(pruned.len(), 5);
        // With all scores equal, the first 5 by stable_key (alphabetical names)
        // must be selected in alphabetical order: VAR_00..VAR_04.
        let picked_names: Vec<String> = pruned
            .iter()
            .map(|(a, _)| a.key.variable_name.clone())
            .collect();
        assert_eq!(
            picked_names,
            vec!["VAR_00", "VAR_01", "VAR_02", "VAR_03", "VAR_04"]
        );
    }

    #[test]
    fn snippet_without_identifiers_does_not_panic() {
        let mut all: HashMap<VariableAddress, String> = HashMap::new();
        all.insert(
            addr("svc", "/p/f.py", Scope::Global, "VAR_A"),
            "1".to_string(),
        );
        // No identifier characters at all.
        let pruned = rank_and_cap(
            &all,
            "   {}[]()+-*/  ",
            Language::Python,
            "svc",
            "/p/f.py",
            5,
        );
        assert_eq!(pruned.len(), 1);
    }

    #[test]
    fn very_long_snippet_completes_quickly() {
        let mut all: HashMap<VariableAddress, String> = HashMap::new();
        for i in 0..500 {
            all.insert(
                addr("svc", "/p/f.py", Scope::Global, &format!("VAR_{i}")),
                format!("http://example.com/{i}"),
            );
        }
        // ~10 KB snippet of mixed identifiers and noise.
        let mut snippet = String::with_capacity(10_240);
        while snippet.len() < 10_000 {
            snippet.push_str("call(VAR_42, base_url, other_thing); // padding 1234567890\n");
        }
        let start = std::time::Instant::now();
        let pruned = rank_and_cap(&all, &snippet, Language::Python, "svc", "/p/f.py", 50);
        let elapsed = start.elapsed();
        assert_eq!(pruned.len(), 50);
        assert!(
            elapsed < std::time::Duration::from_secs(2),
            "rank_and_cap took {elapsed:?}, expected < 2s"
        );
    }

    #[test]
    fn locality_bonus_picks_same_microservice_over_other_service() {
        // Two variables with identical names; locality bonuses (+20 same
        // microservice, +10 same file) differentiate the scores, so this
        // test exercises the locality contribution, not the tie-breaker.
        let local = addr("svc-a", "/p/a.py", Scope::Global, "SHARED");
        let remote = addr("svc-b", "/p/b.py", Scope::Global, "SHARED");

        let mut all: HashMap<VariableAddress, String> = HashMap::new();
        all.insert(local.clone(), "v1".to_string());
        all.insert(remote.clone(), "v2".to_string());

        let pruned = rank_and_cap(&all, "y = 1", Language::Python, "svc-a", "/p/a.py", 1);
        assert_eq!(pruned.len(), 1);
        assert_eq!(pruned[0].0.microservice, "svc-a");
    }

    #[test]
    fn stable_key_decides_when_scores_truly_tie() {
        // Two variables with identical scoring inputs from every angle:
        // same scope (Global -> +5), neither matches the snippet idents,
        // neither value is URL-shaped, neither name hints URL, and locality
        // is identical (same microservice, same file -> +20 +10). Only
        // stable_key (alphabetical by name) can decide.
        let earlier = addr("svc", "/p/f.py", Scope::Global, "AAA");
        let later = addr("svc", "/p/f.py", Scope::Global, "ZZZ");

        let mut all: HashMap<VariableAddress, String> = HashMap::new();
        all.insert(earlier.clone(), "1".to_string());
        all.insert(later.clone(), "2".to_string());

        // Sanity check: scores really tie.
        let idents = extract_identifiers("y = 1", Language::Python);
        assert_eq!(
            score(&earlier, "1", &idents, "svc", "/p/f.py"),
            score(&later, "2", &idents, "svc", "/p/f.py"),
            "test setup error: scores should tie"
        );

        let pruned = rank_and_cap(&all, "y = 1", Language::Python, "svc", "/p/f.py", 1);
        assert_eq!(pruned.len(), 1);
        assert_eq!(pruned[0].0.key.variable_name, "AAA");
    }
}
