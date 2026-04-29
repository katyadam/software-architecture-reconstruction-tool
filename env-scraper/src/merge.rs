//! Deterministic merge of [`EnvEntry`] slices.
//!
//! Rules (from the plan's conflict-resolution table):
//! - Higher `priority` wins.
//! - Same priority -> first-file-encountered wins; collision logged with `warn!`.
//! - Priority 0 entries (docker-compose `${VAR}` placeholders) are **dropped**.

use std::collections::HashMap;

use log::warn;

use crate::parsers::EnvEntry;

/// Result of a merge operation.
#[derive(Debug, Default)]
pub struct MergeResult {
    pub entries: Vec<EnvEntry>,
    pub collisions: usize,
}

/// Merge `entries` into a deterministic flat map.
///
/// `entries` should be in discovery order (files encountered first take
/// priority on ties).  Higher `priority` always wins regardless of order.
pub fn merge(entries: Vec<EnvEntry>) -> MergeResult {
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut result: Vec<EnvEntry> = Vec::new();
    let mut collisions = 0usize;

    for entry in entries {
        // Drop placeholders (priority 0).
        if entry.priority == 0 {
            continue;
        }

        match index.get(&entry.name) {
            None => {
                let idx = result.len();
                index.insert(entry.name.clone(), idx);
                result.push(entry);
            }
            Some(&existing_idx) => {
                let existing = &result[existing_idx];
                if entry.priority > existing.priority {
                    // Incoming wins.
                    warn!(
                        "merge: collision for key '{}': '{}' (priority {}) overrides '{}' (priority {})",
                        entry.name,
                        entry.source_path,
                        entry.priority,
                        existing.source_path,
                        existing.priority,
                    );
                    collisions += 1;
                    result[existing_idx] = entry;
                } else if entry.priority == existing.priority {
                    // Same priority: first encountered wins; log it.
                    warn!(
                        "merge: collision for key '{}': '{}' (priority {}) kept over '{}' (same priority)",
                        entry.name, existing.source_path, existing.priority, entry.source_path,
                    );
                    collisions += 1;
                }
                // Lower priority: silently drop.
            }
        }
    }

    MergeResult {
        entries: result,
        collisions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::SourceKind;

    fn entry(name: &str, value: &str, path: &str, priority: u8) -> EnvEntry {
        EnvEntry {
            name: name.to_owned(),
            value: value.to_owned(),
            source_path: path.to_owned(),
            kind: SourceKind::DotEnv,
            priority,
        }
    }

    #[test]
    fn higher_priority_wins() {
        let entries = vec![
            entry("KEY", "low", "/sample.env", 60),
            entry("KEY", "high", "/.env", 100),
        ];
        let result = merge(entries);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].value, "high");
        assert_eq!(result.collisions, 1);
    }

    #[test]
    fn same_priority_first_wins() {
        let entries = vec![
            entry("KEY", "first", "/a/sample.env", 60),
            entry("KEY", "second", "/b/sample.env", 60),
        ];
        let result = merge(entries);
        assert_eq!(result.entries[0].value, "first");
        assert_eq!(result.collisions, 1);
    }

    #[test]
    fn placeholder_priority_zero_dropped() {
        let entries = vec![EnvEntry {
            name: "KEY".to_owned(),
            value: "${SOME_VAR}".to_owned(),
            source_path: "/docker-compose.yml".to_owned(),
            kind: SourceKind::DockerCompose,
            priority: 0,
        }];
        let result = merge(entries);
        assert!(result.entries.is_empty());
        assert_eq!(result.collisions, 0);
    }

    #[test]
    fn no_collision_different_keys() {
        let entries = vec![
            entry("A", "1", "/.env", 100),
            entry("B", "2", "/sample.env", 60),
        ];
        let result = merge(entries);
        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.collisions, 0);
    }

    #[test]
    fn lower_priority_silently_dropped() {
        let entries = vec![
            entry("KEY", "high", "/.env", 100),
            entry("KEY", "low", "/docker-compose.yml", 40),
        ];
        let result = merge(entries);
        assert_eq!(result.entries[0].value, "high");
        // Lower-priority entry is dropped silently, no collision count increment.
        assert_eq!(result.collisions, 0);
    }

    // -- additional edge cases --

    #[test]
    fn test_merge_three_way_highest_wins() {
        // Three entries for the same key at different priorities.
        let entries = vec![
            entry("KEY", "lowest", "/docker-compose.yml", 40),
            entry("KEY", "middle", "/sample.env", 60),
            entry("KEY", "highest", "/.env", 100),
        ];
        let result = merge(entries);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].value, "highest");
        // Two collisions: middle replaces lowest, then highest replaces middle.
        assert_eq!(result.collisions, 2);
    }

    #[test]
    fn test_merge_two_same_priority_first_wins_second_is_collision() {
        let entries = vec![
            entry("X", "alpha", "/svc-a/sample.env", 60),
            entry("X", "beta", "/svc-b/sample.env", 60),
        ];
        let result = merge(entries);
        assert_eq!(result.entries[0].value, "alpha");
        assert_eq!(result.collisions, 1);
    }

    #[test]
    fn test_merge_three_same_priority_two_collisions() {
        // Three files, all priority 60 — first wins, two collisions logged.
        let entries = vec![
            entry("K", "a", "/a.env", 60),
            entry("K", "b", "/b.env", 60),
            entry("K", "c", "/c.env", 60),
        ];
        let result = merge(entries);
        assert_eq!(result.entries[0].value, "a");
        assert_eq!(result.collisions, 2);
    }

    #[test]
    fn test_merge_empty_input_returns_empty() {
        let result = merge(vec![]);
        assert!(result.entries.is_empty());
        assert_eq!(result.collisions, 0);
    }

    #[test]
    fn test_merge_all_priority_zero_dropped() {
        let entries = vec![
            EnvEntry {
                name: "A".to_owned(),
                value: "${A}".to_owned(),
                source_path: "/dc.yml".to_owned(),
                kind: SourceKind::DockerCompose,
                priority: 0,
            },
            EnvEntry {
                name: "B".to_owned(),
                value: "${B}".to_owned(),
                source_path: "/dc.yml".to_owned(),
                kind: SourceKind::DockerCompose,
                priority: 0,
            },
        ];
        let result = merge(entries);
        assert!(result.entries.is_empty());
        assert_eq!(result.collisions, 0);
    }

    #[test]
    fn test_merge_collision_higher_priority_replaces_existing() {
        // Incoming higher-priority entry must replace the loser.
        let entries = vec![
            entry("KEY", "loser", "/sample.env", 60),
            entry("KEY", "winner", "/.env", 100),
        ];
        let result = merge(entries);
        assert_eq!(result.entries[0].value, "winner");
        assert_eq!(result.collisions, 1);
    }
}
