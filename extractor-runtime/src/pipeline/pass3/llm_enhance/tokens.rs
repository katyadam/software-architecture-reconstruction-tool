/// Not service-name evidence; skipped when matching.
pub(super) const RECEIVER_KEYWORDS: &[&str] = &["self", "this"];

pub(super) fn split_snake(s: &str) -> Vec<String> {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect()
}

pub(super) fn split_camel(s: &str) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_drops_empties() {
        assert_eq!(split_snake("_mds_url_"), vec!["mds", "url"]);
        assert!(split_snake("").is_empty());
    }

    #[test]
    fn camel_splits_on_uppercase() {
        assert_eq!(
            split_camel("MedicalDataService"),
            vec!["Medical", "Data", "Service"]
        );
        assert_eq!(split_camel("url"), vec!["url"]);
    }
}
