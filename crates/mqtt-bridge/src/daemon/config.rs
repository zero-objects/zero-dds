// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Config for `zerodds-mqtt-bridged`. Spec §3.
//!
//! Read by the shared layered loader (`zerodds-config`): base file,
//! `mqtt-bridged.d/` drop-ins, includes, `${VAR}`, and the precedence
//! defaults < environment < files < `--set`. The DDS runtime's keys may sit
//! in the same file. [`DaemonConfig::from_config`] reads this daemon's keys
//! and refuses an unknown key inside a known section, and a value of the
//! wrong shape or type, with its file and line.

use std::path::Path;
use std::string::{String, ToString};
use std::vec::Vec;

pub use zerodds_config::ConfigError;
use zerodds_config::read::Section;

/// The base name of the daemon's file: `mqtt-bridged.yaml`, `mqtt-bridged.d/`.
pub const CONFIG_STEM: &str = "mqtt-bridged";

/// Every key this daemon reads; list- and map-valued keys stop at the list
/// or map (`topics`, `acl.rules`, `auth.sasl_users`).
pub const KEYS: &[&str] = &[
    "domain",
    "log_level",
    "mqtt.broker_url",
    "mqtt.client_id",
    "mqtt.username",
    "mqtt.password",
    "mqtt.keep_alive_secs",
    "mqtt.drain_timeout_secs",
    "mqtt.clean_start",
    "mqtt.tls.enabled",
    "mqtt.tls.ca_file",
    "mqtt.tls.cert_file",
    "mqtt.tls.key_file",
    "mqtt.tls.server_name",
    "auth.mode",
    "auth.bearer_token",
    "auth.bearer_subject",
    "auth.username",
    "auth.password",
    "auth.sasl_users",
    "acl.default_deny",
    "acl.rules",
    "metrics.enabled",
    "metrics.listen",
    "topics",
];

/// The daemon's own environment names (Spec §2): `MQTT_PASSWORD` is
/// `mqtt.password`, beneath every file.
pub const ENV: &[(&str, &str)] = &[("MQTT_PASSWORD", "mqtt.password")];

/// Whether `path` is one of [`KEYS`] or an entry of `auth.sasl_users`.
#[must_use]
pub fn is_key(path: &str) -> bool {
    KEYS.contains(&path) || path.starts_with("auth.sasl_users.")
}

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
    /// `mqtt.drain_timeout_secs:` — Spec §9.2, how long the shutdown waits
    /// for the broker's acks (default 30).
    pub drain_timeout_secs: u64,
    /// `mqtt.clean_start:`.
    pub clean_start: bool,
    /// `topics:`.
    pub topics: Vec<TopicConfig>,
    /// Spec §7.1 — TLS active outbound to the broker (`mqtts://`).
    pub broker_tls_enabled: bool,
    /// `mqtt.tls.ca_file:` — PEM CA for broker-cert validation.
    pub broker_tls_ca_file: String,
    /// `mqtt.tls.cert_file:` — client cert (mTLS).
    pub broker_tls_client_cert_file: String,
    /// `mqtt.tls.key_file:` — client key (mTLS).
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
    /// Spec §7.3 — topic ACL `topic → (read subjects, write subjects)`,
    /// built from the subject-keyed `acl.rules` of Spec §3.
    pub topic_acl: std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
    /// `acl.default_deny` — Spec §7.3. `false` opens every topic no rule
    /// names. `true` denies them — as the key's default only once there is
    /// a rule or an auth mode: without either, an ACL nobody configured
    /// stays open (`zerodds_bridge_security::build_ctx`), so a daemon without
    /// an `acl:` section keeps working.
    pub acl_default_deny: bool,
    /// Whether `acl.default_deny` was given. An explicit `true` denies every
    /// unnamed topic even with no rule and `auth: none`; it used to be
    /// overridden by the open fallback without a word.
    pub acl_default_deny_set: bool,
    /// `metrics.enabled` — toggles the Prometheus endpoint (§8.2).
    pub metrics_enabled: bool,
    /// `metrics.listen`. Default `127.0.0.1:9092`
    /// if `metrics_enabled=true` and the address is empty.
    pub metrics_addr: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self::default_for_dev()
    }
}

/// Per-topic entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
            drain_timeout_secs: 30,
            clean_start: true,
            topics: Vec::new(),
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
            acl_default_deny: true,
            acl_default_deny_set: false,
            metrics_enabled: false,
            metrics_addr: String::new(),
        }
    }

    /// Loads one file (and its sibling `<stem>.d/`), for tests and tools.
    /// The daemon itself loads through `zerodds_dcps::cli::Configured`.
    ///
    /// # Errors
    /// Unreadable file, text outside the subset, or a refused value.
    pub fn load_from_file(path: &Path) -> Result<Self, ConfigError> {
        let config = zerodds_config::Loader::new()
            .base(path)
            .require_base()
            .drop_in_dir(path.with_extension("d"))
            .load()?;
        Self::from_config(Some(&config))
    }

    /// Parses from a string, for tests.
    ///
    /// # Errors
    /// Text outside the subset, or a refused value.
    pub fn load_from_str(raw: &str) -> Result<Self, ConfigError> {
        let mut config = zerodds_config::Config::default();
        config.push(
            zerodds_config::parse_str(raw, "<string>")?,
            "<string>".to_string(),
        )?;
        Self::from_config(Some(&config))
    }

    /// Reads this daemon's keys out of a merged tree, over
    /// [`Self::default_for_dev`]. `None` = nothing was loaded.
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] naming the setting, its origin and what it
    /// accepts.
    #[allow(clippy::too_many_lines)]
    pub fn from_config(config: Option<&zerodds_config::Config>) -> Result<Self, ConfigError> {
        let root = Section::root(config.and_then(zerodds_config::Config::root))?;
        let mut out = Self::default_for_dev();
        if let Some(v) = root.parse::<i32>("domain", "a DDS domain id")? {
            out.domain = v;
        }
        if let Some(v) = root.choice("log_level", &["trace", "debug", "info", "warn", "error"])? {
            out.log_level = v;
        }

        let mqtt = root.section("mqtt")?;
        mqtt.only(&[
            "broker_url",
            "client_id",
            "username",
            "password",
            "keep_alive_secs",
            "drain_timeout_secs",
            "clean_start",
            "tls",
        ])?;
        if let Some((url, origin)) = mqtt.scalar("broker_url")? {
            parse_broker_url(url).map_err(|e| with_origin(e, origin))?;
            out.broker_url = url.to_string();
        }
        if let Some(v) = mqtt.string("client_id")? {
            out.client_id = v;
        }
        out.username = mqtt.string("username")?;
        out.password = mqtt.string("password")?;
        if let Some(v) = mqtt.parse::<u16>("keep_alive_secs", "seconds, 0..=65535")? {
            out.keep_alive_secs = v;
        }
        if let Some(v) = mqtt.parse::<u64>("drain_timeout_secs", "seconds")? {
            out.drain_timeout_secs = v;
        }
        if let Some(v) = mqtt.boolean("clean_start")? {
            out.clean_start = v;
        }
        let tls = mqtt.section("tls")?;
        tls.only(&["enabled", "ca_file", "cert_file", "key_file", "server_name"])?;
        if let Some(v) = tls.boolean("enabled")? {
            out.broker_tls_enabled = v;
        }
        if let Some(v) = tls.string("ca_file")? {
            out.broker_tls_ca_file = v;
        }
        if let Some(v) = tls.string("cert_file")? {
            out.broker_tls_client_cert_file = v;
        }
        if let Some(v) = tls.string("key_file")? {
            out.broker_tls_client_key_file = v;
        }
        if let Some(v) = tls.string("server_name")? {
            out.broker_tls_server_name = v;
        }

        let auth = root.section("auth")?;
        auth.only(&[
            "mode",
            "bearer_token",
            "bearer_subject",
            "username",
            "password",
            "sasl_users",
        ])?;
        if let Some(v) = auth.choice("mode", &["none", "bearer", "sasl", "sasl_plain", "mtls"])? {
            out.auth_mode = v;
        }
        out.auth_bearer_token = auth.string("bearer_token")?;
        out.auth_bearer_subject = auth.string("bearer_subject")?;
        out.outbound_username = auth.string("username")?;
        out.outbound_password = auth.string("password")?;
        let users = auth.section("sasl_users")?;
        if let Some(zerodds_config::Node::Map(map)) = auth.node("sasl_users") {
            for user in map.keys() {
                if let Some(pass) = users.string(user)? {
                    out.sasl_users.insert(user.clone(), pass);
                }
            }
        }

        // Spec §3/§7.3: `acl: { default_deny, rules: [ { subject,
        // allow_publish: [...], allow_subscribe: [...] } ] }`, inverted into
        // the topic-keyed map the security layer consumes.
        let acl = root.section("acl")?;
        acl.only(&["default_deny", "rules"])?;
        if let Some(v) = acl.boolean("default_deny")? {
            out.acl_default_deny = v;
            out.acl_default_deny_set = true;
        }
        for (path, rule) in acl.items("rules")? {
            let rule = Section::of(Some(rule), path)?;
            rule.only(&["subject", "allow_publish", "allow_subscribe"])?;
            let Some(subject) = rule.string("subject")? else {
                return Err(ConfigError::Invalid {
                    setting: rule.path_of("subject"),
                    origin: acl.node("rules").map_or(
                        zerodds_config::Origin::Default,
                        zerodds_config::read::first_origin,
                    ),
                    message: "an acl rule needs a subject".to_string(),
                });
            };
            for pat in rule.strings("allow_subscribe")?.unwrap_or_default() {
                out.topic_acl
                    .entry(pat)
                    .or_default()
                    .0
                    .push(subject.clone());
            }
            for pat in rule.strings("allow_publish")?.unwrap_or_default() {
                out.topic_acl
                    .entry(pat)
                    .or_default()
                    .1
                    .push(subject.clone());
            }
        }

        let metrics = root.section("metrics")?;
        metrics.only(&["enabled", "listen"])?;
        if let Some(v) = metrics.boolean("enabled")? {
            out.metrics_enabled = v;
        }
        // `listen`, as §3 of every bridge spec spells it.
        if let Some(v) = metrics.string("listen")? {
            out.metrics_addr = v;
        }

        for (path, item) in root.items("topics")? {
            out.topics.push(read_topic(&path, item)?);
        }
        Ok(out)
    }
}

/// See [`zerodds_config::read::split_topic_shorthand`].
pub use zerodds_config::read::split_topic_shorthand;

/// One `topics[]` entry; a bare `DDS[:MQTT]` text is the entry
/// `{dds_name: DDS, mqtt_topic: MQTT}`.
fn read_topic(path: &str, item: &zerodds_config::Node) -> Result<TopicConfig, ConfigError> {
    let mut t = TopicConfig {
        direction: "bidir".to_string(),
        history_depth: 10,
        ..TopicConfig::default()
    };
    if let zerodds_config::Node::Scalar(text, _) = item {
        let (dds, mqtt) = split_topic_shorthand(text);
        t.dds_name = dds;
        t.mqtt_topic = mqtt.unwrap_or_default();
    } else {
        let entry = Section::of(Some(item), path)?;
        entry.only(&[
            "dds_name",
            "dds_type",
            "mqtt_topic",
            "direction",
            "mqtt_qos",
            "retain",
            "qos",
        ])?;
        t.dds_name = entry.string("dds_name")?.unwrap_or_default();
        t.dds_type = entry.string("dds_type")?.unwrap_or_default();
        t.mqtt_topic = entry.string("mqtt_topic")?.unwrap_or_default();
        if let Some(v) = entry.choice("direction", &["in", "out", "bidir"])? {
            t.direction = v;
        }
        if let Some(v) = entry.choice("mqtt_qos", &["0", "1", "2"])? {
            t.mqtt_qos = v.parse().unwrap_or(0);
        }
        if let Some(v) = entry.boolean("retain")? {
            t.retain = v;
        }
        let qos = entry.section("qos")?;
        qos.only(&["reliability", "durability", "history"])?;
        t.reliability = qos
            .choice("reliability", &["reliable", "best_effort"])?
            .unwrap_or_default();
        t.durability = qos
            .choice(
                "durability",
                &["volatile", "transient_local", "transient", "persistent"],
            )?
            .unwrap_or_default();
        let history = qos.section("history")?;
        history.only(&["depth"])?;
        if let Some(depth) = history.parse::<i32>("depth", "an integer >= 1")? {
            if depth < 1 {
                return Err(ConfigError::Invalid {
                    setting: history.path_of("depth"),
                    origin: zerodds_config::read::first_origin(item),
                    message: format!("got `{depth}`, expected an integer >= 1"),
                });
            }
            t.history_depth = depth;
        }
    }
    if t.dds_name.is_empty() {
        return Err(ConfigError::Invalid {
            setting: format!("{path}.dds_name"),
            origin: zerodds_config::read::first_origin(item),
            message: "a topic entry needs a dds_name".to_string(),
        });
    }
    if t.dds_type.is_empty() {
        t.dds_type = t.dds_name.clone();
    }
    if t.mqtt_topic.is_empty() {
        t.mqtt_topic = default_mqtt_slug(&t.dds_name);
    }
    Ok(t)
}

/// Puts the origin of the refused value into a refusal made without one.
fn with_origin(error: ConfigError, origin: &zerodds_config::Origin) -> ConfigError {
    match error {
        ConfigError::Invalid {
            setting, message, ..
        } => ConfigError::Invalid {
            setting,
            origin: origin.clone(),
            message,
        },
        other => other,
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

/// How the bridge reaches its broker (Spec §2 `--broker`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerTransport {
    /// MQTT directly on the TCP (or TLS) byte stream.
    Tcp,
    /// MQTT in WebSocket binary frames (MQTT 5.0 §6), subprotocol `mqtt`,
    /// on the resource `path`.
    WebSocket {
        /// The resource name of the opening handshake, `/mqtt` by default.
        path: String,
    },
}

/// A parsed `mqtt.broker_url`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerUrl {
    /// Host name or address; an IPv6 address without its brackets.
    pub host: String,
    /// Port; the scheme's default when the URL names none.
    pub port: u16,
    /// `mqtts://` and `wss://`: TLS below MQTT or below WebSocket.
    pub tls: bool,
    /// TCP or WebSocket.
    pub transport: BrokerTransport,
}

impl BrokerUrl {
    /// The host as a URL writes it: an IPv6 address in brackets.
    fn url_host(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        }
    }

    /// The `host:port` a socket connects to.
    #[must_use]
    pub fn socket_addr(&self) -> String {
        format!("{}:{}", self.url_host(), self.port)
    }

    /// The `Host` header of the WebSocket handshake: the port only when it
    /// is not the scheme's default (RFC 6455 §4.1, RFC 7230 §5.4).
    #[must_use]
    pub fn host_header(&self) -> String {
        let default_port = match (&self.transport, self.tls) {
            (BrokerTransport::WebSocket { .. }, true) => 443,
            (BrokerTransport::WebSocket { .. }, false) => 80,
            (BrokerTransport::Tcp, true) => 8883,
            (BrokerTransport::Tcp, false) => 1883,
        };
        if self.port == default_port {
            self.url_host()
        } else {
            self.socket_addr()
        }
    }
}

/// Parses `mqtt://`, `mqtts://`, `ws://` or `wss://` `host[:port]`, with a
/// resource path for the two WebSocket schemes (`ws://host:8080/mqtt`).
/// Default ports: 1883, 8883, 80, 443. A WebSocket URL without a path uses
/// `/mqtt`, the path the common brokers serve. User information in the URL
/// is refused; credentials are `mqtt.username` / `mqtt.password`.
///
/// # Errors
/// [`ConfigError::Invalid`] on `mqtt.broker_url` if not a valid URL.
pub fn parse_broker_url(url: &str) -> Result<BrokerUrl, ConfigError> {
    let bad = |message: String| ConfigError::Invalid {
        setting: "mqtt.broker_url".to_string(),
        origin: zerodds_config::Origin::Default,
        message,
    };
    let (scheme, rest) = url.split_once("://").ok_or_else(|| {
        bad(format!(
            "got `{url}`, expected mqtt://, mqtts://, ws:// or wss://host[:port]"
        ))
    })?;
    let (tls, websocket, default_port) = match scheme {
        "mqtt" => (false, false, 1883),
        "mqtts" => (true, false, 8883),
        "ws" => (false, true, 80),
        "wss" => (true, true, 443),
        _ => {
            return Err(bad(format!(
                "scheme `{scheme}` is not mqtt, mqtts, ws or wss"
            )));
        }
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    if authority.contains('@') {
        return Err(bad(
            "user information in the URL is not read; set mqtt.username and mqtt.password"
                .to_string(),
        ));
    }
    let (host, port_str) = if let Some(v6) = authority.strip_prefix('[') {
        let (h, after) = v6
            .split_once(']')
            .ok_or_else(|| bad(format!("got `{url}`, an IPv6 address without its `]`")))?;
        match after.strip_prefix(':') {
            Some(p) => (h, Some(p)),
            None if after.is_empty() => (h, None),
            None => return Err(bad(format!("got `{url}`, text after the IPv6 address"))),
        }
    } else {
        match authority.split_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (authority, None),
        }
    };
    if host.is_empty() {
        return Err(bad(format!("got `{url}`, which names no host")));
    }
    let port: u16 = match port_str {
        None => default_port,
        Some(p) => p
            .parse()
            .map_err(|_| bad(format!("port `{p}` is not 0..=65535")))?,
    };
    let transport = if websocket {
        BrokerTransport::WebSocket {
            path: if path.is_empty() {
                "/mqtt".to_string()
            } else {
                path.to_string()
            },
        }
    } else {
        if !path.is_empty() && path != "/" {
            return Err(bad(format!(
                "got `{url}`: a path applies only to ws:// and wss://"
            )));
        }
        BrokerTransport::Tcp
    };
    Ok(BrokerUrl {
        host: host.to_string(),
        port,
        tls,
        transport,
    })
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
        let u = parse_broker_url("mqtt://localhost").unwrap();
        assert_eq!((u.host.as_str(), u.port, u.tls), ("localhost", 1883, false));
        assert_eq!(u.transport, BrokerTransport::Tcp);
    }

    #[test]
    fn parse_broker_url_mqtts_default_port() {
        let u = parse_broker_url("mqtts://x").unwrap();
        assert_eq!((u.port, u.tls), (8883, true));
    }

    #[test]
    fn parse_broker_url_explicit_port() {
        assert_eq!(parse_broker_url("mqtt://h:9999").unwrap().port, 9999);
    }

    /// Spec §2 names four schemes; ws and wss carry a resource path.
    #[test]
    fn parse_broker_url_reads_the_websocket_schemes() {
        let u = parse_broker_url("ws://b").unwrap();
        assert_eq!((u.port, u.tls), (80, false));
        assert_eq!(
            u.transport,
            BrokerTransport::WebSocket {
                path: "/mqtt".to_string()
            }
        );
        let u = parse_broker_url("wss://b:8443/ws/mqtt?x=1").unwrap();
        assert_eq!((u.port, u.tls), (8443, true));
        assert_eq!(
            u.transport,
            BrokerTransport::WebSocket {
                path: "/ws/mqtt?x=1".to_string()
            }
        );
        let u = parse_broker_url("ws://[::1]:9001/").unwrap();
        assert_eq!((u.host.as_str(), u.port), ("::1", 9001));
        assert_eq!(u.socket_addr(), "[::1]:9001");
        assert_eq!(u.host_header(), "[::1]:9001");
        assert_eq!(parse_broker_url("wss://b").unwrap().host_header(), "b");
    }

    #[test]
    fn parse_broker_url_refuses_what_it_would_not_use() {
        for (url, needle) in [
            ("tcp://x", "scheme `tcp`"),
            ("mqtt://x/path", "only to ws://"),
            ("mqtt://user:pw@x", "mqtt.username"),
            ("ws://[::1", "without its `]`"),
            ("mqtt://:1883", "names no host"),
            ("mqtt://x:99999", "0..=65535"),
        ] {
            let err = parse_broker_url(url).expect_err(url).to_string();
            assert!(err.contains(needle), "{url}: {err}");
        }
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

    #[test]
    fn a_topic_shorthand_splits_at_the_lone_colon() {
        assert_eq!(
            split_topic_shorthand("Chat::Message:chat/msg"),
            ("Chat::Message".to_string(), Some("chat/msg".to_string()))
        );
        assert_eq!(
            split_topic_shorthand("Chat::Message"),
            ("Chat::Message".to_string(), None)
        );
        let cfg = DaemonConfig::load_from_str("topics:\n  - \"T:factory/t\"\n").unwrap();
        assert_eq!(cfg.topics[0].dds_name, "T");
        assert_eq!(cfg.topics[0].mqtt_topic, "factory/t");
    }

    #[test]
    fn what_the_old_parser_dropped_is_refused_with_its_line() {
        for (yaml, setting) in [
            ("mqtt:\n  keep_alive_secs: soon\n", "mqtt.keep_alive_secs"),
            ("mqtt:\n  broker_url: http://x\n", "mqtt.broker_url"),
            ("mqtt:\n  tls:\n    enabeld: true\n", "mqtt.tls.enabeld"),
            (
                "topics:\n  - dds_name: T\n    mqtt_qos: 3\n",
                "topics[0].mqtt_qos",
            ),
            ("acl:\n  rules:\n    - subjct: a\n", "acl.rules[0].subjct"),
            ("auth:\n  mode: token\n", "auth.mode"),
        ] {
            let err = DaemonConfig::load_from_str(yaml).unwrap_err();
            assert!(
                matches!(&err, ConfigError::Invalid { setting: s, .. } if s == setting),
                "{yaml}: {err}"
            );
            assert!(err.to_string().contains("<string>:"), "{yaml}: {err}");
        }
    }
}
