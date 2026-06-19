// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Config for `zerodds-mqtt-bridged`. Spec §3.
//!
//! YAML subset, same format as websocket-bridge::daemon::config —
//! duplicated standalone, because the daemons should not share a
//! common helper crate (vendor-spec independence).

use std::fs;
use std::path::Path;
use std::string::{String, ToString};
use std::vec::Vec;

/// Top-Level-Config.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// `domain:`.
    pub domain: i32,
    /// `log_level:`.
    pub log_level: String,
    /// `mqtt.broker_url:`.
    pub broker_url: String,
    /// `mqtt.client_id:`.
    pub client_id: String,
    /// `mqtt.username:`.
    pub username: Option<String>,
    /// `mqtt.password:`.
    pub password: Option<String>,
    /// `mqtt.keep_alive_secs:`.
    pub keep_alive_secs: u16,
    /// `mqtt.clean_start:`.
    pub clean_start: bool,
    /// `topics:`.
    pub topics: Vec<TopicConfig>,
    /// TLS active (L5 stub — legacy flag, kept for backward compat).
    pub tls_enabled: bool,
    /// Spec §7.1 — TLS active outbound to the broker (`mqtts://`).
    pub broker_tls_enabled: bool,
    /// `mqtt.tls.ca_file:` — PEM CA for broker-cert validation.
    pub broker_tls_ca_file: String,
    /// `mqtt.tls.client_cert_file:` — client cert (mTLS).
    pub broker_tls_client_cert_file: String,
    /// `mqtt.tls.client_key_file:` — client key (mTLS).
    pub broker_tls_client_key_file: String,
    /// `mqtt.tls.server_name:` — hostname override for SNI/validation.
    pub broker_tls_server_name: String,
    /// Spec §7.2 — auth mode of the bridge towards the broker:
    /// `none|bearer|sasl|sasl_plain|mtls`.
    pub auth_mode: String,
    /// `auth.bearer_token:` — token sent out as the CONNECT password.
    pub auth_bearer_token: Option<String>,
    /// `auth.bearer_subject:` — local subject name for ACL auth.
    pub auth_bearer_subject: Option<String>,
    /// `auth.username:` — outbound CONNECT username (SASL-PLAIN).
    pub outbound_username: Option<String>,
    /// `auth.password:` — outbound CONNECT password (SASL-PLAIN).
    pub outbound_password: Option<String>,
    /// `auth.sasl_users:` — user/pass map (CSV `u:p,u2:p2` in the YAML).
    /// Used in the daemon-internal ACL path for validation.
    pub sasl_users: std::collections::HashMap<String, String>,
    /// Spec §7.3 — topic ACL `topic → (read subjects, write subjects)`.
    pub topic_acl: std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
    /// `metrics.enabled` — toggles the Prometheus endpoint (§8.2).
    pub metrics_enabled: bool,
    /// `metrics.address`. Default `127.0.0.1:9090`
    /// if `metrics_enabled=true` and the address is empty.
    pub metrics_addr: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self::default_for_dev()
    }
}

/// Per-topic entry.
#[derive(Debug, Clone, Default)]
pub struct TopicConfig {
    /// `dds_name:`.
    pub dds_name: String,
    /// `dds_type:`.
    pub dds_type: String,
    /// `mqtt_topic:`.
    pub mqtt_topic: String,
    /// `direction:`.
    pub direction: String,
    /// `mqtt_qos:` (0/1/2).
    pub mqtt_qos: u8,
    /// `retain:`.
    pub retain: bool,
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
    /// Value type.
    BadValue {
        /// Field name.
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
            Self::MissingField(m) => write!(f, "missing field: {m}"),
            Self::BadValue { field, value } => write!(f, "bad value for {field}: {value}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl DaemonConfig {
    /// Default for dev mode.
    #[must_use]
    pub fn default_for_dev() -> Self {
        Self {
            domain: 0,
            log_level: "info".to_string(),
            broker_url: "mqtt://127.0.0.1:1883".to_string(),
            client_id: "zerodds-bridge".to_string(),
            username: None,
            password: None,
            keep_alive_secs: 60,
            clean_start: true,
            topics: Vec::new(),
            tls_enabled: false,
            broker_tls_enabled: false,
            broker_tls_ca_file: String::new(),
            broker_tls_client_cert_file: String::new(),
            broker_tls_client_key_file: String::new(),
            broker_tls_server_name: String::new(),
            auth_mode: "none".to_string(),
            auth_bearer_token: None,
            auth_bearer_subject: None,
            outbound_username: None,
            outbound_password: None,
            sasl_users: std::collections::HashMap::new(),
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
        let expanded = super::yaml::expand_env_vars(raw);
        let nodes = super::yaml::parse(&expanded)?;
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
                "mqtt" => {
                    if let super::yaml::YamlNode::Map(m) = v {
                        if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("broker_url") {
                            out.broker_url = s.clone();
                        }
                        if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("client_id") {
                            out.client_id = s.clone();
                        }
                        if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("username") {
                            out.username = Some(s.clone());
                        }
                        if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("password") {
                            out.password = Some(s.clone());
                        }
                        if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("keep_alive_secs") {
                            out.keep_alive_secs = s.parse().unwrap_or(60);
                        }
                        if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("clean_start") {
                            out.clean_start = parse_bool(s);
                        }
                        if let Some(super::yaml::YamlNode::Map(tls)) = m.get("tls") {
                            if let Some(super::yaml::YamlNode::Scalar(s)) = tls.get("enabled") {
                                out.tls_enabled = parse_bool(s);
                                out.broker_tls_enabled = parse_bool(s);
                            }
                            if let Some(super::yaml::YamlNode::Scalar(s)) = tls.get("ca_file") {
                                out.broker_tls_ca_file = s.clone();
                            }
                            if let Some(super::yaml::YamlNode::Scalar(s)) =
                                tls.get("client_cert_file")
                            {
                                out.broker_tls_client_cert_file = s.clone();
                            }
                            if let Some(super::yaml::YamlNode::Scalar(s)) =
                                tls.get("client_key_file")
                            {
                                out.broker_tls_client_key_file = s.clone();
                            }
                            if let Some(super::yaml::YamlNode::Scalar(s)) = tls.get("server_name") {
                                out.broker_tls_server_name = s.clone();
                            }
                        }
                    }
                }
                "auth" => {
                    if let super::yaml::YamlNode::Map(m) = v {
                        if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("mode") {
                            out.auth_mode = s.clone();
                        }
                        if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("bearer_token") {
                            out.auth_bearer_token = Some(s.clone());
                        }
                        if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("bearer_subject") {
                            out.auth_bearer_subject = Some(s.clone());
                        }
                        if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("username") {
                            out.outbound_username = Some(s.clone());
                        }
                        if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("password") {
                            out.outbound_password = Some(s.clone());
                        }
                        if let Some(super::yaml::YamlNode::Map(users)) = m.get("sasl_users") {
                            for (u, val) in users.iter() {
                                if let super::yaml::YamlNode::Scalar(p) = val {
                                    out.sasl_users.insert(u.clone(), p.clone());
                                }
                            }
                        }
                    }
                }
                "acl" => {
                    if let super::yaml::YamlNode::Map(m) = v {
                        for (topic, entry) in m.iter() {
                            if let super::yaml::YamlNode::Map(em) = entry {
                                let read = em
                                    .get("read")
                                    .and_then(|n| match n {
                                        super::yaml::YamlNode::Scalar(s) => Some(
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
                                        super::yaml::YamlNode::Scalar(s) => Some(
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
                "metrics" => {
                    if let super::yaml::YamlNode::Map(m) = v {
                        if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("enabled") {
                            out.metrics_enabled = parse_bool(s);
                        }
                        if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("address") {
                            out.metrics_addr = s.clone();
                        }
                    }
                }
                "topics" => {
                    if let super::yaml::YamlNode::Seq(items) = v {
                        for item in items.iter() {
                            if let super::yaml::YamlNode::Map(m) = item {
                                let mut t = TopicConfig::default();
                                if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("dds_name") {
                                    t.dds_name = s.clone();
                                }
                                if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("dds_type") {
                                    t.dds_type = s.clone();
                                }
                                if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("mqtt_topic")
                                {
                                    t.mqtt_topic = s.clone();
                                }
                                if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("direction") {
                                    t.direction = s.clone();
                                } else {
                                    t.direction = "bidir".to_string();
                                }
                                if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("mqtt_qos") {
                                    t.mqtt_qos = s.parse().unwrap_or(0);
                                }
                                if let Some(super::yaml::YamlNode::Scalar(s)) = m.get("retain") {
                                    t.retain = parse_bool(s);
                                }
                                if let Some(super::yaml::YamlNode::Map(qm)) = m.get("qos") {
                                    if let Some(super::yaml::YamlNode::Scalar(s)) =
                                        qm.get("reliability")
                                    {
                                        t.reliability = s.clone();
                                    }
                                    if let Some(super::yaml::YamlNode::Scalar(s)) =
                                        qm.get("durability")
                                    {
                                        t.durability = s.clone();
                                    }
                                    if let Some(super::yaml::YamlNode::Map(hm)) = qm.get("history")
                                    {
                                        if let Some(super::yaml::YamlNode::Scalar(s)) =
                                            hm.get("depth")
                                        {
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
                                if t.mqtt_topic.is_empty() {
                                    t.mqtt_topic = default_mqtt_slug(&t.dds_name);
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

/// Slug pro Spec §5.1: `Chat::Message` → `chat/message`.
#[must_use]
pub fn default_mqtt_slug(topic: &str) -> String {
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

/// Parses `mqtt://host:port` or `mqtts://host:port`.
///
/// # Errors
/// `ConfigError::BadValue` if not a valid URL.
pub fn parse_broker_url(url: &str) -> Result<(String, u16, bool), ConfigError> {
    let (scheme, rest) = url.split_once("://").ok_or(ConfigError::BadValue {
        field: "broker_url".to_string(),
        value: url.to_string(),
    })?;
    let tls = match scheme {
        "mqtt" => false,
        "mqtts" => true,
        _ => {
            return Err(ConfigError::BadValue {
                field: "broker_url.scheme".to_string(),
                value: scheme.to_string(),
            });
        }
    };
    let (host, port_str) = rest
        .split_once(':')
        .unwrap_or((rest, if tls { "8883" } else { "1883" }));
    let port: u16 = port_str.parse().map_err(|_| ConfigError::BadValue {
        field: "broker_url.port".to_string(),
        value: port_str.to_string(),
    })?;
    Ok((host.to_string(), port, tls))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn slug_handles_double_colon() {
        assert_eq!(default_mqtt_slug("Chat::Message"), "chat/message");
    }

    #[test]
    fn slug_replaces_unsafe_chars() {
        assert_eq!(default_mqtt_slug("My Topic!"), "my_topic_");
    }

    #[test]
    fn parse_broker_url_mqtt_default_port() {
        let (h, p, tls) = parse_broker_url("mqtt://localhost").unwrap();
        assert_eq!(h, "localhost");
        assert_eq!(p, 1883);
        assert!(!tls);
    }

    #[test]
    fn parse_broker_url_mqtts_default_port() {
        let (_h, p, tls) = parse_broker_url("mqtts://x").unwrap();
        assert_eq!(p, 8883);
        assert!(tls);
    }

    #[test]
    fn parse_broker_url_explicit_port() {
        let (_h, p, _) = parse_broker_url("mqtt://h:9999").unwrap();
        assert_eq!(p, 9999);
    }

    #[test]
    fn parse_broker_url_rejects_unknown_scheme() {
        assert!(parse_broker_url("ws://x").is_err());
    }

    #[test]
    fn config_loads_minimal() {
        let yaml = "\
domain: 7
mqtt:
  broker_url: mqtt://broker:1883
  client_id: c1
topics:
  - dds_name: T
    direction: bidir
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert_eq!(cfg.domain, 7);
        assert_eq!(cfg.broker_url, "mqtt://broker:1883");
        assert_eq!(cfg.client_id, "c1");
        assert_eq!(cfg.topics.len(), 1);
        assert_eq!(cfg.topics[0].mqtt_topic, "t");
    }
}
