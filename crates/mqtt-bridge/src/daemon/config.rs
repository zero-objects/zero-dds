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

use std::path::{Path, PathBuf};
use std::string::{String, ToString};
use std::vec::Vec;

pub use zerodds_config::ConfigError;
use zerodds_config::read::Section;

/// The base name of the daemon's file: `mqtt-bridged.yaml`, `mqtt-bridged.d/`.
pub const CONFIG_STEM: &str = "mqtt-bridged";

/// Every key this daemon reads; list- and map-valued keys stop at the list
/// or map (`topics`, `tenants`, `acl.rules`, `mqtt.tls.alpn`).
pub const KEYS: &[&str] = &[
    "domain",
    "log_level",
    "mqtt.broker_url",
    "mqtt.client_id",
    "mqtt.protocol_version",
    "mqtt.username",
    "mqtt.password",
    "mqtt.keep_alive_secs",
    "mqtt.drain_timeout_secs",
    "mqtt.clean_start",
    "mqtt.session_expiry_interval_secs",
    "mqtt.session_dir",
    "mqtt.max_packet_size",
    "mqtt.receive_maximum",
    "mqtt.topic_alias_maximum",
    "mqtt.connect_timeout_secs",
    "mqtt.reconnect.initial_delay_ms",
    "mqtt.reconnect.max_delay_ms",
    "mqtt.reconnect.factor",
    "mqtt.own_topic_prefix",
    "mqtt.tls.enabled",
    "mqtt.tls.ca_file",
    "mqtt.tls.cert_file",
    "mqtt.tls.key_file",
    "mqtt.tls.server_name",
    "mqtt.tls.verify_hostname",
    "mqtt.tls.alpn",
    "auth.mode",
    "auth.bearer_token",
    "auth.bearer_subject",
    "auth.username",
    "auth.password",
    "auth.enhanced.method",
    "acl.default_deny",
    "acl.rules",
    "metrics.enabled",
    "metrics.listen",
    "metrics.path",
    "topics",
    "tenants",
];

/// The daemon's own environment names (Spec §2): `MQTT_PASSWORD` is
/// `mqtt.password`, beneath every file.
pub const ENV: &[(&str, &str)] = &[("MQTT_PASSWORD", "mqtt.password")];

/// The largest MQTT packet the bridge reads (`mqtt.max_packet_size`, its
/// default and its upper bound): [`crate::net::MAX_MQTT_PACKET_SIZE`].
pub const MAX_PACKET_SIZE: u32 = crate::net::MAX_MQTT_PACKET_SIZE;

/// The Topic Alias Maximum of the CONNECT when `mqtt.topic_alias_maximum`
/// is not set (Spec §4.1).
pub const DEFAULT_TOPIC_ALIAS_MAXIMUM: u16 = 100;

/// The levels in front of `<client_id>/` in the daemon's own topics when
/// `mqtt.own_topic_prefix` is not set (Spec §5.2, §6).
pub const DEFAULT_OWN_TOPIC_PREFIX: &str = "$zerodds";

/// The path of the Prometheus metrics on the admin endpoint when
/// `metrics.path` is not set (Spec §3, §8.2).
pub const DEFAULT_METRICS_PATH: &str = "/metrics";

/// Whether `path` is one of [`KEYS`].
#[must_use]
pub fn is_key(path: &str) -> bool {
    KEYS.contains(&path)
}

/// Where the Session State of a session that outlives its connection is
/// kept when `mqtt.session_dir` is not set (Spec §4.1): the state directory
/// of `zerodds-deployment-1.0` (`/var/lib/zerodds` on Linux,
/// `/usr/local/var/lib/zerodds` on macOS, `%PROGRAMDATA%\ZeroDDS` on
/// Windows) with `mqtt-bridge`.
#[must_use]
pub fn default_session_dir() -> String {
    #[cfg(windows)]
    let base = std::env::var_os("PROGRAMDATA")
        .map_or_else(|| PathBuf::from(r"C:\ProgramData"), PathBuf::from)
        .join("ZeroDDS");
    #[cfg(target_os = "macos")]
    let base = PathBuf::from("/usr/local/var/lib/zerodds");
    #[cfg(not(any(windows, target_os = "macos")))]
    let base = PathBuf::from("/var/lib/zerodds");
    base.join("mqtt-bridge").display().to_string()
}

/// The client identifier when `mqtt.client_id` is not set (Spec §2):
/// `zerodds-bridge-<host>`, `zerodds-bridge` on a host without a name.
#[must_use]
pub fn default_client_id() -> String {
    match zerodds_dcps::participant::host_name() {
        Some(host) => format!("zerodds-bridge-{host}"),
        None => "zerodds-bridge".to_string(),
    }
}

/// `mqtt.reconnect` (Spec §3): the backoff between two connect attempts,
/// at the start (§9.1) and after a lost connection (§9.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReconnectConfig {
    /// `initial_delay_ms`: the wait after the first failed attempt.
    pub initial_delay_ms: u64,
    /// `max_delay_ms`: the longest wait.
    pub max_delay_ms: u64,
    /// `factor`: each further wait is the last one times this, up to
    /// `max_delay_ms`.
    pub factor: f64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            initial_delay_ms: 500,
            max_delay_ms: 30_000,
            factor: 2.0,
        }
    }
}

impl ReconnectConfig {
    /// The wait after the first failed attempt.
    #[must_use]
    pub fn first_delay(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.initial_delay_ms.min(self.max_delay_ms))
    }

    /// The wait after `last`: `last * factor`, at most `max_delay_ms`.
    #[must_use]
    pub fn next_delay(&self, last: std::time::Duration) -> std::time::Duration {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let next = (last.as_millis() as f64 * self.factor) as u64;
        std::time::Duration::from_millis(next.clamp(self.initial_delay_ms, self.max_delay_ms))
    }
}

/// A `tenants:` entry (Spec §1 L6, §3): a DomainParticipant of its own on
/// `domain` and a broker connection of its own under `client_id`, with its
/// topics and its ACL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantConfig {
    /// `name`: unique, in the log lines and the restart notes of a reload.
    pub name: String,
    /// `domain`: its DDS domain; the top-level `domain` when not set.
    pub domain: i32,
    /// `mqtt.client_id`: the client identifier of its connection, the
    /// binding of the tenant on the broker.
    pub client_id: String,
    /// `mqtt.username`: the user name of its CONNECT; the top level's when
    /// not set.
    pub username: Option<String>,
    /// `mqtt.password`: the password of its CONNECT (the token under
    /// `auth.mode: bearer`); the top level's when not set.
    pub password: Option<String>,
    /// `topics`: its topics, in the form of the top-level `topics`.
    pub topics: Vec<TopicConfig>,
    /// `acl.rules` (Spec §7.3).
    pub acl_rules: Vec<super::acl::AclRule>,
    /// `acl.default_deny`, `None` when not set.
    pub acl_default_deny: Option<bool>,
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
    /// `mqtt.client_id:`; [`default_client_id`] when not set.
    pub client_id: String,
    /// `mqtt.protocol_version:` — `5` (the default) or `3.1.1`, the
    /// compatibility mode of Spec §1 L1.
    pub protocol_version: crate::version::ProtocolVersion,
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
    /// `mqtt.session_expiry_interval_secs:` — the Session Expiry Interval
    /// of the CONNECT (Spec §4.1); 0, the default, ends the session with
    /// the connection.
    pub session_expiry_interval_secs: u32,
    /// `mqtt.session_dir:` — where the Session State of a session that
    /// outlives its connection is kept, one file per client identifier
    /// (Spec §4.1); default [`default_session_dir`].
    pub session_dir: String,
    /// `mqtt.max_packet_size:` — the Maximum Packet Size of the CONNECT
    /// (Spec §4.1); a larger packet from the broker ends the connection
    /// with 0x95. Default and upper bound [`MAX_PACKET_SIZE`].
    pub max_packet_size: u32,
    /// `mqtt.receive_maximum:` — the Receive Maximum of the CONNECT (Spec
    /// §4.1): QoS 2 messages of the broker the bridge holds unreleased at
    /// once (default 65535).
    pub receive_maximum: u16,
    /// `mqtt.topic_alias_maximum:` — the Topic Alias Maximum of the CONNECT
    /// (Spec §4.1, default 100); 0 accepts no Topic Alias.
    pub topic_alias_maximum: u16,
    /// `mqtt.connect_timeout_secs:` — how long the start retries an
    /// unreachable broker before exit 2 (Spec §9.1, default 30); 0 retries
    /// without end.
    pub connect_timeout_secs: u64,
    /// `mqtt.reconnect:`.
    pub reconnect: ReconnectConfig,
    /// `mqtt.own_topic_prefix:` — the levels in front of
    /// `<client_id>/` in the daemon's own topics: the catalog of Spec §5.2,
    /// the reports and the will of §6. Default [`DEFAULT_OWN_TOPIC_PREFIX`];
    /// a broker that keeps the topics beginning with `$` to itself (MQTT
    /// 5.0 §4.7.2) needs another one.
    pub own_topic_prefix: String,
    /// `topics:`.
    pub topics: Vec<TopicConfig>,
    /// `tenants:` (Spec §1 L6).
    pub tenants: Vec<TenantConfig>,
    /// The name of the tenant this is the configuration of (Spec §1 L6),
    /// set by [`Self::of_tenant`]; empty for the top level. Not a key: the
    /// `tenant` of the log lines of its connection (§8.1).
    pub tenant: String,
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
    /// `mqtt.tls.alpn:` — the ALPN protocol names the TLS handshake offers
    /// (RFC 7301); `None` offers `mqtt` under `mqtts://` and `http/1.1`
    /// under `wss://` (Spec §7.1).
    pub broker_tls_alpn: Option<Vec<String>>,
    /// `auth.mode` (Spec §7.2); `None` when not set: the mode follows from
    /// what is configured ([`super::auth::Credentials::kind`]).
    pub auth_mode: Option<super::auth::AuthKind>,
    /// `auth.bearer_token:` — the token: the CONNECT password under
    /// `password`, the OAUTHBEARER token or the Authentication Data of
    /// another method under `enhanced`.
    pub auth_bearer_token: Option<String>,
    /// `auth.bearer_subject:` — the bridge's subject for the ACL (Spec
    /// §7.3) in place of the one it would have.
    pub auth_bearer_subject: Option<String>,
    /// `auth.username:` — the user name of `password` and of SCRAM; wins
    /// over `mqtt.username`.
    pub outbound_username: Option<String>,
    /// `auth.password:` — the password of `password` and of SCRAM; wins
    /// over `mqtt.password`.
    pub outbound_password: Option<String>,
    /// `auth.enhanced.method:` — the Authentication Method under `enhanced`
    /// (`SCRAM-SHA-256`, `OAUTHBEARER`, another).
    pub auth_enhanced_method: Option<String>,
    /// `acl.rules` (Spec §7.3): topic filters per subject.
    pub acl_rules: Vec<super::acl::AclRule>,
    /// `acl.default_deny` (Spec §7.3), `None` when not set: then true once
    /// a rule exists, false without rules.
    pub acl_default_deny: Option<bool>,
    /// `metrics.enabled` — toggles the Prometheus endpoint (§8.2).
    pub metrics_enabled: bool,
    /// `metrics.listen`. Default `127.0.0.1:9092`
    /// if `metrics_enabled=true` and the address is empty.
    pub metrics_addr: String,
    /// `metrics.path` — the path of the Prometheus metrics on the admin
    /// endpoint (default [`DEFAULT_METRICS_PATH`]).
    pub metrics_path: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self::default_for_dev()
    }
}

/// Per-topic entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicConfig {
    /// `dds_name:`.
    pub dds_name: String,
    /// `dds_type:`.
    pub dds_type: String,
    /// `mqtt_topic:`.
    pub mqtt_topic: String,
    /// `direction:`.
    pub direction: String,
    /// `mqtt_qos:` (0/1/2); `None` when not set: derived (Spec §6, see
    /// [`Self::effective_mqtt_qos`]).
    pub mqtt_qos: Option<u8>,
    /// `retain:`; `None` when not set: derived (Spec §6, see
    /// [`Self::effective_retain`]).
    pub retain: Option<bool>,
    /// `qos.reliability:`.
    pub reliability: String,
    /// `qos.durability:`.
    pub durability: String,
    /// `qos.history.kind:` — `keep_last` (the default) or `keep_all`.
    pub history_kind: String,
    /// `qos.history.depth:`.
    pub history_depth: i32,
    /// `schema_url:` — where the type's schema is published; in the catalog
    /// of Spec §5.2 when set.
    pub schema_url: Option<String>,
    /// `qos.lifespan_ms:` — the LIFESPAN of the topic's DataWriter, and the
    /// Message Expiry Interval of a sample whose writer names none (Spec
    /// §6); `None`: infinite.
    pub lifespan_ms: Option<u64>,
    /// `qos.deadline_ms:` — the DEADLINE the topic's DataReader requests and
    /// its DataWriter offers (Spec §6); `None`: infinite.
    pub deadline_ms: Option<u64>,
    /// `qos.liveliness.kind:` — `automatic` (the default),
    /// `manual_by_participant` or `manual_by_topic`.
    pub liveliness_kind: String,
    /// `qos.liveliness.lease_ms:` — the lease duration; `None`: infinite.
    pub liveliness_lease_ms: Option<u64>,
    /// `qos.partition:` — the PARTITION of the topic's DataReader and
    /// DataWriter (Spec §6); empty: the default partition.
    pub partition: Vec<String>,
    /// `data_representation:` — `xcdr1` or `xcdr2`, the one data
    /// representation the topic's DataWriter offers from its creation and
    /// its DataReader accepts (DDS-XTypes 1.3 §7.6.3.1, Spec §4.2, §6);
    /// `None` when not set: the DataWriter offers XCDR1 and takes the
    /// representation the header of each PUBLISH names, the DataReader
    /// accepts both.
    pub data_representation: Option<String>,
}

impl Default for TopicConfig {
    fn default() -> Self {
        Self {
            dds_name: String::new(),
            dds_type: String::new(),
            mqtt_topic: String::new(),
            direction: "bidir".to_string(),
            mqtt_qos: None,
            retain: None,
            reliability: String::new(),
            durability: String::new(),
            history_kind: "keep_last".to_string(),
            history_depth: 10,
            schema_url: None,
            lifespan_ms: None,
            deadline_ms: None,
            liveliness_kind: "automatic".to_string(),
            liveliness_lease_ms: None,
            partition: Vec::new(),
            data_representation: None,
        }
    }
}

impl TopicConfig {
    /// The DataRepresentationId of `data_representation` (DDS-XTypes 1.3
    /// §7.6.3.1.1): `XCDR` for `xcdr1`, `XCDR2` for `xcdr2`; `None` when
    /// not set.
    #[must_use]
    pub fn data_representation_id(&self) -> Option<i16> {
        use zerodds_rtps::publication_data::data_representation as dr;
        match self.data_representation.as_deref() {
            Some("xcdr1") => Some(dr::XCDR),
            Some(_) => Some(dr::XCDR2),
            None => None,
        }
    }

    /// Whether the topic's DDS endpoints are reliable: every reliability but
    /// `best_effort`.
    #[must_use]
    pub fn is_reliable(&self) -> bool {
        self.reliability != "best_effort"
    }

    /// `qos.reliability` as it applies: `reliable` when not set.
    #[must_use]
    pub fn effective_reliability(&self) -> &'static str {
        if self.is_reliable() {
            "reliable"
        } else {
            "best_effort"
        }
    }

    /// `qos.durability` as it applies: `volatile` when not set.
    #[must_use]
    pub fn effective_durability(&self) -> &str {
        if self.durability.is_empty() {
            "volatile"
        } else {
            &self.durability
        }
    }

    /// The MQTT QoS of the topic (Spec §6,
    /// [`super::qos_translation::mqtt_qos`]): `mqtt_qos` when set, else 1
    /// for a reliable topic and 0 for a best-effort one.
    #[must_use]
    pub fn effective_mqtt_qos(&self) -> u8 {
        super::qos_translation::mqtt_qos(self)
    }

    /// The retain flag of the topic (Spec §6,
    /// [`super::qos_translation::retain`]): `retain` when set, else `true`
    /// for the durabilities that keep samples for late joiners
    /// (`transient_local`, `transient`, `persistent`).
    #[must_use]
    pub fn effective_retain(&self) -> bool {
        super::qos_translation::retain(self)
    }

    /// The N of the topic's HISTORY (§3, §6): `history_depth` under
    /// `keep_last`, every sample (`usize::MAX`) under `keep_all`. It is the
    /// HISTORY of the topic's DataWriter and the samples of one instance the
    /// daemon holds for the broker ([`super::history::HistoryQueue`]).
    #[must_use]
    pub fn history_depth(&self) -> usize {
        if self.history_kind == "keep_all" {
            usize::MAX
        } else {
            usize::try_from(self.history_depth.max(1)).unwrap_or(1)
        }
    }
}

impl DaemonConfig {
    /// Default for dev mode.
    #[must_use]
    pub fn default_for_dev() -> Self {
        Self {
            domain: 0,
            log_level: "info".to_string(),
            broker_url: "mqtt://127.0.0.1:1883".to_string(),
            client_id: default_client_id(),
            protocol_version: crate::version::ProtocolVersion::V5,
            username: None,
            password: None,
            keep_alive_secs: 60,
            drain_timeout_secs: 30,
            clean_start: true,
            session_expiry_interval_secs: 0,
            session_dir: default_session_dir(),
            max_packet_size: MAX_PACKET_SIZE,
            receive_maximum: u16::MAX,
            topic_alias_maximum: DEFAULT_TOPIC_ALIAS_MAXIMUM,
            connect_timeout_secs: 30,
            reconnect: ReconnectConfig::default(),
            own_topic_prefix: DEFAULT_OWN_TOPIC_PREFIX.to_string(),
            topics: Vec::new(),
            tenants: Vec::new(),
            tenant: String::new(),
            broker_tls_enabled: false,
            broker_tls_ca_file: String::new(),
            broker_tls_client_cert_file: String::new(),
            broker_tls_client_key_file: String::new(),
            broker_tls_server_name: String::new(),
            broker_tls_alpn: None,
            auth_mode: None,
            auth_bearer_token: None,
            auth_bearer_subject: None,
            outbound_username: None,
            outbound_password: None,
            auth_enhanced_method: None,
            acl_rules: Vec::new(),
            acl_default_deny: None,
            metrics_enabled: false,
            metrics_addr: String::new(),
            metrics_path: DEFAULT_METRICS_PATH.to_string(),
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

    /// The configuration of tenant `t`'s participant and connection (Spec
    /// §1 L6): this one with the tenant's domain, client identifier, topics
    /// and ACL, and its user name and password where it names them.
    #[must_use]
    pub fn of_tenant(&self, t: &TenantConfig) -> Self {
        let mut out = Self {
            domain: t.domain,
            client_id: t.client_id.clone(),
            topics: t.topics.clone(),
            tenants: Vec::new(),
            tenant: t.name.clone(),
            acl_rules: t.acl_rules.clone(),
            acl_default_deny: t.acl_default_deny,
            ..self.clone()
        };
        // The tenant's `mqtt.username` and `mqtt.password` take the place of
        // the top level's `auth.username`/`mqtt.username` and
        // `auth.password`/`mqtt.password` (Spec §1 L6, §7.2); `auth::check`
        // then refuses what the mode does not use, naming the tenant.
        if let Some(user) = &t.username {
            out.username = Some(user.clone());
            out.outbound_username = None;
        }
        if let Some(pass) = &t.password {
            if out.auth_bearer_token.is_some() && out.outbound_password.is_none() {
                // The top level presents its token (the CONNECT password,
                // the OAUTHBEARER token or the data of another method); the
                // tenant's password replaces it.
                out.auth_bearer_token = Some(pass.clone());
            } else {
                out.password = Some(pass.clone());
                out.outbound_password = None;
            }
        }
        out
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
            "protocol_version",
            "username",
            "password",
            "keep_alive_secs",
            "drain_timeout_secs",
            "clean_start",
            "session_expiry_interval_secs",
            "session_dir",
            "max_packet_size",
            "receive_maximum",
            "topic_alias_maximum",
            "connect_timeout_secs",
            "reconnect",
            "own_topic_prefix",
            "tls",
        ])?;
        let mut broker_origin = zerodds_config::Origin::Default;
        if let Some((url, origin)) = mqtt.scalar("broker_url")? {
            parse_broker_url(url).map_err(|e| with_origin(e, origin))?;
            broker_origin = origin.clone();
            out.broker_url = url.to_string();
        }
        if let Some(v) = mqtt.string("client_id")? {
            out.client_id = v;
        }
        if let Some(v) = mqtt.choice("protocol_version", &["5", "3.1.1"])? {
            out.protocol_version = if v == "5" {
                crate::version::ProtocolVersion::V5
            } else {
                crate::version::ProtocolVersion::V311
            };
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
        if let Some(v) = mqtt.parse::<u32>(
            "session_expiry_interval_secs",
            "seconds, 0..=4294967295 (4294967295: the session does not expire)",
        )? {
            out.session_expiry_interval_secs = v;
        }
        if let Some(v) = mqtt.string("session_dir")? {
            if v.is_empty() {
                return Err(ConfigError::Invalid {
                    setting: mqtt.path_of("session_dir"),
                    origin: mqtt.node("session_dir").map_or(
                        zerodds_config::Origin::Default,
                        zerodds_config::read::first_origin,
                    ),
                    message: "a directory, not empty".to_string(),
                });
            }
            out.session_dir = v;
        }
        let packet_bound = format!("octets, 1..={MAX_PACKET_SIZE}");
        if let Some(v) = in_range(
            &mqtt,
            "max_packet_size",
            1..=u64::from(MAX_PACKET_SIZE),
            &packet_bound,
        )? {
            out.max_packet_size = u32::try_from(v).unwrap_or(MAX_PACKET_SIZE);
        }
        if let Some(v) = in_range(&mqtt, "receive_maximum", 1..=65_535, "1..=65535")? {
            out.receive_maximum = u16::try_from(v).unwrap_or(u16::MAX);
        }
        if let Some(v) = mqtt.parse::<u16>("topic_alias_maximum", "0..=65535")? {
            out.topic_alias_maximum = v;
        }
        if let Some(v) =
            mqtt.parse::<u64>("connect_timeout_secs", "seconds (0: retry without end)")?
        {
            out.connect_timeout_secs = v;
        }
        if let Some((v, origin)) = mqtt.scalar("own_topic_prefix")? {
            // A topic name is not empty and carries no wildcard and no
            // U+0000 (MQTT 5.0 §4.7.1, §4.7.3, §1.5.4); the prefix is
            // followed by `/<client_id>/`, so it ends in no `/` of its own.
            if v.is_empty() || v.contains(['+', '#', '\0']) || v.ends_with('/') {
                return Err(ConfigError::Invalid {
                    setting: mqtt.path_of("own_topic_prefix"),
                    origin: origin.clone(),
                    message: format!(
                        "got `{v}`, expected topic levels without `+`, `#` and a trailing `/`, not empty"
                    ),
                });
            }
            out.own_topic_prefix = v.to_string();
        }
        let rc = mqtt.section("reconnect")?;
        rc.only(&["initial_delay_ms", "max_delay_ms", "factor"])?;
        if let Some(v) = in_range(
            &rc,
            "initial_delay_ms",
            1..=u64::MAX,
            "milliseconds, at least 1",
        )? {
            out.reconnect.initial_delay_ms = v;
        }
        if let Some(v) = in_range(
            &rc,
            "max_delay_ms",
            1..=u64::MAX,
            "milliseconds, at least 1",
        )? {
            out.reconnect.max_delay_ms = v;
        }
        if let Some((v, origin)) = rc.scalar("factor")? {
            out.reconnect.factor = v
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|f| f.is_finite() && *f >= 1.0)
                .ok_or_else(|| ConfigError::Invalid {
                    setting: rc.path_of("factor"),
                    origin: origin.clone(),
                    message: format!("got `{v}`, expected a number >= 1.0"),
                })?;
        }
        if out.reconnect.max_delay_ms < out.reconnect.initial_delay_ms {
            return Err(ConfigError::Invalid {
                setting: rc.path_of("max_delay_ms"),
                origin: rc.node("max_delay_ms").map_or(
                    zerodds_config::Origin::Default,
                    zerodds_config::read::first_origin,
                ),
                message: format!(
                    "{} is below initial_delay_ms {}",
                    out.reconnect.max_delay_ms, out.reconnect.initial_delay_ms
                ),
            });
        }
        let tls = mqtt.section("tls")?;
        tls.only(&[
            "enabled",
            "ca_file",
            "cert_file",
            "key_file",
            "server_name",
            "verify_hostname",
            "alpn",
        ])?;
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
        // The client certificate and its key go together (mTLS); one without
        // the other is a configuration error and not TLS material that does
        // not load (exit 4).
        if out.broker_tls_client_cert_file.is_empty() != out.broker_tls_client_key_file.is_empty() {
            let (set, missing) = if out.broker_tls_client_cert_file.is_empty() {
                ("key_file", "cert_file")
            } else {
                ("cert_file", "key_file")
            };
            return Err(ConfigError::Invalid {
                setting: tls.path_of(set),
                origin: tls.node(set).map_or(
                    zerodds_config::Origin::Default,
                    zerodds_config::read::first_origin,
                ),
                message: format!(
                    "needs mqtt.tls.{missing}: the client certificate and its key go together"
                ),
            });
        }
        if let Some(v) = tls.string("server_name")? {
            out.broker_tls_server_name = v;
        }
        // Spec §7.1: the broker's certificate is checked against its name;
        // a certificate for another name is reached with `server_name`.
        if tls.boolean("verify_hostname")? == Some(false) {
            return Err(ConfigError::Invalid {
                setting: tls.path_of("verify_hostname"),
                origin: tls.node("verify_hostname").map_or(
                    zerodds_config::Origin::Default,
                    zerodds_config::read::first_origin,
                ),
                message: "the broker certificate is always checked against the host name; \
                          name the host the certificate carries in mqtt.tls.server_name"
                    .to_string(),
            });
        }
        if let Some(list) = tls.strings("alpn")? {
            if let Some(bad) = list.iter().find(|p| p.is_empty() || p.len() > 255) {
                return Err(ConfigError::Invalid {
                    setting: tls.path_of("alpn"),
                    origin: tls.node("alpn").map_or(
                        zerodds_config::Origin::Default,
                        zerodds_config::read::first_origin,
                    ),
                    message: format!(
                        "got `{bad}`, a protocol name is 1 to 255 octets (RFC 7301 §3.1)"
                    ),
                });
            }
            out.broker_tls_alpn = Some(list);
        }
        // Spec §7.1: the scheme and `mqtt.tls.enabled` name one transport.
        let url = parse_broker_url(&out.broker_url)?;
        if url.tls && !out.broker_tls_enabled {
            return Err(ConfigError::Invalid {
                setting: "mqtt.broker_url".to_string(),
                origin: broker_origin,
                message: format!(
                    "{} needs mqtt.tls.enabled: true and mqtt.tls.ca_file",
                    out.broker_url
                ),
            });
        }
        if !url.tls && out.broker_tls_enabled {
            return Err(ConfigError::Invalid {
                setting: "mqtt.tls.enabled".to_string(),
                origin: tls.node("enabled").map_or(
                    zerodds_config::Origin::Default,
                    zerodds_config::read::first_origin,
                ),
                message: format!(
                    "mqtt.tls.enabled: true with the plain URL {}; use mqtts:// or wss://",
                    out.broker_url
                ),
            });
        }

        let auth = root.section("auth")?;
        auth.only(&[
            "mode",
            "bearer_token",
            "bearer_subject",
            "username",
            "password",
            "enhanced",
        ])?;
        if let Some(v) = auth.choice("mode", &super::auth::AuthKind::NAMES)? {
            out.auth_mode = super::auth::AuthKind::parse(&v);
        }
        out.auth_bearer_token = auth.string("bearer_token")?;
        out.auth_bearer_subject = auth.string("bearer_subject")?;
        out.outbound_username = auth.string("username")?;
        out.outbound_password = auth.string("password")?;
        let enhanced = auth.section("enhanced")?;
        enhanced.only(&["method"])?;
        out.auth_enhanced_method = enhanced.string("method")?;

        // Spec §3/§7.3: `acl: { default_deny, rules: [ { subject,
        // allow_publish: [...], allow_subscribe: [...] } ] }`.
        let (acl_rules, default_deny) = read_acl(&root.section("acl")?)?;
        out.acl_rules = acl_rules;
        out.acl_default_deny = default_deny;

        let metrics = root.section("metrics")?;
        metrics.only(&["enabled", "listen", "path"])?;
        if let Some(v) = metrics.boolean("enabled")? {
            out.metrics_enabled = v;
        }
        // `listen`, as §3 of every bridge spec spells it.
        if let Some(v) = metrics.string("listen")? {
            out.metrics_addr = v;
        }
        if let Some((v, origin)) = metrics.scalar("path")? {
            // §8.2: the metrics have a path of their own beside the admin
            // endpoint's `/catalog` and `/healthz`.
            if !v.starts_with('/') || matches!(v, "/" | "/catalog" | "/healthz") || v.contains('?')
            {
                return Err(ConfigError::Invalid {
                    setting: metrics.path_of("path"),
                    origin: origin.clone(),
                    message: format!(
                        "got `{v}`, expected a path beginning with `/` that is not `/`, \
                         `/catalog` or `/healthz` and has no `?`"
                    ),
                });
            }
            out.metrics_path = v.to_string();
        }

        for (path, item) in root.items("topics")? {
            out.topics.push(read_topic(&path, item)?);
        }
        for (path, item) in root.items("tenants")? {
            out.tenants.push(read_tenant(&path, item, out.domain)?);
        }
        check_tenants(&out)?;
        // Spec §7.2: the auth settings of each connection fit together.
        let auth_origin = auth.node("mode").map_or(
            zerodds_config::Origin::Default,
            zerodds_config::read::first_origin,
        );
        for (name, cfg) in std::iter::once((String::new(), out.clone())).chain(
            out.tenants
                .iter()
                .map(|t| (t.name.clone(), out.of_tenant(t))),
        ) {
            super::auth::check(&cfg).map_err(|m| ConfigError::Invalid {
                setting: if name.is_empty() {
                    "auth".to_string()
                } else {
                    format!("tenants[{name}].mqtt")
                },
                origin: auth_origin.clone(),
                message: m,
            })?;
        }
        Ok(out)
    }
}

/// An integer at `key` within `range`; `expect` says what it accepts.
fn in_range(
    section: &Section<'_>,
    key: &str,
    range: core::ops::RangeInclusive<u64>,
    expect: &str,
) -> Result<Option<u64>, ConfigError> {
    let Some((raw, origin)) = section.scalar(key)? else {
        return Ok(None);
    };
    match raw.trim().parse::<u64>() {
        Ok(v) if range.contains(&v) => Ok(Some(v)),
        _ => Err(ConfigError::Invalid {
            setting: section.path_of(key),
            origin: origin.clone(),
            message: format!("got `{raw}`, expected {expect}"),
        }),
    }
}

/// An `acl:` section (Spec §7.3): its rules, each list a list of MQTT topic
/// filters, and its `default_deny`, `None` when not set.
fn read_acl(acl: &Section<'_>) -> Result<(Vec<super::acl::AclRule>, Option<bool>), ConfigError> {
    acl.only(&["default_deny", "rules"])?;
    let default_deny = acl.boolean("default_deny")?;
    let origin = || {
        acl.node("rules").map_or(
            zerodds_config::Origin::Default,
            zerodds_config::read::first_origin,
        )
    };
    let mut rules = Vec::new();
    for (path, rule) in acl.items("rules")? {
        let rule = Section::of(Some(rule), path)?;
        rule.only(&["subject", "allow_publish", "allow_subscribe"])?;
        let Some(subject) = rule.string("subject")?.filter(|s| !s.is_empty()) else {
            return Err(ConfigError::Invalid {
                setting: rule.path_of("subject"),
                origin: origin(),
                message: "an acl rule needs a subject".to_string(),
            });
        };
        let mut lists = [Vec::new(), Vec::new()];
        for (i, key) in ["allow_publish", "allow_subscribe"].iter().enumerate() {
            for filter in rule.strings(key)?.unwrap_or_default() {
                crate::topic_filter::validate_filter(&filter).map_err(|e| {
                    ConfigError::Invalid {
                        setting: rule.path_of(key),
                        origin: origin(),
                        message: format!("`{filter}` is no MQTT topic filter: {e}"),
                    }
                })?;
                lists[i].push(filter);
            }
        }
        let [allow_publish, allow_subscribe] = lists;
        rules.push(super::acl::AclRule {
            subject,
            allow_publish,
            allow_subscribe,
        });
    }
    Ok((rules, default_deny))
}

/// One `tenants[]` entry (Spec §1 L6); `domain` is the top level's, its
/// default.
fn read_tenant(
    path: &str,
    item: &zerodds_config::Node,
    domain: i32,
) -> Result<TenantConfig, ConfigError> {
    let e = Section::of(Some(item), path)?;
    e.only(&["name", "domain", "mqtt", "topics", "acl"])?;
    let invalid = |setting: String, message: &str| ConfigError::Invalid {
        setting,
        origin: zerodds_config::read::first_origin(item),
        message: message.to_string(),
    };
    let name = e
        .string("name")?
        .filter(|n| !n.is_empty())
        .ok_or_else(|| invalid(e.path_of("name"), "a tenant needs a name"))?;
    if name.contains('/') {
        return Err(invalid(e.path_of("name"), "a tenant name has no `/`"));
    }
    let mqtt = e.section("mqtt")?;
    mqtt.only(&["client_id", "username", "password"])?;
    let client_id = mqtt
        .string("client_id")?
        .filter(|c| !c.is_empty())
        .ok_or_else(|| {
            invalid(
                mqtt.path_of("client_id"),
                "a tenant needs the client identifier of its connection",
            )
        })?;
    let mut topics = Vec::new();
    for (tpath, t) in e.items("topics")? {
        topics.push(read_topic(&tpath, t)?);
    }
    let (acl_rules, acl_default_deny) = read_acl(&e.section("acl")?)?;
    Ok(TenantConfig {
        name,
        domain: e
            .parse::<i32>("domain", "a DDS domain id")?
            .unwrap_or(domain),
        client_id,
        username: mqtt.string("username")?,
        password: mqtt.string("password")?,
        topics,
        acl_rules,
        acl_default_deny,
    })
}

/// Spec §1 L6: a tenant name once, a client identifier for one connection:
/// two connections under one identifier take the session from each other
/// (MQTT 5.0 §3.1.4).
fn check_tenants(cfg: &DaemonConfig) -> Result<(), ConfigError> {
    let refuse = |message: String| ConfigError::Invalid {
        setting: "tenants".to_string(),
        origin: zerodds_config::Origin::Default,
        message,
    };
    let mut names = std::collections::BTreeSet::new();
    let mut ids = std::collections::BTreeMap::new();
    ids.insert(cfg.client_id.clone(), "the top level".to_string());
    for t in &cfg.tenants {
        if !names.insert(t.name.clone()) {
            return Err(refuse(format!("the tenant {} is named twice", t.name)));
        }
        if let Some(other) = ids.insert(t.client_id.clone(), format!("tenant {}", t.name)) {
            return Err(refuse(format!(
                "the client identifier {} is used by {other} and tenant {}",
                t.client_id, t.name
            )));
        }
    }
    Ok(())
}

/// See [`zerodds_config::read::split_topic_shorthand`].
pub use zerodds_config::read::split_topic_shorthand;

/// One `topics[]` entry; a bare `DDS[:MQTT]` text is the entry
/// `{dds_name: DDS, mqtt_topic: MQTT}`.
fn read_topic(path: &str, item: &zerodds_config::Node) -> Result<TopicConfig, ConfigError> {
    let mut t = TopicConfig::default();
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
            "schema_url",
            "data_representation",
        ])?;
        t.dds_name = entry.string("dds_name")?.unwrap_or_default();
        t.dds_type = entry.string("dds_type")?.unwrap_or_default();
        t.mqtt_topic = entry.string("mqtt_topic")?.unwrap_or_default();
        t.data_representation = entry.choice("data_representation", &["xcdr1", "xcdr2"])?;
        if let Some(v) = entry.choice("direction", &["in", "out", "bidir"])? {
            t.direction = v;
        }
        // Absent, the QoS and the retain flag are derived (Spec §6); a
        // value that is set is kept as it is.
        t.mqtt_qos = entry
            .choice("mqtt_qos", &["0", "1", "2"])?
            .and_then(|v| v.parse().ok());
        t.retain = entry.boolean("retain")?;
        t.schema_url = entry.string("schema_url")?.filter(|u| !u.is_empty());
        let qos = entry.section("qos")?;
        qos.only(&[
            "reliability",
            "durability",
            "history",
            "lifespan_ms",
            "deadline_ms",
            "liveliness",
            "partition",
        ])?;
        t.lifespan_ms = in_range(
            &qos,
            "lifespan_ms",
            1..=u64::MAX,
            "milliseconds, at least 1",
        )?;
        t.deadline_ms = in_range(
            &qos,
            "deadline_ms",
            1..=u64::MAX,
            "milliseconds, at least 1",
        )?;
        let liveliness = qos.section("liveliness")?;
        liveliness.only(&["kind", "lease_ms"])?;
        if let Some(kind) = liveliness.choice(
            "kind",
            &["automatic", "manual_by_participant", "manual_by_topic"],
        )? {
            t.liveliness_kind = kind;
        }
        t.liveliness_lease_ms = in_range(
            &liveliness,
            "lease_ms",
            1..=u64::MAX,
            "milliseconds, at least 1",
        )?;
        t.partition = qos.strings("partition")?.unwrap_or_default();
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
        history.only(&["kind", "depth"])?;
        if let Some(kind) = history.choice("kind", &["keep_last", "keep_all"])? {
            t.history_kind = kind;
        }
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
    check_mqtt_topic(&t).map_err(|message| ConfigError::Invalid {
        setting: format!("{path}.mqtt_topic"),
        origin: zerodds_config::read::first_origin(item),
        message,
    })?;
    Ok(t)
}

/// Spec §5.1, MQTT 5.0 §4.7: the `mqtt_topic` of a topic the daemon
/// publishes on (`out`, `bidir`) is a Topic Name, without the wildcards `+`
/// and `#` ([MQTT-3.3.2-2]); the one of an `in` topic is a Topic Filter the
/// daemon subscribes to, whose wildcards each fill a level and whose `#` is
/// the last level ([MQTT-4.7.1-1], [MQTT-4.7.1-2]).
///
/// # Errors
/// What the topic breaks, for the configuration error.
pub fn check_mqtt_topic(t: &TopicConfig) -> Result<(), String> {
    use crate::topic_filter::{validate_filter, validate_topic_name};
    let publishes = matches!(t.direction.as_str(), "out" | "bidir");
    let checked = if publishes {
        validate_topic_name(&t.mqtt_topic)
    } else {
        validate_filter(&t.mqtt_topic)
    };
    checked.map_err(|e| {
        if publishes {
            format!(
                "`{}` is no MQTT topic name ({e}): direction {} publishes on it, and a topic \
                 name has no wildcard (MQTT 5.0 §4.7)",
                t.mqtt_topic, t.direction
            )
        } else {
            format!(
                "`{}` is no MQTT topic filter ({e}; MQTT 5.0 §4.7)",
                t.mqtt_topic
            )
        }
    })
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
    fn mqttbridge_5_1_slug_handles_double_colon() {
        assert_eq!(default_mqtt_slug("Chat::Message"), "chat/message");
    }

    #[test]
    fn mqttbridge_5_1_slug_replaces_unsafe_chars() {
        assert_eq!(default_mqtt_slug("My Topic!"), "my_topic_");
    }

    /// Spec §5.1, the third row of its table.
    #[test]
    fn mqttbridge_5_1_slug_of_three_levels() {
        assert_eq!(
            default_mqtt_slug("RoboticArm::Joint::Position"),
            "roboticarm/joint/position"
        );
    }

    /// Spec §5.1, MQTT 5.0 §4.7: a topic the daemon publishes on (`out`,
    /// `bidir`, also the default direction of the shorthand) has a topic
    /// name; a wildcard there prevents the start, naming the entry and its
    /// line.
    #[test]
    fn mqttbridge_5_1_an_out_topic_with_a_wildcard_is_refused() {
        for (yaml, setting) in [
            (
                "topics:\n  - dds_name: T\n    mqtt_topic: \"factory/+/telemetry\"\n    direction: out\n",
                "topics[0].mqtt_topic",
            ),
            (
                "topics:\n  - dds_name: T\n    mqtt_topic: \"a/#\"\n    direction: bidir\n",
                "topics[0].mqtt_topic",
            ),
            ("topics:\n  - A\n  - \"B:b/#\"\n", "topics[1].mqtt_topic"),
            (
                "tenants:\n  - name: t\n    mqtt:\n      client_id: x\n    topics:\n      - dds_name: T\n        mqtt_topic: \"t/+\"\n        direction: out\n",
                "tenants[0].topics[0].mqtt_topic",
            ),
        ] {
            let err = DaemonConfig::load_from_str(yaml).unwrap_err();
            assert!(
                matches!(&err, ConfigError::Invalid { setting: s, .. } if s == setting),
                "{yaml}: {err}"
            );
            let text = err.to_string();
            assert!(text.contains("<string>:"), "{text}");
            assert!(text.contains("no MQTT topic name"), "{text}");
        }
    }

    /// Spec §5.1: an `in` topic subscribes to a topic filter; a wildcard
    /// that does not fill its level, or `#` before the last level, is
    /// refused.
    #[test]
    fn mqttbridge_5_1_an_in_topic_may_be_a_filter() {
        let cfg = DaemonConfig::load_from_str(
            "topics:\n  - dds_name: T\n    mqtt_topic: \"factory/+/telemetry\"\n    direction: in\n  - dds_name: U\n    mqtt_topic: \"sensors/#\"\n    direction: in\n",
        )
        .unwrap();
        assert_eq!(cfg.topics[0].mqtt_topic, "factory/+/telemetry");
        assert_eq!(cfg.topics[1].mqtt_topic, "sensors/#");
        for bad in ["a/#/b", "a/b+", "a#"] {
            let yaml =
                format!("topics:\n  - dds_name: T\n    mqtt_topic: \"{bad}\"\n    direction: in\n");
            let err = DaemonConfig::load_from_str(&yaml).unwrap_err();
            assert!(
                matches!(&err, ConfigError::Invalid { setting: s, .. } if s == "topics[0].mqtt_topic"),
                "{bad}: {err}"
            );
            assert!(err.to_string().contains("no MQTT topic filter"), "{err}");
        }
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
    fn mqttbridge_2_parse_broker_url_reads_the_websocket_schemes() {
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
    fn mqttbridge_2_parse_broker_url_refuses_what_it_would_not_use() {
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
    fn mqttbridge_12_1_config_loads_minimal() {
        let yaml = "\
domain: 7
mqtt:
  broker_url: mqtt://localhost:1883
  client_id: c1
topics:
  - dds_name: T
    direction: bidir
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert_eq!(cfg.domain, 7);
        assert_eq!(cfg.broker_url, "mqtt://localhost:1883");
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

    /// The example of the specification (§3), with its `${VAR}` given a
    /// default so the test does not depend on the environment.
    fn spec_example() -> String {
        let spec = include_str!("../../../../docs/specs/zerodds-mqtt-bridge-1.0.md");
        let section = &spec[spec.find("## §3 Config file format").expect("§3")..];
        let start = section.find("```yaml\n").expect("the example") + "```yaml\n".len();
        let yaml = &section[start..start + section[start..].find("```").expect("its end")];
        let mut out = String::new();
        let mut rest = yaml;
        while let Some(at) = rest.find("${") {
            let end = at + rest[at..].find('}').expect("closing brace");
            let name = &rest[at + 2..end];
            out.push_str(&rest[..at]);
            out.push_str(&format!("${{{name}:-{}}}", name.to_ascii_lowercase()));
            rest = &rest[end + 1..];
        }
        out.push_str(rest);
        out
    }

    /// Spec §3: the example of the specification loads, every key in it
    /// reaches the configuration with the value it names.
    #[test]
    fn mqttbridge_3_the_example_of_the_spec_loads() {
        let cfg = DaemonConfig::load_from_str(&spec_example()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((cfg.domain, cfg.log_level.as_str()), (0, "info"));
        assert_eq!(cfg.broker_url, "mqtts://mosquitto.example.com:8883");
        assert_eq!(cfg.client_id, "zerodds-bridge-prod-01");
        assert_eq!(cfg.protocol_version, crate::version::ProtocolVersion::V5);
        assert_eq!(cfg.username.as_deref(), Some("mqtt_user"));
        assert_eq!(cfg.password.as_deref(), Some("mqtt_password"));
        assert_eq!(cfg.keep_alive_secs, 60);
        assert_eq!(cfg.drain_timeout_secs, 30);
        assert!(!cfg.clean_start);
        assert_eq!(cfg.session_expiry_interval_secs, 86_400);
        assert_eq!(cfg.max_packet_size, 1_048_576);
        assert_eq!(cfg.receive_maximum, 1000);
        assert_eq!(cfg.topic_alias_maximum, 100);
        assert_eq!(cfg.connect_timeout_secs, 30);
        assert!(cfg.broker_tls_enabled);
        assert_eq!(cfg.broker_tls_ca_file, "/etc/zerodds/mqtt-ca.pem");
        assert_eq!(
            cfg.broker_tls_client_cert_file,
            "/etc/zerodds/mqtt-client.pem"
        );
        assert_eq!(
            cfg.broker_tls_client_key_file,
            "/etc/zerodds/mqtt-client.key"
        );
        assert_eq!(cfg.broker_tls_server_name, "mosquitto.example.com");
        assert_eq!(cfg.broker_tls_alpn, Some(vec!["mqtt".to_string()]));
        assert_eq!(
            cfg.reconnect,
            ReconnectConfig {
                initial_delay_ms: 500,
                max_delay_ms: 30_000,
                factor: 2.0
            }
        );
        assert_eq!(cfg.auth_mode, Some(super::super::auth::AuthKind::Password));
        assert_eq!(cfg.topics.len(), 3);
        let chat = &cfg.topics[0];
        assert_eq!(
            (chat.dds_name.as_str(), chat.mqtt_topic.as_str()),
            ("Chat::Message", "chat/message")
        );
        assert_eq!((chat.mqtt_qos, chat.retain), (Some(1), Some(false)));
        assert_eq!(
            chat.schema_url.as_deref(),
            Some("https://schema.example/chat-message.idl")
        );
        assert_eq!(cfg.topics[1].schema_url, None);
        assert_eq!(
            (chat.history_kind.as_str(), chat.history_depth),
            ("keep_last", 10)
        );
        let sensor = &cfg.topics[1];
        assert_eq!((sensor.mqtt_qos, sensor.retain), (Some(0), None));
        assert_eq!(sensor.history_kind, "keep_all");
        let snapshot = &cfg.topics[2];
        assert_eq!((snapshot.mqtt_qos, snapshot.retain), (None, Some(true)));
        assert_eq!(snapshot.effective_mqtt_qos(), 1);
        assert_eq!(snapshot.history_depth, 1);
        assert_eq!(cfg.acl_default_deny, Some(false));
        assert_eq!(
            cfg.acl_rules,
            vec![super::super::acl::AclRule {
                subject: "alice".into(),
                allow_publish: vec!["chat/+".into()],
                allow_subscribe: vec!["chat/+".into(), "sensors/+".into()],
            }]
        );
        assert_eq!(cfg.tenants.len(), 1);
        let plant = &cfg.tenants[0];
        assert_eq!((plant.name.as_str(), plant.domain), ("plant-a", 7));
        assert_eq!(plant.client_id, "zerodds-bridge-plant-a");
        assert_eq!(plant.username.as_deref(), Some("plant_a_user"));
        assert_eq!(plant.password.as_deref(), Some("plant_a_password"));
        assert_eq!(plant.topics[0].dds_name, "Plant::Status");
        assert_eq!(plant.acl_default_deny, Some(false));
        assert!(cfg.metrics_enabled);
        assert_eq!(cfg.metrics_addr, "127.0.0.1:9092");
        assert_eq!(cfg.metrics_path, "/metrics");
        assert_eq!(cfg.own_topic_prefix, "$zerodds");
        assert_eq!(chat.data_representation.as_deref(), Some("xcdr2"));
    }

    /// Spec §6: an absent `mqtt_qos` or `retain` is derived; one that is set
    /// stays as it is, also against the derivation.
    #[test]
    fn mqttbridge_3_an_absent_mqtt_qos_is_none() {
        let cfg = DaemonConfig::load_from_str(
            "topics:\n  - dds_name: A\n    qos:\n      reliability: reliable\n      durability: transient_local\n  - dds_name: B\n    mqtt_qos: 0\n    retain: false\n    qos:\n      reliability: reliable\n      durability: transient_local\n  - dds_name: C\n    qos:\n      reliability: best_effort\n",
        )
        .unwrap();
        let [a, b, c] = &cfg.topics[..] else {
            panic!("three topics")
        };
        assert_eq!((a.mqtt_qos, a.retain), (None, None));
        assert_eq!((a.effective_mqtt_qos(), a.effective_retain()), (1, true));
        assert_eq!((b.mqtt_qos, b.retain), (Some(0), Some(false)));
        assert_eq!((b.effective_mqtt_qos(), b.effective_retain()), (0, false));
        assert_eq!((c.effective_mqtt_qos(), c.effective_retain()), (0, false));
    }

    /// Spec §3: the defaults of the keys a file does not set.
    #[test]
    fn mqttbridge_3_the_defaults_of_the_keys() {
        let cfg = DaemonConfig::load_from_str("domain: 1\n").unwrap();
        assert_eq!(cfg.protocol_version, crate::version::ProtocolVersion::V5);
        assert_eq!(cfg.session_expiry_interval_secs, 0);
        #[cfg(target_os = "linux")]
        assert_eq!(cfg.session_dir, "/var/lib/zerodds/mqtt-bridge");
        assert_eq!(cfg.session_dir, default_session_dir());
        let dir =
            DaemonConfig::load_from_str("mqtt:\n  session_dir: /srv/mqtt-sessions\n").unwrap();
        assert_eq!(dir.session_dir, "/srv/mqtt-sessions");
        assert_eq!(cfg.max_packet_size, MAX_PACKET_SIZE);
        assert_eq!(cfg.receive_maximum, 65_535);
        assert_eq!(cfg.topic_alias_maximum, 100);
        assert_eq!(cfg.connect_timeout_secs, 30);
        assert_eq!(cfg.reconnect, ReconnectConfig::default());
        assert_eq!(cfg.broker_tls_alpn, None);
        assert_eq!(cfg.metrics_path, "/metrics");
        assert!(cfg.tenants.is_empty());
        let t = DaemonConfig::load_from_str("topics:\n  - T\n").unwrap();
        assert_eq!(
            (t.topics[0].history_kind.as_str(), t.topics[0].history_depth),
            ("keep_last", 10)
        );
        assert_eq!(t.topics[0].history_depth(), 10);
        let all = DaemonConfig::load_from_str(
            "topics:\n  - dds_name: T\n    qos:\n      history: { kind: keep_all }\n",
        )
        .unwrap();
        assert_eq!(all.topics[0].history_depth(), usize::MAX);
        let v311 = DaemonConfig::load_from_str("mqtt:\n  protocol_version: 3.1.1\n").unwrap();
        assert_eq!(v311.protocol_version, crate::version::ProtocolVersion::V311);
        assert_eq!(cfg.own_topic_prefix, "$zerodds");
    }

    /// Spec §3: `data_representation` names the one representation of the
    /// topic's endpoints; not set, it is none.
    #[test]
    fn mqttbridge_3_data_representation_names_the_representation_of_the_topic() {
        use zerodds_rtps::publication_data::data_representation as dr;
        let cfg = DaemonConfig::load_from_str(
            "topics:\n  - dds_name: A\n    data_representation: xcdr1\n  - dds_name: B\n    data_representation: xcdr2\n  - dds_name: C\n",
        )
        .unwrap();
        let ids: Vec<Option<i16>> = cfg
            .topics
            .iter()
            .map(TopicConfig::data_representation_id)
            .collect();
        assert_eq!(ids, [Some(dr::XCDR), Some(dr::XCDR2), None]);
    }

    /// Spec §3, §5.2, §6: `mqtt.own_topic_prefix` names the levels in front
    /// of `<client_id>/` of the daemon's own topics; a tenant's connection
    /// takes the one of the top level.
    #[test]
    fn mqttbridge_3_the_own_topic_prefix_is_read_and_reaches_the_tenants() {
        let cfg = DaemonConfig::load_from_str(
            "mqtt:\n  own_topic_prefix: \"plant/zerodds\"\ntenants:\n  - name: a\n    mqtt:\n      client_id: ta\n    topics:\n      - dds_name: A\n        direction: out\n",
        )
        .unwrap();
        assert_eq!(cfg.own_topic_prefix, "plant/zerodds");
        assert_eq!(
            cfg.of_tenant(&cfg.tenants[0]).own_topic_prefix,
            "plant/zerodds"
        );
        let dollar = DaemonConfig::load_from_str("mqtt:\n  own_topic_prefix: \"$sys2\"\n").unwrap();
        assert_eq!(dollar.own_topic_prefix, "$sys2");
    }

    /// Spec §2: without `mqtt.client_id` the client identifier names the
    /// host; the oracle is the host name the operating system reports.
    #[test]
    fn mqttbridge_2_the_default_client_id_names_the_host() {
        let reported = std::process::Command::new("hostname")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        let cfg = DaemonConfig::load_from_str("domain: 0\n").unwrap();
        if reported.is_empty() {
            assert_eq!(cfg.client_id, "zerodds-bridge");
        } else {
            assert_eq!(cfg.client_id, format!("zerodds-bridge-{reported}"));
        }
    }

    /// Spec §9.1/§9.3: the waits grow by `factor` from `initial_delay_ms` up
    /// to `max_delay_ms`.
    #[test]
    fn mqttbridge_3_the_reconnect_delays_grow_by_the_factor_up_to_the_maximum() {
        let rc = ReconnectConfig {
            initial_delay_ms: 500,
            max_delay_ms: 3000,
            factor: 2.0,
        };
        let mut d = rc.first_delay();
        let mut seen = vec![d.as_millis()];
        for _ in 0..4 {
            d = rc.next_delay(d);
            seen.push(d.as_millis());
        }
        assert_eq!(seen, [500, 1000, 2000, 3000, 3000]);
        let slow = ReconnectConfig {
            initial_delay_ms: 100,
            max_delay_ms: 1000,
            factor: 1.5,
        };
        assert_eq!(slow.next_delay(slow.first_delay()).as_millis(), 150);
    }

    /// Spec §1 L6: the tenants with their participant, client identifier,
    /// credentials, topics and ACL; a tenant configuration is the top
    /// level's with these replaced.
    #[test]
    fn mqttbridge_1_l6_the_tenants() {
        let cfg = DaemonConfig::load_from_str(
            "domain: 3\nmqtt:\n  client_id: top\n  username: u0\nauth:\n  mode: sasl\n  username: u0\n  password: p0\nacl:\n  default_deny: true\ntenants:\n  - name: a\n    domain: 7\n    mqtt:\n      client_id: id-a\n      username: ua\n      password: pa\n    topics:\n      - dds_name: A\n        direction: out\n    acl:\n      default_deny: false\n  - name: b\n    mqtt:\n      client_id: id-b\n    topics:\n      - B:b/topic\n",
        )
        .unwrap();
        assert_eq!(cfg.tenants.len(), 2);
        let a = cfg.of_tenant(&cfg.tenants[0]);
        assert_eq!((a.domain, a.client_id.as_str()), (7, "id-a"));
        assert_eq!(a.topics.len(), 1);
        assert!(a.tenants.is_empty());
        // The `tenant` of its log lines (§8.1); the top level has none.
        assert_eq!((a.tenant.as_str(), cfg.tenant.as_str()), ("a", ""));
        let ka = super::super::auth::Credentials::of(&a);
        assert_eq!(
            (
                ka.connect_user_name().as_deref(),
                ka.connect_password().as_deref()
            ),
            (Some("ua"), Some(b"pa".as_ref()))
        );
        assert_eq!(a.acl_default_deny, Some(false));
        let b = cfg.of_tenant(&cfg.tenants[1]);
        assert_eq!((b.domain, b.client_id.as_str()), (3, "id-b"));
        assert_eq!(b.topics[0].mqtt_topic, "b/topic");
        let kb = super::super::auth::Credentials::of(&b);
        assert_eq!(
            (
                kb.connect_user_name().as_deref(),
                kb.connect_password().as_deref()
            ),
            (Some("u0"), Some(b"p0".as_ref()))
        );
        // The tenant without `acl` does not take the top level's rules.
        assert_eq!(b.acl_default_deny, None);
        assert!(b.acl_rules.is_empty());
        for (yaml, needle) in [
            ("tenants:\n  - mqtt:\n      client_id: x\n", "needs a name"),
            (
                "tenants:\n  - name: a/b\n    mqtt:\n      client_id: x\n",
                "no `/`",
            ),
            ("tenants:\n  - name: a\n", "client identifier"),
            (
                "tenants:\n  - name: a\n    mqtt:\n      client_id: x\n  - name: a\n    mqtt:\n      client_id: y\n",
                "named twice",
            ),
            (
                "tenants:\n  - name: a\n    mqtt:\n      client_id: x\n  - name: b\n    mqtt:\n      client_id: x\n",
                "tenant a and tenant b",
            ),
            (
                "mqtt:\n  client_id: x\ntenants:\n  - name: a\n    mqtt:\n      client_id: x\n",
                "the top level and tenant a",
            ),
            (
                "tenants:\n  - name: a\n    mqtt:\n      client_id: x\n    broker: y\n",
                "broker",
            ),
        ] {
            let err = DaemonConfig::load_from_str(yaml)
                .expect_err(yaml)
                .to_string();
            assert!(err.contains(needle), "{yaml}: {err}");
        }
    }

    /// Spec §1 L6 with §7.2: a tenant authenticates in the mode of the top
    /// level. Its `mqtt.password` is the token where the top level presents
    /// `auth.bearer_token` (under `password` and under a token method of
    /// `enhanced`), the SCRAM password under SCRAM-SHA-256; a user name or
    /// password the mode does not use prevents the start, naming the
    /// tenant.
    #[test]
    fn mqttbridge_1_l6_a_tenant_authenticates_in_the_mode_of_the_top_level() {
        use super::super::auth::{Credentials, Exchange};
        let tenant =
            |mqtt: &str| format!("tenants:\n  - name: a\n    mqtt:\n      client_id: id-a\n{mqtt}");
        let oauth = "auth:\n  mode: enhanced\n  bearer_token: tok-top\n  enhanced:\n    method: OAUTHBEARER\n";
        let cfg =
            DaemonConfig::load_from_str(&format!("{oauth}{}", tenant("      password: tok-a\n")))
                .expect("a tenant with its password under OAUTHBEARER");
        let a = cfg.of_tenant(&cfg.tenants[0]);
        assert_eq!(Credentials::of(&a).token(), Some("tok-a"));
        assert_eq!(Credentials::of(&cfg).token(), Some("tok-top"));
        let x = Exchange::of(&a, "broker", 1883).unwrap().unwrap();
        assert_eq!(
            x.initial(),
            b"n,,\x01host=broker\x01port=1883\x01auth=Bearer tok-a\x01\x01".to_vec()
        );
        let err = DaemonConfig::load_from_str(&format!(
            "{oauth}{}",
            tenant("      username: ua\n      password: tok-a\n")
        ))
        .expect_err("a tenant user name under OAUTHBEARER")
        .to_string();
        assert!(
            err.contains("tenants[a].mqtt") && err.contains("does not use mqtt.username"),
            "{err}"
        );
        assert!(!err.contains("auth.username"), "{err}");
        let jwt = "auth:\n  mode: enhanced\n  bearer_token: t0\n  enhanced:\n    method: JWT\n";
        let cfg = DaemonConfig::load_from_str(&format!("{jwt}{}", tenant("      password: t-a\n")))
            .unwrap();
        assert_eq!(
            Exchange::of(&cfg.of_tenant(&cfg.tenants[0]), "h", 1)
                .unwrap()
                .unwrap()
                .initial(),
            b"t-a".to_vec()
        );
        let scram = "auth:\n  mode: enhanced\n  username: u0\n  password: p0\n  enhanced:\n    method: SCRAM-SHA-256\n";
        let cfg = DaemonConfig::load_from_str(&format!(
            "{scram}{}",
            tenant("      username: ua\n      password: pa\n")
        ))
        .unwrap();
        let a = cfg.of_tenant(&cfg.tenants[0]);
        let k = Credentials::of(&a);
        assert_eq!((k.user(), k.password()), (Some("ua"), Some("pa")));
        let bearer = "auth:\n  mode: bearer\n  bearer_token: tok-top\n";
        let cfg = DaemonConfig::load_from_str(&format!(
            "{bearer}{}",
            tenant("      username: ua\n      password: tok-a\n")
        ))
        .unwrap();
        let a = cfg.of_tenant(&cfg.tenants[0]);
        let k = Credentials::of(&a);
        assert_eq!(
            (k.connect_user_name().as_deref(), k.connect_password()),
            (Some("ua"), Some(b"tok-a".to_vec()))
        );
        let mtls = "mqtt:\n  broker_url: mqtts://broker.example:8883\n  tls:\n    enabled: true\n    cert_file: /c.pem\n    key_file: /k.pem\nauth:\n  mode: mtls\n";
        let err = DaemonConfig::load_from_str(&format!("{mtls}{}", tenant("      password: pa\n")))
            .expect_err("a tenant password under mtls")
            .to_string();
        assert!(
            err.contains("tenants[a].mqtt") && err.contains("mqtt.password"),
            "{err}"
        );
    }

    /// Spec §3: a value outside what a key accepts prevents the start, with
    /// the key and the line.
    #[test]
    fn mqttbridge_3_the_new_keys_refuse_what_they_do_not_accept() {
        for (yaml, setting) in [
            ("mqtt:\n  protocol_version: 4\n", "mqtt.protocol_version"),
            ("mqtt:\n  max_packet_size: 0\n", "mqtt.max_packet_size"),
            (
                "mqtt:\n  max_packet_size: 16777217\n",
                "mqtt.max_packet_size",
            ),
            ("mqtt:\n  receive_maximum: 0\n", "mqtt.receive_maximum"),
            ("mqtt:\n  session_dir: \"\"\n", "mqtt.session_dir"),
            ("mqtt:\n  own_topic_prefix: \"\"\n", "mqtt.own_topic_prefix"),
            (
                "topics:\n  - dds_name: T\n    data_representation: xcdr3\n",
                "topics[0].data_representation",
            ),
            (
                "mqtt:\n  own_topic_prefix: \"zerodds/+\"\n",
                "mqtt.own_topic_prefix",
            ),
            (
                "mqtt:\n  own_topic_prefix: \"zerodds/#\"\n",
                "mqtt.own_topic_prefix",
            ),
            (
                "mqtt:\n  own_topic_prefix: \"zerodds/\"\n",
                "mqtt.own_topic_prefix",
            ),
            (
                "mqtt:\n  topic_alias_maximum: 70000\n",
                "mqtt.topic_alias_maximum",
            ),
            (
                "mqtt:\n  session_expiry_interval_secs: -1\n",
                "mqtt.session_expiry_interval_secs",
            ),
            (
                "mqtt:\n  reconnect:\n    factor: 0.5\n",
                "mqtt.reconnect.factor",
            ),
            (
                "mqtt:\n  reconnect:\n    initial_delay_ms: 0\n",
                "mqtt.reconnect.initial_delay_ms",
            ),
            (
                "mqtt:\n  reconnect:\n    initial_delay_ms: 900\n    max_delay_ms: 100\n",
                "mqtt.reconnect.max_delay_ms",
            ),
            (
                "mqtt:\n  reconnect:\n    jitter: 1\n",
                "mqtt.reconnect.jitter",
            ),
            (
                "mqtt:\n  tls:\n    verify_hostname: false\n",
                "mqtt.tls.verify_hostname",
            ),
            ("mqtt:\n  tls:\n    alpn: [\"\"]\n", "mqtt.tls.alpn"),
            (
                "mqtt:\n  tls:\n    cert_file: /c.pem\n",
                "mqtt.tls.cert_file",
            ),
            ("mqtt:\n  tls:\n    key_file: /k.pem\n", "mqtt.tls.key_file"),
            ("metrics:\n  path: /catalog\n", "metrics.path"),
            ("metrics:\n  path: metrics\n", "metrics.path"),
            (
                "topics:\n  - dds_name: T\n    qos:\n      history: { kind: keep_some }\n",
                "topics[0].qos.history.kind",
            ),
            ("mqtt:\n  broker_url: mqtts://b\n", "mqtt.broker_url"),
            (
                "mqtt:\n  broker_url: mqtt://b\n  tls:\n    enabled: true\n",
                "mqtt.tls.enabled",
            ),
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
