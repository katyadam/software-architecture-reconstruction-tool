//! Parser for `.env*` / `*.env` files.
//!
//! Handles:
//! - `KEY=value` (bare)
//! - `KEY="double quoted"` / `KEY='single quoted'`
//! - `export KEY=value`
//! - Comments (`# ...`)
//! - Blank lines
//! - JSON-array values like `MDS_CORS_ALLOW_ORIGINS=["*"]` (kept verbatim)
//! - `${VAR}` inside values (kept verbatim — TODO: second-pass interpolation)
//! - Malformed lines (no `=`) — skipped with a `debug!` log

use std::path::Path;

use log::debug;
use statix::strings::strip_quotes;

use super::{EnvEntry, Parser, SourceKind};

pub struct DotEnvParser {
    priority: u8,
}

impl DotEnvParser {
    pub fn new(priority: u8) -> Self {
        Self { priority }
    }
}

impl Parser for DotEnvParser {
    fn parse(&self, content: &str, path: &Path) -> Vec<EnvEntry> {
        let path_str = path.to_string_lossy();
        let mut entries = Vec::new();

        for (line_num, raw_line) in content.lines().enumerate() {
            let line = raw_line.trim();

            // Skip blank lines and comments.
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Strip optional `export ` prefix.
            let line = if let Some(rest) = line.strip_prefix("export ") {
                rest.trim_start()
            } else {
                line
            };

            // Split on the first `=`.
            let Some(eq_pos) = line.find('=') else {
                debug!(
                    "dotenv: skipping malformed line {} in {}: {:?}",
                    line_num + 1,
                    path_str,
                    raw_line
                );
                continue;
            };

            let key = line[..eq_pos].trim();
            let raw_val = line[eq_pos + 1..].trim();

            if key.is_empty() {
                debug!("dotenv: empty key at line {} in {}", line_num + 1, path_str);
                continue;
            }

            let value = strip_quotes(raw_val);

            // TODO: resolve `${VAR}` references against other entries in a second pass.

            entries.push(EnvEntry {
                name: key.to_owned(),
                value: value.to_owned(),
                source_path: path_str.to_string(),
                kind: SourceKind::DotEnv,
                priority: self.priority,
            });
        }

        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn parse(src: &str) -> Vec<EnvEntry> {
        DotEnvParser::new(100).parse(src, Path::new("/fake/.env"))
    }

    #[test]
    fn bare_assignment() {
        let entries = parse("KEY=value");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "KEY");
        assert_eq!(entries[0].value, "value");
    }

    #[test]
    fn double_quoted() {
        let entries = parse(r#"KEY="hello world""#);
        assert_eq!(entries[0].value, "hello world");
    }

    #[test]
    fn single_quoted() {
        let entries = parse("KEY='hello world'");
        assert_eq!(entries[0].value, "hello world");
    }

    #[test]
    fn export_prefix() {
        let entries = parse("export KEY=value");
        assert_eq!(entries[0].name, "KEY");
        assert_eq!(entries[0].value, "value");
    }

    #[test]
    fn comment_and_blank_lines_skipped() {
        let src = "\n# comment\nKEY=val\n\n";
        let entries = parse(src);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "KEY");
    }

    #[test]
    fn malformed_line_skipped() {
        let src = "NOEQUALS\nKEY=val";
        let entries = parse(src);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "KEY");
    }

    #[test]
    fn json_array_value_verbatim() {
        let entries = parse(r#"MDS_CORS_ALLOW_ORIGINS=["*"]"#);
        assert_eq!(entries[0].value, r#"["*"]"#);
    }

    #[test]
    fn interpolation_kept_verbatim() {
        let entries = parse("KEY=${OTHER}");
        assert_eq!(entries[0].value, "${OTHER}");
    }

    #[test]
    fn value_with_equals_sign() {
        // Only the first `=` is the separator.
        let entries = parse("KEY=a=b=c");
        assert_eq!(entries[0].value, "a=b=c");
    }

    #[test]
    fn empty_value() {
        let entries = parse("KEY=");
        assert_eq!(entries[0].name, "KEY");
        assert_eq!(entries[0].value, "");
    }

    // -- additional edge cases --

    #[test]
    fn test_parse_dotenv_quoted_value_internal_spaces() {
        // double-quoted value that contains internal spaces keeps them all
        let entries = parse(r#"MY_KEY="hello   world  foo""#);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].value, "hello   world  foo");
    }

    #[test]
    fn test_parse_dotenv_single_quoted_internal_spaces() {
        let entries = parse("MY_KEY='  leading and trailing  '");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].value, "  leading and trailing  ");
    }

    #[test]
    fn test_parse_dotenv_export_with_extra_spaces() {
        // `export   KEY=val` (multiple spaces after `export`)
        let entries = parse("export   KEY=value");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "KEY");
        assert_eq!(entries[0].value, "value");
    }

    #[test]
    fn test_parse_dotenv_crlf_line_endings() {
        // Windows CRLF — `str::lines()` should handle these transparently,
        // but the trimmed value must not contain a stray `\r`.
        let src = "KEY=value\r\nOTHER=second\r\n";
        let entries = parse(src);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "KEY");
        assert_eq!(entries[0].value, "value");
        assert_eq!(entries[1].name, "OTHER");
        assert_eq!(entries[1].value, "second");
    }

    #[test]
    fn test_parse_dotenv_value_with_multiple_equals() {
        // Only the first `=` is the separator; rest stays in value.
        let entries = parse("KEY=a=b=c=d");
        assert_eq!(entries[0].value, "a=b=c=d");
    }

    #[test]
    fn test_parse_dotenv_empty_key_skipped() {
        // `=value` has an empty key and must be skipped entirely.
        let entries = parse("=orphan_value");
        assert!(entries.is_empty(), "empty-key line must be skipped");
    }

    #[test]
    fn test_parse_dotenv_kind_is_dotenv() {
        let entries = parse("KEY=val");
        assert_eq!(entries[0].kind, SourceKind::DotEnv);
    }

    #[test]
    fn test_parse_dotenv_priority_propagated() {
        let entries = DotEnvParser::new(95).parse("KEY=val", Path::new("/a/.env.local"));
        assert_eq!(entries[0].priority, 95);
    }

    #[test]
    fn test_parse_dotenv_source_path_stored() {
        let entries = DotEnvParser::new(60).parse("KEY=val", Path::new("/svc/sample.env"));
        assert!(entries[0].source_path.contains("sample.env"));
    }
}
