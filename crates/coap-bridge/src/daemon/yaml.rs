// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! YAML-subset parser for daemon configs. Identical format to
//! `mqtt-bridge::daemon::yaml` and `websocket-bridge::daemon::config`,
//! duplicated independently per vendor spec.

use std::collections::BTreeMap;
use std::env;
use std::string::{String, ToString};
use std::vec::Vec;

use super::config::ConfigError;

/// AST node.
#[derive(Debug, Clone)]
pub enum YamlNode {
    /// Scalar.
    Scalar(String),
    /// Sequence.
    Seq(Vec<YamlNode>),
    /// Map.
    Map(BTreeMap<String, YamlNode>),
}

impl YamlNode {
    /// Returns the scalar.
    ///
    /// # Errors
    /// `Syntax` if not a scalar.
    pub fn as_scalar(&self) -> Result<String, ConfigError> {
        match self {
            Self::Scalar(s) => Ok(s.clone()),
            _ => Err(ConfigError::Syntax("expected scalar".to_string())),
        }
    }
}

/// `${VAR}` and `${VAR:-default}` substitution.
#[must_use]
pub fn expand_env_vars(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '$' && chars[i + 1] == '{' {
            if let Some(end) = chars[i + 2..].iter().position(|&c| c == '}') {
                let inner: String = chars[i + 2..i + 2 + end].iter().collect();
                let (name, default) = match inner.split_once(":-") {
                    Some((n, d)) => (n.to_string(), Some(d.to_string())),
                    None => (inner.clone(), None),
                };
                let value = env::var(&name).ok().or(default).unwrap_or_default();
                out.push_str(&value);
                i += 2 + end + 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Parses a YAML-subset string into a top-level map.
///
/// # Errors
/// `Syntax`.
pub fn parse(raw: &str) -> Result<BTreeMap<String, YamlNode>, ConfigError> {
    let mut lines: Vec<(usize, String)> = Vec::new();
    for line in raw.split('\n') {
        let stripped = strip_comment(line);
        if stripped.trim().is_empty() {
            continue;
        }
        let indent = stripped.chars().take_while(|c| *c == ' ').count();
        let content = stripped[indent..].to_string();
        lines.push((indent, content));
    }
    let (out, _) = parse_block_map(&lines, 0, 0)?;
    Ok(out)
}

fn strip_comment(line: &str) -> String {
    let mut out = String::new();
    let mut in_quote: Option<char> = None;
    for c in line.chars() {
        match in_quote {
            Some(q) => {
                out.push(c);
                if c == q {
                    in_quote = None;
                }
            }
            None => {
                if c == '#' {
                    break;
                }
                if c == '"' || c == '\'' {
                    in_quote = Some(c);
                }
                out.push(c);
            }
        }
    }
    out.trim_end().to_string()
}
/// zerodds-lint: recursion-depth 64 (parse_block_map bounded by AST depth)
fn parse_block_map(
    lines: &[(usize, String)],
    start: usize,
    indent: usize,
) -> Result<(BTreeMap<String, YamlNode>, usize), ConfigError> {
    let mut map = BTreeMap::new();
    let mut i = start;
    while i < lines.len() {
        let (line_indent, content) = &lines[i];
        if *line_indent < indent {
            break;
        }
        if *line_indent > indent {
            return Err(ConfigError::Syntax(format!(
                "unexpected indent at: {content}"
            )));
        }
        if content.starts_with("- ") || content.as_str() == "-" {
            return Err(ConfigError::Syntax(
                "unexpected seq marker in map".to_string(),
            ));
        }
        let (key, value) = match content.split_once(':') {
            Some((k, v)) => (k.trim().to_string(), v.trim().to_string()),
            None => return Err(ConfigError::Syntax(format!("no colon in: {content}"))),
        };
        if !value.is_empty() {
            map.insert(key, YamlNode::Scalar(unquote(&value)));
            i += 1;
        } else {
            i += 1;
            if i >= lines.len() || lines[i].0 <= indent {
                map.insert(key, YamlNode::Scalar(String::new()));
                continue;
            }
            let child_indent = lines[i].0;
            let child_content = &lines[i].1;
            if child_content.starts_with("- ") || child_content.as_str() == "-" {
                let (seq, advanced) = parse_block_seq(lines, i, child_indent)?;
                map.insert(key, YamlNode::Seq(seq));
                i = advanced;
            } else {
                let (sub, advanced) = parse_block_map(lines, i, child_indent)?;
                map.insert(key, YamlNode::Map(sub));
                i = advanced;
            }
        }
    }
    Ok((map, i))
}
/// zerodds-lint: recursion-depth 64 (parse_block_seq bounded by AST depth)
fn parse_block_seq(
    lines: &[(usize, String)],
    start: usize,
    indent: usize,
) -> Result<(Vec<YamlNode>, usize), ConfigError> {
    let mut seq = Vec::new();
    let mut i = start;
    while i < lines.len() {
        let (line_indent, content) = &lines[i];
        if *line_indent < indent {
            break;
        }
        if *line_indent > indent {
            return Err(ConfigError::Syntax("seq misindent".to_string()));
        }
        if !content.starts_with('-') {
            break;
        }
        let after_dash = if content == "-" {
            String::new()
        } else if content.starts_with("- ") {
            content[2..].to_string()
        } else {
            return Err(ConfigError::Syntax("malformed seq item".to_string()));
        };
        if after_dash.is_empty() {
            i += 1;
            if i >= lines.len() || lines[i].0 <= indent {
                seq.push(YamlNode::Scalar(String::new()));
                continue;
            }
            let child_indent = lines[i].0;
            let (sub, advanced) = parse_block_map(lines, i, child_indent)?;
            seq.push(YamlNode::Map(sub));
            i = advanced;
        } else if let Some((k, v)) = after_dash.split_once(':') {
            let k = k.trim().to_string();
            let v = v.trim();
            let mut sub = BTreeMap::new();
            if v.is_empty() {
                i += 1;
                if i >= lines.len() {
                    sub.insert(k, YamlNode::Scalar(String::new()));
                } else if lines[i].0 > indent + 2 {
                    let ci = lines[i].0;
                    let child = &lines[i].1;
                    if child.starts_with("- ") || child == "-" {
                        let (s2, advanced) = parse_block_seq(lines, i, ci)?;
                        sub.insert(k, YamlNode::Seq(s2));
                        i = advanced;
                    } else {
                        let (m2, advanced) = parse_block_map(lines, i, ci)?;
                        sub.insert(k, YamlNode::Map(m2));
                        i = advanced;
                    }
                } else {
                    sub.insert(k, YamlNode::Scalar(String::new()));
                }
            } else {
                sub.insert(k, YamlNode::Scalar(unquote(v)));
                i += 1;
            }
            let item_member_indent = indent + 2;
            while i < lines.len() {
                let (li, lc) = &lines[i];
                if *li < item_member_indent {
                    break;
                }
                if *li == indent && (lc.starts_with("- ") || lc == "-") {
                    break;
                }
                if *li != item_member_indent {
                    break;
                }
                if lc.starts_with("- ") {
                    break;
                }
                let (kk, vv) = lc
                    .split_once(':')
                    .ok_or_else(|| ConfigError::Syntax("seq map missing colon".to_string()))?;
                let kk = kk.trim().to_string();
                let vv = vv.trim();
                if vv.is_empty() {
                    i += 1;
                    if i < lines.len() && lines[i].0 > item_member_indent {
                        let ci = lines[i].0;
                        let child = &lines[i].1;
                        if child.starts_with("- ") || child == "-" {
                            let (s2, advanced) = parse_block_seq(lines, i, ci)?;
                            sub.insert(kk, YamlNode::Seq(s2));
                            i = advanced;
                        } else {
                            let (m2, advanced) = parse_block_map(lines, i, ci)?;
                            sub.insert(kk, YamlNode::Map(m2));
                            i = advanced;
                        }
                    } else {
                        sub.insert(kk, YamlNode::Scalar(String::new()));
                    }
                } else {
                    sub.insert(kk, YamlNode::Scalar(unquote(vv)));
                    i += 1;
                }
            }
            seq.push(YamlNode::Map(sub));
        } else {
            seq.push(YamlNode::Scalar(unquote(&after_dash)));
            i += 1;
        }
    }
    Ok((seq, i))
}

fn unquote(v: &str) -> String {
    let v = v.trim();
    if (v.starts_with('"') && v.ends_with('"') && v.len() >= 2)
        || (v.starts_with('\'') && v.ends_with('\'') && v.len() >= 2)
    {
        v[1..v.len() - 1].to_string()
    } else {
        v.to_string()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parses_flat_map() {
        let m = parse("a: 1\nb: foo").unwrap();
        assert!(matches!(m.get("a"), Some(YamlNode::Scalar(s)) if s == "1"));
    }

    #[test]
    fn parses_nested_map() {
        let m = parse("a:\n  b: 1").unwrap();
        if let Some(YamlNode::Map(_)) = m.get("a") {
        } else {
            panic!("expected map");
        }
    }

    #[test]
    fn parses_seq_of_maps() {
        let m = parse("topics:\n  - name: a\n  - name: b").unwrap();
        if let Some(YamlNode::Seq(items)) = m.get("topics") {
            assert_eq!(items.len(), 2);
        } else {
            panic!("expected seq");
        }
    }

    #[test]
    fn strips_comments() {
        let m = parse("a: 1 # c\nb: 2").unwrap();
        assert!(matches!(m.get("a"), Some(YamlNode::Scalar(s)) if s == "1"));
    }

    #[test]
    fn env_substitution() {
        let s = expand_env_vars("v: ${ZERODDS_FAKE_xyz_pqr:-fb}");
        assert!(s.contains("fb"));
    }
}
