// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Config parser for `zerodds-coap-bridged`. Spec §3.

use std::fs;
use std::path::Path;
use std::string::{String, ToString};
use std::vec::Vec;

use super::yaml::{YamlNode, expand_env_vars, parse};

/// Daemon config.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// `domain:`.
    pub domain: i32,
    /// `log_level:`.
    pub log_level: String,
    /// `coap.bind:`.
    pub bind: String,
    /// `coap.bind_dtls:` — L5 stub.
    pub bind_dtls: Option<String>,
    /// `coap.max_message_size:`.
    pub max_message_size: usize,
    /// `coap.ack_timeout_ms:`.
    pub ack_timeout_ms: u32,
    /// `coap.max_retransmit:`.
    pub max_retransmit: u8,
    /// `coap.observe_max_age_secs:`.
    pub observe_max_age_secs: u32,
    /// `topics:`.
    pub topics: Vec<TopicConfig>,
    /// DTLS active (L5 stub) — stays rejected with a clear message.
    pub dtls_enabled: bool,
    /// Spec §7.2 — auth mode `none|bearer|jwt|mtls`.
    pub auth_mode: String,
    /// `auth.bearer_token:`.
    pub auth_bearer_token: Option<String>,
    /// `auth.bearer_subject:`.
    pub auth_bearer_subject: Option<String>,
    /// Spec §7.3 — ACL per CoAP resource path.
    pub topic_acl: std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
    /// `metrics.enabled` — toggles the Prometheus endpoint (§8.2).
    pub metrics_enabled: bool,
    /// `metrics.address` — bind address for catalog/healthz/metrics.
    pub metrics_addr: String,
}

/// Topic entry.
#[derive(Debug, Clone, Default)]
pub struct TopicConfig {
    /// `dds_name:`.
    pub dds_name: String,
    /// `dds_type:`.
    pub dds_type: String,
    /// `coap_uri_path:`.
    pub coap_uri_path: String,
    /// `direction:`.
    pub direction: String,
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
    /// IO.
    Io(String),
    /// Syntax.
    Syntax(String),
    /// Required field missing.
    MissingField(String),
    /// Value not parseable.
    BadValue {
        /// Field.
        field: String,
        /// Value.
        value: String,
    },
}

impl core::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(m) => write!(f, "io: {m}"),
            Self::Syntax(m) => write!(f, "syntax: {m}"),
            Self::MissingField(m) => write!(f, "missing: {m}"),
            Self::BadValue { field, value } => write!(f, "bad value {field}: {value}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self::default_for_dev()
    }
}

impl DaemonConfig {
    /// Default for dev.
    #[must_use]
    pub fn default_for_dev() -> Self {
        Self {
            domain: 0,
            log_level: "info".to_string(),
            bind: "0.0.0.0:5683".to_string(),
            bind_dtls: None,
            max_message_size: 1152,
            ack_timeout_ms: 2000,
            max_retransmit: 4,
            observe_max_age_secs: 60,
            topics: Vec::new(),
            dtls_enabled: false,
            auth_mode: "none".to_string(),
            auth_bearer_token: None,
            auth_bearer_subject: None,
            topic_acl: std::collections::HashMap::new(),
            metrics_enabled: false,
            metrics_addr: String::new(),
        }
    }

    /// Loads from a file.
    ///
    /// # Errors
    /// [`ConfigError`].
    pub fn load_from_file(path: &Path) -> Result<Self, ConfigError> {
        let raw = fs::read_to_string(path).map_err(|e| ConfigError::Io(e.to_string()))?;
        Self::load_from_str(&raw)
    }

    /// Parses from a string.
    ///
    /// # Errors
    /// [`ConfigError`].
    pub fn load_from_str(raw: &str) -> Result<Self, ConfigError> {
        let expanded = expand_env_vars(raw);
        let nodes = parse(&expanded)?;
        let mut out = Self::default_for_dev();
        for (k, v) in nodes.iter() {
            match k.as_str() {
                "domain" => {
                    let s = v.as_scalar()?;
                    out.domain = s.parse().map_err(|_| ConfigError::BadValue {
                        field: "domain".to_string(),
                        value: s,
                    })?;
                }
                "log_level" => out.log_level = v.as_scalar()?,
                "coap" => {
                    if let YamlNode::Map(m) = v {
                        if let Some(YamlNode::Scalar(s)) = m.get("bind") {
                            out.bind = s.clone();
                        }
                        if let Some(YamlNode::Scalar(s)) = m.get("bind_dtls") {
                            out.bind_dtls = Some(s.clone());
                        }
                        if let Some(YamlNode::Scalar(s)) = m.get("max_message_size") {
                            out.max_message_size = s.parse().unwrap_or(1152);
                        }
                        if let Some(YamlNode::Scalar(s)) = m.get("ack_timeout_ms") {
                            out.ack_timeout_ms = s.parse().unwrap_or(2000);
                        }
                        if let Some(YamlNode::Scalar(s)) = m.get("max_retransmit") {
                            out.max_retransmit = s.parse().unwrap_or(4);
                        }
                        if let Some(YamlNode::Scalar(s)) = m.get("observe_max_age_secs") {
                            out.observe_max_age_secs = s.parse().unwrap_or(60);
                        }
                        if let Some(YamlNode::Map(dtls)) = m.get("dtls") {
                            if let Some(YamlNode::Scalar(s)) = dtls.get("enabled") {
                                out.dtls_enabled = parse_bool(s);
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
                                            s.split(',')
                                                .map(|x| x.trim().to_string())
                                                .collect::<Vec<_>>(),
                                        ),
                                        _ => None,
                                    })
                                    .unwrap_or_default();
                                let write = em
                                    .get("write")
                                    .and_then(|n| match n {
                                        YamlNode::Scalar(s) => Some(
                                            s.split(',')
                                                .map(|x| x.trim().to_string())
                                                .collect::<Vec<_>>(),
                                        ),
                                        _ => None,
                                    })
                                    .unwrap_or_default();
                                out.topic_acl.insert(topic.clone(), (read, write));
                            }
                        }
                    }
                }
                "topics" => {
                    if let YamlNode::Seq(items) = v {
                        for item in items {
                            if let YamlNode::Map(m) = item {
                                let mut t = TopicConfig::default();
                                if let Some(YamlNode::Scalar(s)) = m.get("dds_name") {
                                    t.dds_name = s.clone();
                                }
                                if let Some(YamlNode::Scalar(s)) = m.get("dds_type") {
                                    t.dds_type = s.clone();
                                }
                                if let Some(YamlNode::Scalar(s)) = m.get("coap_uri_path") {
                                    t.coap_uri_path = s.clone();
                                }
                                if let Some(YamlNode::Scalar(s)) = m.get("direction") {
                                    t.direction = s.clone();
                                } else {
                                    t.direction = "bidir".to_string();
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
                                if t.dds_name.is_empty() {
                                    return Err(ConfigError::MissingField(
                                        "topics[].dds_name".to_string(),
                                    ));
                                }
                                if t.dds_type.is_empty() {
                                    t.dds_type = t.dds_name.clone();
                                }
                                if t.coap_uri_path.is_empty() {
                                    t.coap_uri_path = default_coap_path(&t.dds_name);
                                }
                                out.topics.push(t);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(out)
    }
}

/// Slug per Spec §5.1: `Chat::Message` → `chat/message`.
#[must_use]
pub fn default_coap_path(topic: &str) -> String {
    let mut buf = String::new();
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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn slug_handles_double_colon() {
        assert_eq!(default_coap_path("Chat::Message"), "chat/message");
    }

    #[test]
    fn slug_replaces_unsafe_chars() {
        assert_eq!(default_coap_path("My Topic!"), "my_topic_");
    }

    #[test]
    fn parses_minimal_config() {
        let yaml = "\
domain: 7
coap:
  bind: 0.0.0.0:5683
  max_message_size: 2048
topics:
  - dds_name: T
    direction: in
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert_eq!(cfg.domain, 7);
        assert_eq!(cfg.bind, "0.0.0.0:5683");
        assert_eq!(cfg.max_message_size, 2048);
        assert_eq!(cfg.topics[0].direction, "in");
    }

    #[test]
    fn rejects_topic_without_name() {
        let yaml = "\
domain: 0
topics:
  - direction: in
";
        assert!(matches!(
            DaemonConfig::load_from_str(yaml).unwrap_err(),
            ConfigError::MissingField(_)
        ));
    }

    #[test]
    fn topic_default_direction_is_bidir() {
        let yaml = "\
domain: 0
topics:
  - dds_name: T
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert_eq!(cfg.topics[0].direction, "bidir");
    }
}
