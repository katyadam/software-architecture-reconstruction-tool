//! Constant substitution in REST call targets, shared by synthesizer and matcher.

use std::cmp::Reverse;
use std::collections::HashMap;

use regex::{Captures, Regex};

/// Replaces `{NAME}` holes and bare whole-word names, longest name first.
pub struct Substituter {
    re: Option<Regex>,
    values: HashMap<String, String>,
}

impl Substituter {
    /// `constants` are `(name, value)` pairs; on a duplicate name the last wins.
    pub fn new(constants: &[(String, String)]) -> Result<Self, regex::Error> {
        let mut sorted = constants.to_vec();
        sorted.sort_by_key(|(name, _)| Reverse(name.len()));
        let values = sorted.iter().cloned().collect();
        if sorted.is_empty() {
            return Ok(Self { re: None, values });
        }
        let names = sorted
            .iter()
            .map(|(name, _)| regex::escape(name))
            .collect::<Vec<_>>()
            .join("|");
        let re = Regex::new(&format!(r"\{{(?:{names})\}}|\b(?:{names})\b"))?;
        Ok(Self {
            re: Some(re),
            values,
        })
    }

    pub fn apply(&self, target: &str) -> String {
        let Some(re) = &self.re else {
            return target.to_string();
        };
        re.replace_all(target, |caps: &Captures| {
            let key = caps[0].trim_start_matches('{').trim_end_matches('}');
            self.values
                .get(key)
                .cloned()
                .unwrap_or_else(|| caps[0].to_string())
        })
        .into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(pairs: &[(&str, &str)]) -> Substituter {
        let pairs: Vec<_> = pairs
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect();
        Substituter::new(&pairs).expect("regex compiles")
    }

    #[test]
    fn hole_replaced() {
        let s = sub(&[("MDS_URL", "http://mds:8000")]);
        assert_eq!(s.apply("{MDS_URL}/v3/cases"), "http://mds:8000/v3/cases");
    }

    #[test]
    fn bare_word_replaced() {
        let s = sub(&[("settings.mds_url", "http://mds:8000")]);
        assert_eq!(s.apply("settings.mds_url/v3"), "http://mds:8000/v3");
    }

    #[test]
    fn name_inside_word_kept() {
        let s = sub(&[("HOST", "h")]);
        assert_eq!(s.apply("HOSTNAME/x"), "HOSTNAME/x");
    }

    #[test]
    fn no_constants_is_identity() {
        assert_eq!(sub(&[]).apply("{X}/y"), "{X}/y");
    }
}
