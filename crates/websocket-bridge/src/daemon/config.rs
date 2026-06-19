// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Config file parser for `zerodds-ws-bridged`.
//!
//! Spec: `zerodds-ws-bridge-1.0.md` §3.
//!
//! YAML subset (no external parser in the workspace):
//!
//! * Top-level mapping (key-value).
//! * Nested mappings via indent (2 spaces).
//! * Sequences via `- ` prefix with indent.
//! * Scalars: strings (with/without quotes), integers, bool (`true`/`false`).
//! * `#` comments up to EOL.
//! * `${VAR}` and `${VAR:-default}` env substitution before the parse.
//!
//! Deliberately not a generic YAML parser — the spec subset is
//! explicit; anything outside it is rejected with `ConfigError::Syntax`.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;
use std::string::{String, ToString};
use std::vec::Vec;

/// Parsed daemon config.
#[derive(Debug, Clone, Default)]
pub struct DaemonConfig {
    /// `listen: <addr>` — bind address.
    pub listen: String,
    /// `domain: <id>` — DDS domain id.
    pub domain: i32,
    /// `log_level: <level>`.
    pub log_level: String,
    /// `topics:` list.
    pub topics: Vec<TopicConfig>,
    /// `tls.enabled` — if true, `tls_cert_file`+`tls_key_file` must
    /// be set. Spec §7.1.
    pub tls_enabled: bool,
    /// `tls.cert_file` — PEM cert path.
    pub tls_cert_file: String,
    /// `tls.key_file` — PEM key path.
    pub tls_key_file: String,
    /// `tls.client_ca_file` — PEM CA bundle for mTLS client auth.
    pub tls_client_ca_file: String,
    /// `auth.mode` — `none|bearer|jwt|mtls|sasl`. Spec §7.2.
    pub auth_mode: String,
    /// `auth.bearer_token` — single-token form (map with one entry).
    pub auth_bearer_token: Option<String>,
    /// `auth.bearer_token_subject` — who is behind the bearer.
    pub auth_bearer_subject: Option<String>,
    /// Topic ACL: `topic → ("read,write" CSV of subjects)`. Spec §7.3.
    pub topic_acl: std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
    /// `metrics.enabled` — toggles the Prometheus endpoint (§8.2).
    pub metrics_enabled: bool,
    /// Bind address for the admin endpoint (`/metrics`, `/catalog`,
    /// `/healthz`). If empty but `metrics_enabled=true`: default
    /// `127.0.0.1:9090`. Overridable via CLI/`metrics.address`.
    pub metrics_addr: String,
}

/// Single topic map entry.
#[derive(Debug, Clone, Default)]
pub struct TopicConfig {
    /// `name:` — DDS topic name.
    pub name: String,
    /// `type:` — DDS type name.
    pub type_name: String,
    /// `direction:` — `in|out|bidir`.
    pub direction: String,
    /// `ws_path:` — override URL path.
    pub ws_path: String,
    /// `qos.reliability:`.
    pub reliability: String,
    /// `qos.durability:`.
    pub durability: String,
    /// `qos.history.depth:`.
    pub history_depth: i32,
}

/// Config error.
#[derive(Debug, Clone)]
pub enum ConfigError {
    /// File I/O error.
    Io(String),
    /// YAML syntax error.
    Syntax(String),
    /// A required field is missing.
    MissingField(String),
    /// Value type mismatch.
    BadValue {
        /// Field name.
        field: String,
        /// Raw value.
        value: String,
    },
}

impl core::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(m) => write!(f, "config io: {m}"),
            Self::Syntax(m) => write!(f, "config syntax: {m}"),
            Self::MissingField(m) => write!(f, "config missing field: {m}"),
            Self::BadValue { field, value } => {
                write!(f, "config bad value for {field}: {value}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

impl DaemonConfig {
    /// Default config (when neither a file nor a CLI override is set).
    #[must_use]
    pub fn default_for_dev() -> Self {
        Self {
            listen: "127.0.0.1:8080".to_string(),
            domain: 0,
            log_level: "info".to_string(),
            topics: Vec::new(),
            tls_enabled: false,
            tls_cert_file: String::new(),
            tls_key_file: String::new(),
            tls_client_ca_file: String::new(),
            auth_mode: "none".to_string(),
            auth_bearer_token: None,
            auth_bearer_subject: None,
            topic_acl: std::collections::HashMap::new(),
            metrics_enabled: false,
            metrics_addr: String::new(),
        }
    }

    /// Loads + parses a config from a file.
    ///
    /// # Errors
    /// `Io` on a read error, `Syntax`/`MissingField`/`BadValue` on
    /// malformed YAML.
    pub fn load_from_file(path: &Path) -> Result<Self, ConfigError> {
        let raw = fs::read_to_string(path).map_err(|e| ConfigError::Io(e.to_string()))?;
        Self::load_from_str(&raw)
    }

    /// Parses a config from a YAML string. Public for tests.
    ///
    /// # Errors
    /// See [`ConfigError`].
    pub fn load_from_str(raw: &str) -> Result<Self, ConfigError> {
        let expanded = expand_env_vars(raw);
        let nodes = parse_yaml_subset(&expanded)?;
        let mut out = Self::default_for_dev();
        for (k, v) in nodes.iter() {
            match k.as_str() {
                "listen" => out.listen = v.as_scalar()?,
                "domain" => {
                    let s = v.as_scalar()?;
                    out.domain = s.parse().map_err(|_| ConfigError::BadValue {
                        field: "domain".to_string(),
                        value: s,
                    })?;
                }
                "log_level" => out.log_level = v.as_scalar()?,
                "tls" => {
                    if let YamlNode::Map(m) = v {
                        if let Some(YamlNode::Scalar(s)) = m.get("enabled") {
                            out.tls_enabled = parse_bool(s);
                        }
                        if let Some(YamlNode::Scalar(s)) = m.get("cert_file") {
                            out.tls_cert_file = s.clone();
                        }
                        if let Some(YamlNode::Scalar(s)) = m.get("key_file") {
                            out.tls_key_file = s.clone();
                        }
                        if let Some(YamlNode::Scalar(s)) = m.get("client_ca_file") {
                            out.tls_client_ca_file = s.clone();
                        }
                    }
                }
                "auth" => {
                    if let YamlNode::Map(m) = v {
                        if let Some(YamlNode::Scalar(s)) = m.get("mode") {
                            out.auth_mode = s.clone();
                        }
                        if let Some(YamlNode::Scalar(s)) = m.get("bearer_token") {
                            out.auth_bearer_token = Some(s.clone());
                        }
                        if let Some(YamlNode::Scalar(s)) = m.get("bearer_subject") {
                            out.auth_bearer_subject = Some(s.clone());
                        }
                    }
                }
                "acl" => {
                    if let YamlNode::Map(m) = v {
                        for (topic, entry) in m.iter() {
                            if let YamlNode::Map(em) = entry {
                                let read = em
                                    .get("read")
                                    .and_then(|n| match n {
                                        YamlNode::Scalar(s) => Some(
                                            s.split(',').map(|x| x.trim().to_string()).collect(),
                                        ),
                                        _ => None,
                                    })
                                    .unwrap_or_default();
                                let write = em
                                    .get("write")
                                    .and_then(|n| match n {
                                        YamlNode::Scalar(s) => Some(
                                            s.split(',').map(|x| x.trim().to_string()).collect(),
                                        ),
                                        _ => None,
                                    })
                                    .unwrap_or_default();
                                out.topic_acl.insert(topic.clone(), (read, write));
                            }
                        }
                    }
                }
                "metrics" => {
                    if let YamlNode::Map(m) = v {
                        if let Some(YamlNode::Scalar(s)) = m.get("enabled") {
                            out.metrics_enabled = parse_bool(s);
                        }
                        if let Some(YamlNode::Scalar(s)) = m.get("address") {
                            out.metrics_addr = s.clone();
                        }
                    }
                }
                "topics" => {
                    if let YamlNode::Seq(items) = v {
                        for item in items.iter() {
                            if let YamlNode::Map(m) = item {
                                let mut t = TopicConfig::default();
                                if let Some(YamlNode::Scalar(s)) = m.get("name") {
                                    t.name = s.clone();
                                }
                                if let Some(YamlNode::Scalar(s)) = m.get("type") {
                                    t.type_name = s.clone();
                                }
                                if let Some(YamlNode::Scalar(s)) = m.get("direction") {
                                    t.direction = s.clone();
                                } else {
                                    t.direction = "bidir".to_string();
                                }
                                if let Some(YamlNode::Scalar(s)) = m.get("ws_path") {
                                    t.ws_path = s.clone();
                                }
                                if let Some(YamlNode::Map(qm)) = m.get("qos") {
                                    if let Some(YamlNode::Scalar(s)) = qm.get("reliability") {
                                        t.reliability = s.clone();
                                    }
                                    if let Some(YamlNode::Scalar(s)) = qm.get("durability") {
                                        t.durability = s.clone();
                                    }
                                    if let Some(YamlNode::Map(hm)) = qm.get("history") {
                                        if let Some(YamlNode::Scalar(s)) = hm.get("depth") {
                                            t.history_depth = s.parse().unwrap_or(10);
                                        }
                                    }
                                }
                                if t.name.is_empty() {
                                    return Err(ConfigError::MissingField(
                                        "topics[].name".to_string(),
                                    ));
                                }
                                if t.type_name.is_empty() {
                                    t.type_name = t.name.clone();
                                }
                                if t.ws_path.is_empty() {
                                    t.ws_path = default_ws_path(&t.name);
                                }
                                out.topics.push(t);
                            }
                        }
                    }
                }
                _ => {
                    // Unknown top-level keys are NOT treated as an error
                    // (forward compatibility), but made visible via a stderr WARN,
                    // so that typos or doc drift (e.g. outdated `participant:` /
                    // `websocket:` / `routes:` / `observability:` sections) do not
                    // silently lead to default values.
                    eprintln!(
                        "[zerodds-ws-bridged config] WARN: unknown top-level key {:?} ignored \
                         (typo or schema drift? expected one of: listen, domain, log_level, \
                         tls, auth, acl, metrics, topics — see docs/specs/zerodds-ws-bridge-1.0.md §3)",
                        k
                    );
                }
            }
        }
        Ok(out)
    }
}

/// Slug algorithm per Spec §5.1: `Chat::Message` → `/topics/chat/message`.
#[must_use]
pub fn default_ws_path(topic: &str) -> String {
    let mut buf = String::from("/topics/");
    let lower = topic.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b':' && bytes[i + 1] == b':' {
            buf.push('/');
            i += 2;
            continue;
        }
        let c = bytes[i] as char;
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '/' {
            buf.push(c);
        } else {
            buf.push('_');
        }
        i += 1;
    }
    buf
}

fn parse_bool(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "yes" | "1")
}

/// `${VAR}` and `${VAR:-default}` substitution.
#[must_use]
pub fn expand_env_vars(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '$' && chars[i + 1] == '{' {
            // Find closing `}`.
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

/// YAML subset AST.
#[derive(Debug, Clone)]
enum YamlNode {
    Scalar(String),
    Seq(Vec<YamlNode>),
    Map(BTreeMap<String, YamlNode>),
}

impl YamlNode {
    fn as_scalar(&self) -> Result<String, ConfigError> {
        match self {
            Self::Scalar(s) => Ok(s.clone()),
            _ => Err(ConfigError::Syntax("expected scalar".to_string())),
        }
    }
}

/// Mini YAML parser. Processes only the spec subset.
fn parse_yaml_subset(raw: &str) -> Result<BTreeMap<String, YamlNode>, ConfigError> {
    // Tokenize: `(indent, content)` per line.
    let mut lines: Vec<(usize, String)> = Vec::new();
    for line in raw.split('\n') {
        // Strip `#` comments (outside of quotes).
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
    // Trim trailing whitespace.
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
            return Err(ConfigError::Syntax(alloc_format(format_args!(
                "unexpected indent at line containing {content}"
            ))));
        }
        if content.starts_with("- ") || content.as_str() == "-" {
            // We are inside a map but encountered a sequence-marker.
            return Err(ConfigError::Syntax(
                "unexpected sequence marker in map context".to_string(),
            ));
        }
        let (key, value) = match content.split_once(':') {
            Some((k, v)) => (k.trim().to_string(), v.trim().to_string()),
            None => {
                return Err(ConfigError::Syntax(alloc_format(format_args!(
                    "no `:` in line: {content}"
                ))));
            }
        };
        if !value.is_empty() {
            // Inline scalar.
            map.insert(key, YamlNode::Scalar(unquote(&value)));
            i += 1;
        } else {
            // Block child at the next-deeper indent.
            i += 1;
            // The next non-empty line determines the format.
            if i >= lines.len() || lines[i].0 <= indent {
                // Empty body — as an empty scalar.
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
            return Err(ConfigError::Syntax("seq misindented".to_string()));
        }
        if !content.starts_with('-') {
            break;
        }
        // `- key: value` form vs `-` block child on the next line
        let after_dash = if content == "-" {
            String::new()
        } else if content.starts_with("- ") {
            content[2..].to_string()
        } else {
            return Err(ConfigError::Syntax("malformed seq item".to_string()));
        };
        if after_dash.is_empty() {
            // Item body on the next line.
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
            // Collect: first entry inline + following lines with `child_indent =
            // indent + 2` as map members.
            let mut sub = BTreeMap::new();
            if v.is_empty() {
                // Block child for the first key on the next line.
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
            // Collect further members of this map item: indent must be
            // > the seq indent, exactly = indent + 2.
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
            // Inline scalar.
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

fn alloc_format(args: core::fmt::Arguments<'_>) -> String {
    use core::fmt::Write as _;
    let mut s = String::new();
    let _ = s.write_fmt(args);
    s
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn slug_strips_double_colon() {
        assert_eq!(default_ws_path("Chat::Message"), "/topics/chat/message");
    }

    #[test]
    fn unknown_top_level_keys_do_not_fail_parse() {
        // Forward compatibility: unknown top-level keys (typos,
        // outdated schema drift like `participant:` / `websocket:` / `routes:`
        // / `observability:` from the old yaml example) must NOT abort the
        // parse as a ConfigError — they are made visible via a stderr WARN
        // and the daemon boots with defaults for the missing known keys.
        // Changed semantics (e.g. strict rejection in the future)
        // deliberately break this test.
        let yaml = "\
participant:
  domain_id: 7
websocket:
  bind: \"0.0.0.0:8080\"
typo_listen: \"1.2.3.4:9999\"
listen: \"0.0.0.0:1234\"
";
        let cfg = DaemonConfig::load_from_str(yaml).expect("must not error on unknown keys");
        // The known key takes effect, everything under unknown is ignored.
        assert_eq!(cfg.listen, "0.0.0.0:1234");
        assert_eq!(cfg.domain, 0); // participant.domain_id was NOT mapped
    }

    #[test]
    fn slug_replaces_unsafe_chars() {
        assert_eq!(default_ws_path("My Topic!"), "/topics/my_topic_");
    }

    #[test]
    fn env_substitution_with_default() {
        // Test with a guaranteed-unset variable name
        // (UUID-style, so we do not rely on process state).
        let s = expand_env_vars("token: ${ZERODDS_PROBABLY_UNSET_VAR_e2afb0b9_test:-fallback}");
        assert!(s.contains("fallback"), "got: {s}");
    }

    #[test]
    fn env_substitution_passthrough_when_no_placeholder() {
        let s = expand_env_vars("plain: value");
        assert_eq!(s, "plain: value");
    }

    #[test]
    fn parse_minimal_config() {
        let yaml = "\
listen: \"0.0.0.0:8080\"
domain: 0
log_level: info
topics:
  - name: \"Chat::Message\"
    type: \"Chat::Message\"
    direction: bidir
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert_eq!(cfg.listen, "0.0.0.0:8080");
        assert_eq!(cfg.domain, 0);
        assert_eq!(cfg.topics.len(), 1);
        assert_eq!(cfg.topics[0].name, "Chat::Message");
        assert_eq!(cfg.topics[0].direction, "bidir");
        assert_eq!(cfg.topics[0].ws_path, "/topics/chat/message");
    }

    #[test]
    fn parse_qos_block() {
        let yaml = "\
listen: 0.0.0.0:8080
domain: 0
topics:
  - name: T
    qos:
      reliability: reliable
      durability: volatile
      history:
        depth: 25
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert_eq!(cfg.topics[0].reliability, "reliable");
        assert_eq!(cfg.topics[0].durability, "volatile");
        assert_eq!(cfg.topics[0].history_depth, 25);
    }

    #[test]
    fn parse_tls_and_auth_blocks() {
        let yaml = "\
listen: 0.0.0.0:8080
domain: 0
tls:
  enabled: true
auth:
  mode: bearer
  bearer_token: secret
metrics:
  enabled: true
topics:
  - name: T
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert!(cfg.tls_enabled);
        assert_eq!(cfg.auth_mode, "bearer");
        assert_eq!(cfg.auth_bearer_token.as_deref(), Some("secret"));
        assert!(cfg.metrics_enabled);
    }

    #[test]
    fn parse_rejects_bad_domain() {
        let yaml = "\
listen: x
domain: notanint
";
        let err = DaemonConfig::load_from_str(yaml).unwrap_err();
        assert!(matches!(err, ConfigError::BadValue { .. }));
    }
}
