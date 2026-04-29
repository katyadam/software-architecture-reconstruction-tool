//! Parser for `docker-compose*.yml` / `compose*.yml` `environment:` blocks.
//!
//! Handles:
//! - `environment:` as a **map** (`KEY: value`)
//! - `environment:` as a **list** (`- KEY=value`)
//! - Multi-service files (all services iterated)
//! - `null` values (skipped)
//! - `${VAR}` placeholders in values (priority 0 -> dropped by the merge step)
//!
//! `env_file:` references are intentionally ignored — those files are picked
//! up by the dotenv pass that already walked the same directory tree.

use std::path::Path;

use log::{debug, warn};
use serde_yaml::Value;

use super::{EnvEntry, Parser, SourceKind};

const DC_LITERAL_PRIORITY: u8 = 40;
const DC_PLACEHOLDER_PRIORITY: u8 = 0;

pub struct DockerComposeParser;

impl Parser for DockerComposeParser {
    fn parse(&self, content: &str, path: &Path) -> Vec<EnvEntry> {
        let path_str = path.to_string_lossy();

        let doc: Value = match serde_yaml::from_str(content) {
            Ok(v) => v,
            Err(e) => {
                warn!("docker_compose: failed to parse {}: {}", path_str, e);
                return Vec::new();
            }
        };

        let services = match doc.get("services") {
            Some(Value::Mapping(m)) => m,
            _ => {
                debug!(
                    "docker_compose: no 'services' mapping found in {}",
                    path_str
                );
                return Vec::new();
            }
        };

        let mut entries = Vec::new();

        for (svc_name, svc_val) in services {
            let svc_label = svc_name.as_str().unwrap_or("<unknown>");

            let env_block = match svc_val.get("environment") {
                Some(v) => v,
                None => continue,
            };

            match env_block {
                // Map form: `environment:\n  KEY: value`
                Value::Mapping(map) => {
                    for (k, v) in map {
                        let key = match k.as_str() {
                            Some(s) => s,
                            None => {
                                debug!(
                                    "docker_compose: non-string key in service '{}' of {}",
                                    svc_label, path_str
                                );
                                continue;
                            }
                        };

                        match v {
                            Value::Null => {
                                debug!(
                                    "docker_compose: null value for key '{}' in service '{}' of {}; skipping",
                                    key, svc_label, path_str
                                );
                            }
                            _ => {
                                let value_str = yaml_value_to_string(v);
                                push_entry(&mut entries, key, &value_str, path_str.as_ref());
                            }
                        }
                    }
                }

                // List form: `environment:\n  - KEY=value`
                Value::Sequence(seq) => {
                    for item in seq {
                        let item_str = match item.as_str() {
                            Some(s) => s,
                            None => {
                                debug!(
                                    "docker_compose: non-string list item in service '{}' of {}",
                                    svc_label, path_str
                                );
                                continue;
                            }
                        };

                        match item_str.find('=') {
                            Some(eq_pos) => {
                                let key = &item_str[..eq_pos];
                                let value = &item_str[eq_pos + 1..];
                                push_entry(&mut entries, key, value, path_str.as_ref());
                            }
                            None => {
                                // `- KEY` form (value from host env) — skip.
                                debug!(
                                    "docker_compose: no '=' in list item {:?} service '{}' of {}; skipping",
                                    item_str, svc_label, path_str
                                );
                            }
                        }
                    }
                }

                other => {
                    debug!(
                        "docker_compose: unexpected environment value type {:?} in service '{}' of {}",
                        other, svc_label, path_str
                    );
                }
            }
        }

        entries
    }
}

/// Decide priority and push an entry.
/// `${VAR}` placeholders get priority 0 (they will be dropped during merge).
fn push_entry(entries: &mut Vec<EnvEntry>, key: &str, value: &str, path_str: &str) {
    let priority = if is_placeholder(value) {
        DC_PLACEHOLDER_PRIORITY
    } else {
        DC_LITERAL_PRIORITY
    };

    entries.push(EnvEntry {
        name: key.to_owned(),
        value: value.to_owned(),
        source_path: path_str.to_string(),
        kind: SourceKind::DockerCompose,
        priority,
    });
}

/// Returns `true` if the entire value is a shell variable placeholder (`${VAR}` or `$VAR`).
fn is_placeholder(value: &str) -> bool {
    let v = value.trim();
    if v.starts_with("${") && v.ends_with('}') {
        return true;
    }
    if v.starts_with('$') && v.len() > 1 && !v.contains('{') {
        return true;
    }
    false
}

fn yaml_value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Sequence(_) | Value::Mapping(_) => serde_yaml::to_string(v)
            .unwrap_or_default()
            .trim()
            .to_owned(),
        Value::Null | Value::Tagged(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn parse(src: &str) -> Vec<EnvEntry> {
        DockerComposeParser.parse(src, Path::new("/fake/docker-compose.yml"))
    }

    #[test]
    fn map_form() {
        let yaml = r#"
services:
  svc:
    environment:
      KEY: value
      OTHER: 123
"#;
        let entries = parse(yaml);
        assert_eq!(entries.len(), 2);
        let kv: std::collections::HashMap<_, _> = entries
            .iter()
            .map(|e| (e.name.as_str(), e.value.as_str()))
            .collect();
        assert_eq!(kv["KEY"], "value");
        assert_eq!(kv["OTHER"], "123");
    }

    #[test]
    fn list_form() {
        let yaml = r#"
services:
  svc:
    environment:
      - KEY=value
      - OTHER=hello
"#;
        let entries = parse(yaml);
        assert_eq!(entries.len(), 2);
        assert!(
            entries
                .iter()
                .any(|e| e.name == "KEY" && e.value == "value")
        );
    }

    #[test]
    fn placeholder_gets_priority_zero() {
        let yaml = r#"
services:
  svc:
    environment:
      KEY: "${SOME_VAR}"
"#;
        let entries = parse(yaml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].priority, 0);
    }

    #[test]
    fn null_value_skipped() {
        let yaml = r#"
services:
  svc:
    environment:
      KEY: ~
"#;
        let entries = parse(yaml);
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn multi_service() {
        let yaml = r#"
services:
  a:
    environment:
      A_KEY: a_val
  b:
    environment:
      B_KEY: b_val
"#;
        let entries = parse(yaml);
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn no_services_key_returns_empty() {
        let yaml = "version: '3'\n";
        let entries = parse(yaml);
        assert!(entries.is_empty());
    }

    // -- additional edge cases --

    #[test]
    fn test_parse_dc_service_without_environment_key_skipped() {
        let yaml = r#"
services:
  svc:
    image: myimage
"#;
        let entries = parse(yaml);
        assert!(
            entries.is_empty(),
            "service without environment: must produce nothing"
        );
    }

    #[test]
    fn test_parse_dc_list_form_value_with_equals() {
        // `- KEY=val=with=extras` -> value keeps all chars after first `=`
        let yaml = r#"
services:
  svc:
    environment:
      - JDBC_URL=jdbc:postgresql://host:5432/db?ssl=true
"#;
        let entries = parse(yaml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "JDBC_URL");
        assert_eq!(entries[0].value, "jdbc:postgresql://host:5432/db?ssl=true");
    }

    #[test]
    fn test_parse_dc_bare_dollar_var_priority_zero() {
        // `$VAR` (non-braced) should also be treated as a placeholder
        let yaml = r#"
services:
  svc:
    environment:
      KEY: $BARE_VAR
"#;
        let entries = parse(yaml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].priority, 0, "$BARE_VAR must get priority 0");
    }

    #[test]
    fn test_parse_dc_multi_service_no_env_one_has_env() {
        let yaml = r#"
services:
  no_env:
    image: foo
  has_env:
    environment:
      K: V
"#;
        let entries = parse(yaml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "K");
    }

    #[test]
    fn test_parse_dc_malformed_yaml_returns_empty() {
        let yaml = "services: [\nunclosed";
        let entries = parse(yaml);
        assert!(entries.is_empty(), "malformed YAML must return empty vec");
    }

    #[test]
    fn test_parse_dc_kind_is_docker_compose() {
        let yaml = r#"
services:
  svc:
    environment:
      K: V
"#;
        let entries = parse(yaml);
        assert_eq!(entries[0].kind, SourceKind::DockerCompose);
    }
}
