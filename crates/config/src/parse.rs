// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The YAML subset the daemon configs already used, with line numbers kept.
//!
//! Three crates carried this parser in three copies and each header said it
//! was identical to the other two. This is that parser, once, with two
//! additions the copies did not have: every scalar remembers the line it came
//! from, and every construct outside the subset is refused by name instead of
//! being read as something it is not.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::{ConfigError, Node, Origin};

/// Expands `${VAR}` and `${VAR:-default}` against the process environment.
///
/// An unset variable with no default expands to nothing, which is what the
/// three copies did and what a config that names an optional path expects.
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
                    None => (inner, None),
                };
                let value = std::env::var(&name).ok().or(default).unwrap_or_default();
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

/// One physical line, already stripped of its comment and measured.
#[derive(Clone)]
struct Line {
    indent: usize,
    text: String,
    number: usize,
}

/// Reads `text` as the configuration subset.
///
/// `path` names the file in every [`Origin`] and every error this produces —
/// it is what the reader will look at, so it is passed rather than derived.
///
/// # Errors
/// [`ConfigError::Syntax`] with the line, for anything outside the subset.
pub fn parse_str(text: &str, path: &str) -> Result<Node, ConfigError> {
    let lines = significant_lines(text, path)?;
    if lines.is_empty() {
        return Ok(Node::Map(BTreeMap::new()));
    }
    let mut at = 0usize;
    let node = parse_block(&lines, &mut at, lines[0].indent, path, 0)?;
    Ok(node)
}

/// Drops comments and blank lines, measures indentation, refuses what the
/// subset does not cover.
fn significant_lines(text: &str, path: &str) -> Result<Vec<Line>, ConfigError> {
    let mut out = Vec::new();
    for (idx, raw) in text.lines().enumerate() {
        let number = idx + 1;
        if raw.contains('\t') {
            return Err(ConfigError::Syntax {
                path: path.to_string(),
                line: number,
                message: "tab in indentation; this reader takes spaces".to_string(),
            });
        }
        let without_comment = strip_comment(raw);
        let trimmed = without_comment.trim_end();
        if trimmed.trim().is_empty() {
            continue;
        }
        let body = trimmed.trim_start();
        if let Some(unsupported) = unsupported_construct(body) {
            return Err(ConfigError::Syntax {
                path: path.to_string(),
                line: number,
                message: unsupported,
            });
        }
        out.push(Line {
            indent: trimmed.len() - body.len(),
            text: body.to_string(),
            number,
        });
    }
    Ok(out)
}

/// `#` starts a comment unless it is inside quotes.
fn strip_comment(raw: &str) -> &str {
    let bytes = raw.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    for (i, b) in bytes.iter().enumerate() {
        match b {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b'#' if !in_single && !in_double => return &raw[..i],
            _ => {}
        }
    }
    raw
}

/// Names a construct this reader does not implement, so it can say so.
///
/// The three copies read `key: [a, b]` as the literal string `[a, b]` and a
/// `*alias` as the literal `*alias`. That is worse than refusing: the value
/// looks accepted and is wrong at the far end.
fn unsupported_construct(body: &str) -> Option<String> {
    let value = body.split_once(": ").map(|(_, v)| v.trim()).unwrap_or("");
    if body.starts_with("---") || body.starts_with("...") {
        return Some("multi-document streams are not read here".to_string());
    }
    if body.starts_with('&') || value.starts_with('&') {
        return Some("anchors (`&name`) are not read here".to_string());
    }
    if body.starts_with('*') || value.starts_with('*') {
        return Some("aliases (`*name`) are not read here".to_string());
    }
    if value.starts_with('{') && flow_map(value).is_none() {
        return Some(
            "only a flat map of scalars (`{k: v, k2: \"w\"}`) with distinct keys is read in \
             flow form; use block form"
                .to_string(),
        );
    }
    if value.starts_with('[') && flow_list(value).is_none() {
        return Some(
            "only a flat list of scalars (`[a, \"b\"]`) is read in flow form; use block form"
                .to_string(),
        );
    }
    if value == "|" || value == ">" || value.starts_with("|-") || value.starts_with(">-") {
        return Some("block scalars (`|`, `>`) are not read here".to_string());
    }
    None
}

/// Parses every line at `indent`, descending where a line opens a block.
///
/// `depth` is the nesting level, refused past [`crate::MAX_NESTING_DEPTH`]:
/// this recurses once per indentation level of a file somebody else wrote,
/// and a file is not allowed to decide how much stack it gets.
///
/// zerodds-lint: recursion-depth 32
fn parse_block(
    lines: &[Line],
    at: &mut usize,
    indent: usize,
    path: &str,
    depth: usize,
) -> Result<Node, ConfigError> {
    if depth > crate::MAX_NESTING_DEPTH {
        return Err(ConfigError::TooDeep(crate::MAX_NESTING_DEPTH));
    }
    if lines.get(*at).is_some_and(|l| l.text.starts_with("- ")) {
        return parse_seq(lines, at, indent, path, depth);
    }
    let mut map = BTreeMap::new();
    while let Some(line) = lines.get(*at) {
        if line.indent < indent {
            break;
        }
        if line.indent > indent {
            return Err(ConfigError::Syntax {
                path: path.to_string(),
                line: line.number,
                message: "indented further than the key it belongs to".to_string(),
            });
        }
        let (key, rest) = split_key(&line.text).ok_or_else(|| ConfigError::Syntax {
            path: path.to_string(),
            line: line.number,
            message: "expected `key: value` or `key:`".to_string(),
        })?;
        let number = line.number;
        *at += 1;
        if rest.is_empty() {
            // A block follows, or the key is deliberately empty.
            let deeper = lines.get(*at).map_or(0, |l| l.indent);
            if deeper > indent {
                map.insert(key, parse_block(lines, at, deeper, path, depth + 1)?);
            } else {
                map.insert(
                    key,
                    Node::Scalar(
                        String::new(),
                        Origin::File {
                            path: path.to_string(),
                            line: number,
                        },
                    ),
                );
            }
        } else {
            map.insert(key, value_node(rest, path, number));
        }
    }
    Ok(Node::Map(map))
}

/// A `key: value` value: a flat flow list becomes a sequence, a flat flow
/// map a map, anything else a scalar. [`unsupported_construct`] has already
/// refused the other flow forms.
fn value_node(rest: &str, path: &str, line: usize) -> Node {
    let origin = Origin::File {
        path: path.to_string(),
        line,
    };
    if let Some(entries) = flow_map(rest) {
        return Node::Map(
            entries
                .into_iter()
                .map(|(k, v)| {
                    (
                        k,
                        Node::Scalar(unquote(&expand_env_vars(&v)), origin.clone()),
                    )
                })
                .collect(),
        );
    }
    match flow_list(rest) {
        Some(items) => Node::Seq(
            items
                .into_iter()
                .map(|item| Node::Scalar(unquote(&expand_env_vars(&item)), origin.clone()))
                .collect(),
        ),
        None => Node::Scalar(unquote(&expand_env_vars(rest)), origin),
    }
}

/// `{kind: keep_last, depth: 10}` as its entries, or `None` when the text is
/// not a flat map of scalars: no nested `[`/`{`, balanced quotes, every
/// entry `key: value`, no key twice, a closing `}` last.
fn flow_map(value: &str) -> Option<Vec<(String, String)>> {
    let inner = value.trim().strip_prefix('{')?.strip_suffix('}')?;
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    let items = split_flow_items(inner)?;
    let mut out: Vec<(String, String)> = Vec::new();
    for item in items {
        let (k, v) = item.split_once(':')?;
        let key = unquote(k.trim());
        let v = v.trim();
        if key.is_empty() || v.is_empty() || out.iter().any(|(existing, _)| *existing == key) {
            return None;
        }
        out.push((key, v.to_string()));
    }
    Some(out)
}

/// The comma-separated items of a flow collection's inside, quotes kept;
/// `None` for a nested collection, an open quote or an empty item.
fn split_flow_items(inner: &str) -> Option<Vec<String>> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for c in inner.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => {
                quote = None;
                current.push(c);
            }
            (Some(_), c) => current.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                current.push(c);
            }
            (None, '[' | ']' | '{' | '}') => return None,
            (None, ',') => items.push(core::mem::take(&mut current).trim().to_string()),
            (None, c) => current.push(c),
        }
    }
    if quote.is_some() {
        return None;
    }
    let last = current.trim().to_string();
    if !last.is_empty() || !items.is_empty() {
        items.push(last);
    }
    if items.iter().any(String::is_empty) {
        return None;
    }
    Some(items)
}

/// `[a, "b, c", 'd']` as its items, or `None` when the text is not a flat
/// list of scalars: no nested `[`/`{`, balanced quotes, a closing `]` last.
fn flow_list(value: &str) -> Option<Vec<String>> {
    let inner = value.trim().strip_prefix('[')?.strip_suffix(']')?;
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    split_flow_items(inner)
}

/// Parses `- item` lines at `indent`.
///
/// An item that starts with `key: …` is a map, read as a whole block: its
/// first key sits two columns past the dash and the item's further lines
/// are indented past the dash, so `- name: a` followed by `  qos:` and a
/// deeper `    depth: 5` nests as written. An item without a key is a
/// scalar and may not continue on deeper lines.
///
/// zerodds-lint: recursion-depth 32
fn parse_seq(
    lines: &[Line],
    at: &mut usize,
    indent: usize,
    path: &str,
    depth: usize,
) -> Result<Node, ConfigError> {
    let mut items = Vec::new();
    while let Some(line) = lines.get(*at) {
        if line.indent != indent || !line.text.starts_with("- ") {
            break;
        }
        let item = line.text[2..].trim();
        let number = line.number;
        *at += 1;
        let mut block: Vec<Line> = Vec::new();
        while let Some(next) = lines.get(*at) {
            if next.indent <= indent {
                break;
            }
            block.push(next.clone());
            *at += 1;
        }
        if item.starts_with("- ") {
            return Err(ConfigError::Syntax {
                path: path.to_string(),
                line: number,
                message: "a sequence directly inside a sequence item is not read here".to_string(),
            });
        }
        if split_key(item).is_some() {
            let mut item_lines = Vec::with_capacity(block.len() + 1);
            item_lines.push(Line {
                indent: indent + 2,
                text: item.to_string(),
                number,
            });
            item_lines.extend(block);
            let mut i = 0;
            let node = parse_block(&item_lines, &mut i, indent + 2, path, depth + 1)?;
            if let Some(rest) = item_lines.get(i) {
                return Err(ConfigError::Syntax {
                    path: path.to_string(),
                    line: rest.number,
                    message: "not aligned with the keys of its sequence item".to_string(),
                });
            }
            items.push(node);
        } else {
            if let Some(next) = block.first() {
                return Err(ConfigError::Syntax {
                    path: path.to_string(),
                    line: next.number,
                    message: "a scalar sequence item does not continue on deeper lines".to_string(),
                });
            }
            items.push(value_node(item, path, number));
        }
    }
    Ok(Node::Seq(items))
}

/// Splits `key: value` and `key:`; `None` when there is no key at all.
fn split_key(text: &str) -> Option<(String, &str)> {
    if let Some((k, v)) = text.split_once(": ") {
        return Some((k.trim().to_string(), v.trim()));
    }
    let stripped = text.strip_suffix(':')?;
    Some((stripped.trim().to_string(), ""))
}

/// Removes one matching pair of surrounding quotes.
fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}
