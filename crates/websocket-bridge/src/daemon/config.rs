// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Configuration of `zerodds-ws-bridged`.
//!
//! Spec: `zerodds-ws-bridge-1.0.md` §3. The file is read by the shared
//! layered loader (`zerodds-config`): base file, `ws-bridged.d/` drop-ins,
//! includes, `${VAR}` substitution, and the precedence defaults <
//! environment < files < `--set`. The DDS runtime's own keys
//! (`transport.*`, `discovery.*`, …) may sit in the same file.
//!
//! [`DaemonConfig::from_config`] reads this daemon's keys out of the merged
//! tree. A key inside a known section that the schema does not name, and a
//! value of the wrong shape or type, is refused with its file and line.

use std::path::Path;
use std::string::{String, ToString};
use std::vec::Vec;

pub use zerodds_config::ConfigError;
use zerodds_config::read::Section;

/// The base name of the daemon's file in the system configuration
/// directory: `ws-bridged.yaml`, `ws-bridged.d/`.
pub const CONFIG_STEM: &str = "ws-bridged";

/// Every key this daemon reads, as a dotted path; list-valued keys stop at
/// the list (`topics`). The unknown-key check of the loader uses it.
pub const KEYS: &[&str] = &[
    "listen",
    "domain",
    "log_level",
    "tls.enabled",
    "tls.cert_file",
    "tls.key_file",
    "tls.client_ca_file",
    "auth.mode",
    "auth.bearer_token",
    "auth.bearer_subject",
    "auth.jwt.public_key",
    "auth.jwt.issuer",
    "auth.jwt.audience",
    "auth.jwt.required_claim",
    "metrics.enabled",
    "metrics.listen",
    "metrics.path",
    "limits.max_frame_size",
    "limits.max_message_size",
    "origin.allowed",
    "origin.required",
    "max_send_queue_per_connection",
    "max_burst_samples",
    "ping_interval_ms",
    "shutdown_drain_ms",
    "catalog_on_connect",
    "topics",
    "tenants",
];

/// Whether `path` is one of [`KEYS`].
#[must_use]
pub fn is_key(path: &str) -> bool {
    KEYS.contains(&path)
}

/// Parsed daemon config.
#[derive(Debug, Clone)]
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
    /// `auth.mode` — `none|bearer|jwt|mtls`. Spec §7.2.
    pub auth_mode: String,
    /// `auth.bearer_token` — single-token form (map with one entry).
    pub auth_bearer_token: Option<String>,
    /// `auth.bearer_subject` — who is behind the bearer.
    pub auth_bearer_subject: Option<String>,
    /// `auth.jwt.*` — Spec §3, for `auth.mode: jwt`.
    pub auth_jwt: zerodds_bridge_security::JwtSettings,
    /// Topic ACL from `topics[].acl` — `topic → (read subjects,
    /// write subjects)`. Spec §3/§7.3.
    pub topic_acl: std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
    /// `metrics.enabled` — toggles the Prometheus endpoint (§8.2).
    pub metrics_enabled: bool,
    /// Bind address for the admin endpoint (`/metrics`, `/catalog`,
    /// `/healthz`). If empty but `metrics_enabled=true`: default
    /// `127.0.0.1:9091`. Overridable via CLI/`metrics.listen`.
    pub metrics_addr: String,
    /// `metrics.path` — the path of the Prometheus metrics on the admin
    /// endpoint (Spec §3, §4.2). Default [`DEFAULT_METRICS_PATH`].
    pub metrics_path: String,
    /// `limits.max_frame_size` — the largest payload of one frame of a
    /// client, in octets (RFC 6455 §10.4; Spec §3, §4.5). At least
    /// [`MIN_MAX_FRAME_SIZE`].
    pub max_frame_size: usize,
    /// `limits.max_message_size` — the largest message of a client after
    /// reassembly and decompression, in octets (RFC 6455 §10.4; Spec §3,
    /// §4.5). At least 1.
    pub max_message_size: usize,
    /// `origin.allowed` — the origins the daemon expects in the `Origin`
    /// field of the opening handshake (RFC 6455 §10.2; Spec §3, §4.1):
    /// `scheme://host[:port]`, `null`, or `*`. Empty: no check.
    pub origin_allowed: Vec<String>,
    /// `origin.required` — refuse a handshake without `Origin`. Needs
    /// `origin.allowed`.
    pub origin_required: bool,
    /// `max_send_queue_per_connection` — the samples that wait at most in
    /// the send queue of one connection (Spec §6). At least 1.
    pub max_send_queue_per_connection: usize,
    /// `max_burst_samples` — the samples a durable topic keeps at most and
    /// sends to a connection that subscribes (Spec §6). At least 1.
    pub max_burst_samples: usize,
    /// `ping_interval_ms` — the interval of the daemon's WebSocket Ping on
    /// every connection (Spec §6, RFC 6455 §5.5.2); a connection without a
    /// Pong by the next Ping is closed with 1011. 0: no Ping.
    pub ping_interval_ms: u64,
    /// `shutdown_drain_ms` — how long a shutdown waits at most for the
    /// pending DDS samples to reach the clients, the `shutdown` frames and
    /// the Close handshakes (Spec §9.2). At least 1.
    pub shutdown_drain_ms: u64,
    /// `catalog_on_connect` — every WebSocket connection gets the catalog
    /// frame of Spec §5.2 as its first frame, not only one of
    /// `/topics/__catalog__` (§4.2). Default false.
    pub catalog_on_connect: bool,
    /// `tenants` (Spec §1 L6): DomainParticipants beside the one of
    /// `domain`, each with its topics; a connection whose authenticated
    /// subject a tenant names is bound to it.
    pub tenants: Vec<TenantConfig>,
}

/// A tenant (Spec §1 L6, §3 `tenants`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TenantConfig {
    /// `name:` — unique; in the log, the spans and the metrics.
    pub name: String,
    /// `domain:` — the DDS domain of its participant; default the
    /// top-level `domain`.
    pub domain: i32,
    /// `subjects:` — the authenticated subjects bound to it (Spec §7.3:
    /// the bearer subject, the JWT `sub`, the client certificate's subject
    /// DN or its `mtls:` fingerprint, `anonymous` under `auth.mode: none`).
    pub subjects: Vec<String>,
    /// `topics:` — as the top-level `topics`. Their `acl` is in
    /// [`DaemonConfig::topic_acl`] under [`tenant_acl_topic`].
    pub topics: Vec<TopicConfig>,
}

impl TenantConfig {
    /// Whether `subject` is bound to this tenant: its name or its mTLS
    /// fingerprint is one of `subjects`.
    #[must_use]
    pub fn binds(&self, subject: &zerodds_bridge_security::AuthSubject) -> bool {
        self.subjects
            .iter()
            .any(|s| *s == subject.name || subject.fingerprint.as_deref() == Some(s.as_str()))
    }
}

/// The key of a tenant's topic in [`DaemonConfig::topic_acl`]:
/// `<tenant>/<topic>`, so that a tenant's topic of the name of a top-level
/// topic (or of another tenant's) has an ACL of its own. The same string is
/// the `topic` label of the topic's metrics (Spec §8.2).
#[must_use]
pub fn tenant_acl_topic(tenant: &str, topic: &str) -> String {
    let mut out = String::with_capacity(tenant.len() + 1 + topic.len());
    out.push_str(tenant);
    out.push('/');
    out.push_str(topic);
    out
}

/// The default of `shutdown_drain_ms` (Spec §9.2: 30 s).
pub const DEFAULT_SHUTDOWN_DRAIN_MS: u64 = 30_000;

/// The default of `ping_interval_ms`.
pub const DEFAULT_PING_INTERVAL_MS: u64 = 30_000;

/// The smallest `ping_interval_ms` other than 0.
pub const MIN_PING_INTERVAL_MS: u64 = 100;

/// The smallest `limits.max_frame_size`: a control frame carries up to 125
/// octets (RFC 6455 §5.5), and a limit below that would refuse frames the
/// protocol needs.
pub const MIN_MAX_FRAME_SIZE: usize = 125;

/// The default of `metrics.path`.
pub const DEFAULT_METRICS_PATH: &str = "/metrics";

/// The path of the catalog meta endpoint (Spec §4.2); no topic may take it.
pub const CATALOG_PATH: &str = "/topics/__catalog__";

/// The path of the health endpoint on the WebSocket listener (Spec §4.2).
pub const HEALTH_PATH: &str = "/healthz";

/// Single topic map entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TopicConfig {
    /// `name:` — DDS topic name.
    pub name: String,
    /// `type:` — DDS type name.
    pub type_name: String,
    /// `direction:` — `in|out|bidir`.
    pub direction: String,
    /// `ws_path:` — the path of the topic's WebSocket endpoint (§4.2).
    /// Empty: the default path of §5.1, see [`Self::path`].
    pub ws_path: String,
    /// `qos.reliability:`.
    pub reliability: String,
    /// `qos.durability:`.
    pub durability: String,
    /// `qos.history.kind:` — `keep_last` or `keep_all`. Empty: `keep_last`.
    pub history_kind: String,
    /// `qos.history.depth:`.
    pub history_depth: i32,
    /// `qos.deadline_ms:` — the deadline of each instance (Spec §6).
    pub deadline_ms: Option<u64>,
    /// `qos.liveliness_lease_ms:` — the AUTOMATIC liveliness lease of the
    /// daemon's DataWriter and DataReader of the topic (Spec §6).
    pub liveliness_lease_ms: Option<u64>,
    /// `qos.partition:` — the partitions of the daemon's DataWriter and
    /// DataReader of the topic (Spec §6). Empty: the default partition.
    pub partition: Vec<String>,
    /// `idl_file:` — the IDL file that declares `type`; the JSON text
    /// frames (§4.3) read and write the topic's samples with it. Empty:
    /// none, and a JSON-mode client cannot subscribe to or publish on the
    /// topic.
    pub idl_file: String,
}

impl TopicConfig {
    /// The path of the topic's WebSocket endpoint (Spec §4.2): `ws_path`,
    /// or the default path of §5.1 where the entry names none. A topic is
    /// served at this path alone.
    #[must_use]
    pub fn path(&self) -> String {
        if self.ws_path.is_empty() {
            default_ws_path(&self.name)
        } else {
            self.ws_path.clone()
        }
    }

    /// Whether the history keeps every sample (`qos.history.kind:
    /// keep_all`); else the last `qos.history.depth`.
    #[must_use]
    pub fn keep_all(&self) -> bool {
        self.history_kind == "keep_all"
    }
}

impl Default for DaemonConfig {
    /// [`Self::default_for_dev`]: a configuration without limits would
    /// refuse every frame.
    fn default() -> Self {
        Self::default_for_dev()
    }
}

/// Whether every address `listen` names is a loopback address: an
/// `ip:port` is checked as it stands, a `name:port` such as
/// `localhost:8080` by what it resolves to (no address: not loopback).
#[must_use]
pub fn listen_is_loopback(listen: &str) -> bool {
    use std::net::ToSocketAddrs as _;
    if let Ok(addr) = listen.parse::<std::net::SocketAddr>() {
        return addr.ip().is_loopback();
    }
    match listen.to_socket_addrs() {
        Ok(addrs) => {
            let addrs: Vec<_> = addrs.collect();
            !addrs.is_empty() && addrs.iter().all(|a| a.ip().is_loopback())
        }
        Err(_) => false,
    }
}

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
            auth_jwt: zerodds_bridge_security::JwtSettings::default(),
            topic_acl: std::collections::HashMap::new(),
            metrics_enabled: false,
            metrics_addr: String::new(),
            metrics_path: DEFAULT_METRICS_PATH.to_string(),
            max_frame_size: crate::connection::DEFAULT_MAX_FRAME_SIZE,
            max_message_size: crate::connection::DEFAULT_MAX_MESSAGE_SIZE,
            origin_allowed: Vec::new(),
            origin_required: false,
            max_send_queue_per_connection: super::qos_translation::DEFAULT_MAX_SEND_QUEUE,
            max_burst_samples: super::qos_translation::DEFAULT_MAX_BURST,
            ping_interval_ms: DEFAULT_PING_INTERVAL_MS,
            shutdown_drain_ms: DEFAULT_SHUTDOWN_DRAIN_MS,
            catalog_on_connect: false,
            tenants: Vec::new(),
        }
    }

    /// The daemon-wide bounds of §6.
    #[must_use]
    pub fn queue_limits(&self) -> super::qos_translation::QueueLimits {
        super::qos_translation::QueueLimits {
            max_send_queue: self.max_send_queue_per_connection,
            max_burst: self.max_burst_samples,
        }
    }

    /// The limits of a connection's receiver (RFC 6455 §10.4), from
    /// `limits.*`.
    #[must_use]
    pub fn limits(&self) -> crate::connection::Limits {
        crate::connection::Limits {
            max_frame_size: self.max_frame_size,
            max_message_size: self.max_message_size,
        }
    }

    /// The origin check of the opening handshake (RFC 6455 §10.2), from
    /// `origin.*`.
    ///
    /// # Errors
    /// [`crate::origin::OriginPolicyError`] for an entry that is no origin
    /// and for `origin.required` without `origin.allowed`.
    pub fn origin_policy(
        &self,
    ) -> Result<crate::origin::OriginPolicy, crate::origin::OriginPolicyError> {
        crate::origin::OriginPolicy::new(&self.origin_allowed, self.origin_required)
    }

    /// Spec §7.2, the auth settings that need no file: `auth.mode: none` is
    /// the development mode and is served on a loopback `listen` address
    /// only; `bearer` checks against `auth.bearer_token`, so it needs one
    /// that is not empty (an unset `${VAR}` expands to nothing). Checked at
    /// start, on a SIGHUP reload (against the address served) and by
    /// `--check-config`.
    ///
    /// # Errors
    /// The message for `none` (or no mode) on an address that is not
    /// loopback, and for `bearer` without a token.
    pub fn check_auth(&self) -> Result<(), String> {
        match self.auth_mode.as_str() {
            "" | "none" if !listen_is_loopback(&self.listen) => Err(format!(
                "auth.mode: none needs a loopback listen address (127.0.0.0/8 or ::1), got {}",
                self.listen
            )),
            "bearer" if self.auth_bearer_token.as_deref().is_none_or(str::is_empty) => {
                Err("auth.mode: bearer needs a non-empty auth.bearer_token".to_string())
            }
            _ => Ok(()),
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

    /// Parses a config from a YAML string, for tests.
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
    pub fn from_config(config: Option<&zerodds_config::Config>) -> Result<Self, ConfigError> {
        let root = Section::root(config.and_then(zerodds_config::Config::root))?;
        let mut out = Self::default_for_dev();
        if let Some(v) = root.string("listen")? {
            out.listen = v;
        }
        if let Some(v) = root.parse::<i32>("domain", "a DDS domain id")? {
            out.domain = v;
        }
        if let Some(v) = root.choice("log_level", &["trace", "debug", "info", "warn", "error"])? {
            out.log_level = v;
        }

        let tls = root.section("tls")?;
        tls.only(&["enabled", "cert_file", "key_file", "client_ca_file"])?;
        if let Some(v) = tls.boolean("enabled")? {
            out.tls_enabled = v;
        }
        if let Some(v) = tls.string("cert_file")? {
            out.tls_cert_file = v;
        }
        if let Some(v) = tls.string("key_file")? {
            out.tls_key_file = v;
        }
        if let Some(v) = tls.string("client_ca_file")? {
            out.tls_client_ca_file = v;
        }

        let auth = root.section("auth")?;
        auth.only(&["mode", "bearer_token", "bearer_subject", "jwt"])?;
        // Spec §7.2 names four modes; `sasl` was accepted too and then
        // failed the start for want of a user list no key could supply.
        if let Some(v) = auth.choice("mode", &["none", "bearer", "jwt", "mtls"])? {
            out.auth_mode = v;
        }
        out.auth_bearer_token = auth.string("bearer_token")?;
        out.auth_bearer_subject = auth.string("bearer_subject")?;
        let jwt = auth.section("jwt")?;
        jwt.only(&["public_key", "issuer", "audience", "required_claim"])?;
        out.auth_jwt = zerodds_bridge_security::JwtSettings {
            public_key: jwt.string("public_key")?.unwrap_or_default(),
            issuer: jwt.string("issuer")?,
            audience: jwt.string("audience")?,
            required_claim: jwt.string("required_claim")?,
        };

        let metrics = root.section("metrics")?;
        metrics.only(&["enabled", "listen", "path"])?;
        if let Some(v) = metrics.boolean("enabled")? {
            out.metrics_enabled = v;
        }
        // `listen`, as §3 of every bridge spec spells it.
        if let Some(v) = metrics.string("listen")? {
            out.metrics_addr = v;
        }
        if let Some(v) = metrics.string("path")? {
            // `/catalog` and `/healthz` are the admin endpoint's own.
            if !is_http_path(&v) || ["/", "/catalog", HEALTH_PATH].contains(&v.as_str()) {
                return Err(ConfigError::Invalid {
                    setting: metrics.path_of("path"),
                    origin: metrics.node("path").map_or(
                        zerodds_config::Origin::Default,
                        zerodds_config::read::first_origin,
                    ),
                    message: format!(
                        "got `{v}`, expected a path that begins with `/`, other than `/`, `/catalog` and `/healthz`"
                    ),
                });
            }
            out.metrics_path = v;
        }

        let limits = root.section("limits")?;
        limits.only(&["max_frame_size", "max_message_size"])?;
        if let Some(v) = read_size(&limits, "max_frame_size", MIN_MAX_FRAME_SIZE)? {
            out.max_frame_size = v;
        }
        if let Some(v) = read_size(&limits, "max_message_size", 1)? {
            out.max_message_size = v;
        }

        if let Some(v) = read_bounded(
            &root,
            "max_send_queue_per_connection",
            1,
            u64::from(u32::MAX),
            "a number of samples >= 1",
        )? {
            out.max_send_queue_per_connection = usize::try_from(v).unwrap_or(usize::MAX);
        }
        if let Some(v) = read_bounded(
            &root,
            "max_burst_samples",
            1,
            u64::from(u32::MAX),
            "a number of samples >= 1",
        )? {
            out.max_burst_samples = usize::try_from(v).unwrap_or(usize::MAX);
        }
        if let Some(v) = read_bounded(
            &root,
            "ping_interval_ms",
            0,
            MAX_MS,
            "0 or milliseconds >= 100",
        )? {
            if v != 0 && v < MIN_PING_INTERVAL_MS {
                return Err(ConfigError::Invalid {
                    setting: root.path_of("ping_interval_ms"),
                    origin: root.node("ping_interval_ms").map_or(
                        zerodds_config::Origin::Default,
                        zerodds_config::read::first_origin,
                    ),
                    message: format!("got `{v}`, expected 0 or milliseconds >= 100"),
                });
            }
            out.ping_interval_ms = v;
        }
        if let Some(v) = read_bounded(&root, "shutdown_drain_ms", 1, MAX_MS, "milliseconds >= 1")? {
            out.shutdown_drain_ms = v;
        }
        if let Some(v) = root.boolean("catalog_on_connect")? {
            out.catalog_on_connect = v;
        }

        let origin = root.section("origin")?;
        origin.only(&["allowed", "required"])?;
        if let Some(v) = origin.strings("allowed")? {
            out.origin_allowed = v;
        }
        if let Some(v) = origin.boolean("required")? {
            out.origin_required = v;
        }
        if let Err(e) = out.origin_policy() {
            let key = match e {
                crate::origin::OriginPolicyError::InvalidEntry(_) => "allowed",
                crate::origin::OriginPolicyError::RequiredWithoutList => "required",
            };
            return Err(ConfigError::Invalid {
                setting: origin.path_of(key),
                origin: origin.node(key).map_or(
                    zerodds_config::Origin::Default,
                    zerodds_config::read::first_origin,
                ),
                message: e.to_string(),
            });
        }

        out.topics = read_topics(&root, &mut out.topic_acl)?;

        // Spec §1 L6: the tenants, each a participant with its topics.
        for (path, item) in root.items("tenants")? {
            let entry = Section::of(Some(item), path.clone())?;
            entry.only(&["name", "domain", "subjects", "topics"])?;
            let origin = zerodds_config::read::first_origin(item);
            let invalid = |key: &str, message: String| ConfigError::Invalid {
                setting: entry.path_of(key),
                origin: origin.clone(),
                message,
            };
            let name = entry
                .string("name")?
                .filter(|n| !n.is_empty())
                .ok_or_else(|| invalid("name", "a tenant needs a name".to_string()))?;
            // The name is the prefix of the ACL keys and the metric labels.
            if name.contains('/') {
                return Err(invalid(
                    "name",
                    format!("got `{name}`, expected a name without `/`"),
                ));
            }
            if out.tenants.iter().any(|t| t.name == name) {
                return Err(invalid("name", format!("a second tenant `{name}`")));
            }
            let domain = entry
                .parse::<i32>("domain", "a DDS domain id")?
                .unwrap_or(out.domain);
            let subjects = entry.strings("subjects")?.unwrap_or_default();
            if subjects.is_empty() || subjects.iter().any(String::is_empty) {
                return Err(invalid(
                    "subjects",
                    "a tenant needs the subjects bound to it".to_string(),
                ));
            }
            if let Some(subject) = subjects
                .iter()
                .find(|s| out.tenants.iter().any(|t| t.subjects.contains(s)))
            {
                return Err(invalid(
                    "subjects",
                    format!("`{subject}` is bound to another tenant"),
                ));
            }
            let mut acl = std::collections::HashMap::new();
            let topics = read_topics(&entry, &mut acl)?;
            // The ACL key and the metric label of a tenant's topic are
            // `<tenant>/<topic>`; a top-level topic of that name would
            // share the tenant topic's ACL and label.
            if let Some((i, topic)) = topics.iter().enumerate().find(|(_, t)| {
                let key = tenant_acl_topic(&name, &t.name);
                out.topics.iter().any(|o| o.name == key)
            }) {
                return Err(ConfigError::Invalid {
                    setting: alloc_path(&format!("{}[{i}]", entry.path_of("topics")), "name"),
                    origin: origin.clone(),
                    message: format!(
                        "the top-level topic `{}` has the ACL and the metric label of topic `{}` of tenant `{name}`",
                        tenant_acl_topic(&name, &topic.name),
                        topic.name
                    ),
                });
            }
            for (topic, rule) in acl {
                out.topic_acl.insert(tenant_acl_topic(&name, &topic), rule);
            }
            out.tenants.push(TenantConfig {
                name,
                domain,
                subjects,
                topics,
            });
        }
        Ok(out)
    }
}

/// The `topics` list of `section` (the top level or a tenant): one path and
/// one schema path per topic; the `acl` of each into `acl` by topic name.
fn read_topics(
    section: &Section<'_>,
    acl: &mut std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
) -> Result<Vec<TopicConfig>, ConfigError> {
    let mut topics: Vec<TopicConfig> = Vec::new();
    for (path, item) in section.items("topics")? {
        let t = read_topic(&path, item, acl)?;
        // §4.2: one path, one topic. A second entry at the path of an
        // earlier one would never be reached.
        if let Some(first) = topics.iter().find(|o| o.ws_path == t.ws_path) {
            return Err(ConfigError::Invalid {
                setting: alloc_path(&path, "ws_path"),
                origin: zerodds_config::read::first_origin(item),
                message: format!(
                    "`{}` is the path of topic `{}` already",
                    t.ws_path, first.name
                ),
            });
        }
        // §5.2: one schema path, one type. Two names of one slug
        // (`A.B`, `A_B`) would serve the first type for both.
        if !t.idl_file.is_empty() {
            let slug = super::schema::slug(&t.name);
            if let Some(first) = topics
                .iter()
                .find(|o| !o.idl_file.is_empty() && super::schema::slug(&o.name) == slug)
            {
                return Err(ConfigError::Invalid {
                    setting: alloc_path(&path, "name"),
                    origin: zerodds_config::read::first_origin(item),
                    message: format!(
                        "`/schema/{slug}` is the schema path of topic `{}` already",
                        first.name
                    ),
                });
            }
        }
        topics.push(t);
    }
    Ok(topics)
}

/// One `topics[]` entry. A bare name (`- Chat::Message`, or
/// `--topic Chat::Message`) is the entry `{name: Chat::Message}`; behind a
/// single `:` it names the path too (`--topic Chat::Message:/chat`, Spec
/// §2), as the other bridge daemons split their `--topic DDS:OTHER`.
fn read_topic(
    path: &str,
    item: &zerodds_config::Node,
    acl: &mut std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
) -> Result<TopicConfig, ConfigError> {
    let mut t = TopicConfig {
        direction: "bidir".to_string(),
        history_kind: "keep_last".to_string(),
        history_depth: 10,
        ..TopicConfig::default()
    };
    if let zerodds_config::Node::Scalar(text, _) = item {
        let (name, ws_path) = zerodds_config::read::split_topic_shorthand(text);
        t.name = name;
        t.ws_path = ws_path.unwrap_or_default();
        if t.ws_path.is_empty() && text.trim().len() != t.name.len() {
            return Err(ConfigError::Invalid {
                setting: path.to_string(),
                origin: zerodds_config::read::first_origin(item),
                message: format!("got `{}`, expected NAME or NAME:WS_PATH", text.trim()),
            });
        }
    } else {
        let entry = Section::of(Some(item), path)?;
        entry.only(&[
            "name",
            "type",
            "idl_file",
            "direction",
            "ws_path",
            "qos",
            "acl",
        ])?;
        t.name = entry.string("name")?.unwrap_or_default();
        t.type_name = entry.string("type")?.unwrap_or_default();
        t.idl_file = entry.string("idl_file")?.unwrap_or_default();
        if let Some(v) = entry.choice("direction", &["in", "out", "bidir"])? {
            t.direction = v;
        }
        t.ws_path = entry.string("ws_path")?.unwrap_or_default();
        let qos = entry.section("qos")?;
        qos.only(&[
            "reliability",
            "durability",
            "history",
            "deadline_ms",
            "liveliness_lease_ms",
            "partition",
        ])?;
        t.deadline_ms = read_bounded(&qos, "deadline_ms", 1, MAX_MS, "milliseconds >= 1")?;
        t.liveliness_lease_ms =
            read_bounded(&qos, "liveliness_lease_ms", 1, MAX_MS, "milliseconds >= 1")?;
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
                    message: alloc_msg(depth),
                });
            }
            t.history_depth = depth;
        }
        // Spec §3/§7.3: the ACL lives per topic entry,
        // `acl: { read: [...], write: [...] }`, flattened into the flat
        // `topic_acl` map the security layer consumes.
        let entry_acl = entry.section("acl")?;
        entry_acl.only(&["read", "write"])?;
        if entry.node("acl").is_some() {
            // One ACL per topic: a second entry of the topic with its own
            // `acl` would replace the first one's.
            if acl.contains_key(&t.name) {
                return Err(ConfigError::Invalid {
                    setting: alloc_path(path, "acl"),
                    origin: zerodds_config::read::first_origin(item),
                    message: format!(
                        "an earlier entry of topic `{}` has an `acl` already",
                        t.name
                    ),
                });
            }
            let read = entry_acl.strings("read")?.unwrap_or_default();
            let write = entry_acl.strings("write")?.unwrap_or_default();
            acl.insert(t.name.clone(), (read, write));
        }
    }
    if t.name.is_empty() {
        return Err(ConfigError::Invalid {
            setting: alloc_path(path, "name"),
            origin: zerodds_config::read::first_origin(item),
            message: "a topic entry needs a name".to_string(),
        });
    }
    if t.type_name.is_empty() {
        t.type_name = t.name.clone();
    }
    if t.ws_path.is_empty() {
        t.ws_path = default_ws_path(&t.name);
    }
    // §4.2: the path is the request target a client names. One that is no
    // absolute path is never asked for, and the meta endpoints keep theirs.
    let refused = if !is_http_path(&t.ws_path) {
        Some("expected a path that begins with `/`")
    } else if t.ws_path == CATALOG_PATH || t.ws_path == HEALTH_PATH {
        Some("the path of a meta endpoint (§4.2)")
    } else {
        None
    };
    if let Some(why) = refused {
        return Err(ConfigError::Invalid {
            setting: alloc_path(path, "ws_path"),
            origin: zerodds_config::read::first_origin(item),
            message: format!("got `{}`, {why}", t.ws_path),
        });
    }
    Ok(t)
}

/// Whether `path` can be the target of an HTTP request (RFC 9112 §3.2.1
/// origin-form): it begins with `/` and carries no whitespace, control
/// character, `?` or `#`.
fn is_http_path(path: &str) -> bool {
    path.starts_with('/')
        && path
            .chars()
            .all(|c| c.is_ascii_graphic() && c != '?' && c != '#')
}

/// A size in octets at `key`, at least `min`.
fn read_size(section: &Section<'_>, key: &str, min: usize) -> Result<Option<usize>, ConfigError> {
    let expect = format!("a size in octets >= {min}");
    let Some(v) = section.parse::<u64>(key, &expect)? else {
        return Ok(None);
    };
    match usize::try_from(v) {
        Ok(v) if v >= min => Ok(Some(v)),
        _ => Err(ConfigError::Invalid {
            setting: section.path_of(key),
            origin: section.node(key).map_or(
                zerodds_config::Origin::Default,
                zerodds_config::read::first_origin,
            ),
            message: format!("got `{v}`, expected {expect}"),
        }),
    }
}

/// The largest duration in milliseconds a DDS QoS takes here
/// (`zerodds_qos::Duration::from_millis` takes an `i32`).
const MAX_MS: u64 = 2_147_483_647;

/// An integer at `key` in `min..=max`.
fn read_bounded(
    section: &Section<'_>,
    key: &str,
    min: u64,
    max: u64,
    expect: &str,
) -> Result<Option<u64>, ConfigError> {
    let Some(v) = section.parse::<u64>(key, expect)? else {
        return Ok(None);
    };
    if (min..=max).contains(&v) {
        return Ok(Some(v));
    }
    Err(ConfigError::Invalid {
        setting: section.path_of(key),
        origin: section.node(key).map_or(
            zerodds_config::Origin::Default,
            zerodds_config::read::first_origin,
        ),
        message: format!("got `{v}`, expected {expect}"),
    })
}

fn alloc_path(path: &str, key: &str) -> String {
    format!("{path}.{key}")
}

fn alloc_msg(depth: i32) -> String {
    format!("got `{depth}`, expected an integer >= 1")
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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn wsbridge_5_1_slug_strips_double_colon() {
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
    fn wsbridge_5_1_slug_replaces_unsafe_chars() {
        assert_eq!(default_ws_path("My Topic!"), "/topics/my_topic_");
    }

    #[test]
    fn env_substitution_with_default() {
        // Test with a guaranteed-unset variable name
        // (UUID-style, so we do not rely on process state).
        let cfg = DaemonConfig::load_from_str(
            "listen: ${ZERODDS_PROBABLY_UNSET_VAR_e2afb0b9_test:-10.0.0.1:1}\n",
        )
        .unwrap();
        assert_eq!(cfg.listen, "10.0.0.1:1");
    }

    #[test]
    fn wsbridge_12_1_parse_minimal_config() {
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
        assert!(
            matches!(&err, ConfigError::Invalid { setting, .. } if setting == "domain"),
            "{err}"
        );
        assert!(err.to_string().contains("<string>:2"), "{err}");
    }

    #[test]
    fn a_misspelled_key_inside_a_known_section_is_refused_with_its_line() {
        for (yaml, setting) in [
            ("tls:\n  cert_fiel: /x\n", "tls.cert_fiel"),
            (
                "topics:\n  - name: T\n    qos:\n      reliabilty: reliable\n",
                "topics[0].qos.reliabilty",
            ),
            (
                "topics:\n  - name: T\n    qos:\n      history:\n        depth: many\n",
                "topics[0].qos.history.depth",
            ),
            ("tls:\n  enabled: yes\n", "tls.enabled"),
            ("auth:\n  mode: bearr\n", "auth.mode"),
        ] {
            let err = DaemonConfig::load_from_str(yaml).unwrap_err();
            assert!(
                matches!(&err, ConfigError::Invalid { setting: s, .. } if s == setting),
                "{yaml}: {err}"
            );
        }
    }

    #[test]
    fn ws_10_4_the_limits_are_read_and_default_to_16_mib() {
        let cfg = DaemonConfig::load_from_str("listen: x\n").unwrap();
        assert_eq!(cfg.max_frame_size, 16 * 1024 * 1024);
        assert_eq!(cfg.max_message_size, 16 * 1024 * 1024);
        let cfg = DaemonConfig::load_from_str(
            "limits:\n  max_frame_size: 65536\n  max_message_size: 1048576\n",
        )
        .unwrap();
        assert_eq!(cfg.max_frame_size, 65536);
        assert_eq!(cfg.max_message_size, 1_048_576);
        assert_eq!(
            cfg.limits(),
            crate::connection::Limits {
                max_frame_size: 65536,
                max_message_size: 1_048_576,
            }
        );
        // One of the two alone leaves the other at its default.
        let cfg = DaemonConfig::load_from_str("limits:\n  max_frame_size: 125\n").unwrap();
        assert_eq!(cfg.max_frame_size, 125);
        assert_eq!(cfg.max_message_size, 16 * 1024 * 1024);
    }

    #[test]
    fn ws_10_4_a_limit_that_cannot_be_served_is_refused_with_its_line() {
        for (yaml, setting) in [
            // No limit at all, and one below a control frame.
            ("limits:\n  max_frame_size: 0\n", "limits.max_frame_size"),
            ("limits:\n  max_frame_size: 124\n", "limits.max_frame_size"),
            (
                "limits:\n  max_message_size: 0\n",
                "limits.max_message_size",
            ),
            (
                "limits:\n  max_message_size: -1\n",
                "limits.max_message_size",
            ),
            (
                "limits:\n  max_frame_size: 16MiB\n",
                "limits.max_frame_size",
            ),
            ("limits:\n  max_frame: 1\n", "limits.max_frame"),
        ] {
            let err = DaemonConfig::load_from_str(yaml).unwrap_err();
            assert!(
                matches!(&err, ConfigError::Invalid { setting: s, .. } if s == setting),
                "{yaml}: {err}"
            );
            assert!(err.to_string().contains("<string>:2"), "{yaml}: {err}");
        }
    }

    #[test]
    fn ws_10_2_the_origin_keys_are_read() {
        let cfg = DaemonConfig::load_from_str("listen: x\n").unwrap();
        assert!(cfg.origin_allowed.is_empty());
        assert!(!cfg.origin_required);
        assert!(!cfg.origin_policy().unwrap().is_checking());
        let cfg = DaemonConfig::load_from_str(
            "origin:\n  allowed:\n    - https://app.example.org\n    - http://localhost:3000\n  required: true\n",
        )
        .unwrap();
        assert_eq!(
            cfg.origin_allowed,
            ["https://app.example.org", "http://localhost:3000"]
        );
        assert!(cfg.origin_required);
        let policy = cfg.origin_policy().unwrap();
        assert_eq!(policy.check(Some("http://localhost:3000")), Ok(()));
        assert!(policy.check(Some("https://evil.example")).is_err());
        assert!(policy.check(None).is_err());
        // The inline list of the YAML subset.
        let cfg =
            DaemonConfig::load_from_str("origin:\n  allowed: [\"https://a.example\", \"null\"]\n")
                .unwrap();
        assert_eq!(cfg.origin_allowed, ["https://a.example", "null"]);
    }

    #[test]
    fn ws_10_2_an_origin_setting_that_cannot_be_served_is_refused_with_its_line() {
        for (yaml, setting) in [
            // An origin has no path; a host alone is no origin.
            (
                "origin:\n  allowed: [\"https://app.example.org/\"]\n",
                "origin.allowed",
            ),
            ("origin:\n  allowed: app.example.org\n", "origin.allowed"),
            // `*` is an entry of its own and no pattern inside a host.
            (
                "origin:\n  allowed: [\"https://*.example.org\"]\n",
                "origin.allowed",
            ),
            // Required, and nothing expected.
            ("origin:\n  required: true\n", "origin.required"),
            ("origin:\n  required: maybe\n", "origin.required"),
            ("origin:\n  alowed: [\"*\"]\n", "origin.alowed"),
        ] {
            let err = DaemonConfig::load_from_str(yaml).unwrap_err();
            assert!(
                matches!(&err, ConfigError::Invalid { setting: s, .. } if s == setting),
                "{yaml}: {err}"
            );
            assert!(err.to_string().contains("<string>:2"), "{yaml}: {err}");
        }
    }

    /// Spec §2: `--topic NAME[:WS_PATH]` is a bare entry of `topics`; the
    /// single `:` (not the `::` of a scoped name) separates the path.
    #[test]
    fn wsbridge_2_a_bare_topic_entry_names_its_path_behind_a_colon() {
        let cfg = DaemonConfig::load_from_str(
            "topics:\n  - \"Chat::Message:/chat\"\n  - Plain\n  - \"Sensor::Reading:/s/r\"\n",
        )
        .unwrap();
        let got: Vec<_> = cfg
            .topics
            .iter()
            .map(|t| (t.name.as_str(), t.ws_path.as_str(), t.path()))
            .collect();
        assert_eq!(
            got,
            [
                ("Chat::Message", "/chat", "/chat".to_string()),
                ("Plain", "/topics/plain", "/topics/plain".to_string()),
                ("Sensor::Reading", "/s/r", "/s/r".to_string()),
            ]
        );
        for (yaml, setting) in [
            // A path is absolute; an empty one behind the colon is none.
            ("topics:\n  - \"Chat::Message:chat\"\n", "topics[0].ws_path"),
            ("topics:\n  - \"Chat::Message:\"\n", "topics[0]"),
            ("topics:\n  - \":/chat\"\n", "topics[0].name"),
        ] {
            let err = DaemonConfig::load_from_str(yaml).unwrap_err();
            assert!(
                matches!(&err, ConfigError::Invalid { setting: s, .. } if s == setting),
                "{yaml}: {err}"
            );
        }
    }

    /// Spec §4.2: a topic has one path. A `ws_path` that is no absolute
    /// path, the path of a meta endpoint, or the path of an earlier entry
    /// is refused with its line.
    #[test]
    fn wsbridge_4_2_a_ws_path_that_cannot_be_served_is_refused() {
        for ws_path in [
            "topics/chat",
            "/topics/__catalog__",
            "/healthz",
            "/a b",
            "/a?x=1",
        ] {
            let yaml = format!("topics:\n  - name: T\n    ws_path: \"{ws_path}\"\n");
            let err = DaemonConfig::load_from_str(&yaml).unwrap_err();
            assert!(
                matches!(&err, ConfigError::Invalid { setting, .. } if setting == "topics[0].ws_path"),
                "{ws_path}: {err}"
            );
            assert!(err.to_string().contains("<string>:2"), "{ws_path}: {err}");
        }
        // Two entries at one path: by `ws_path`, and by the default path.
        for yaml in [
            "topics:\n  - name: A\n    ws_path: /x\n  - name: B\n    ws_path: /x\n",
            "topics:\n  - name: Chat::Message\n  - name: B\n    ws_path: /topics/chat/message\n",
        ] {
            let err = DaemonConfig::load_from_str(yaml).unwrap_err();
            assert!(
                matches!(&err, ConfigError::Invalid { setting, .. } if setting == "topics[1].ws_path"),
                "{yaml}: {err}"
            );
        }
        // A topic without `ws_path` is at its default path, and `path()`
        // names it for an entry built in code as well.
        let built = TopicConfig {
            name: "Chat::Message".to_string(),
            ..TopicConfig::default()
        };
        assert_eq!(built.path(), "/topics/chat/message");
    }

    /// Spec §3: `qos.history.kind` and `metrics.path`.
    #[test]
    fn wsbridge_3_the_history_kind_and_the_metrics_path_are_read() {
        let cfg = DaemonConfig::load_from_str("topics:\n  - name: T\n").unwrap();
        assert_eq!(cfg.topics[0].history_kind, "keep_last");
        assert!(!cfg.topics[0].keep_all());
        assert_eq!(cfg.metrics_path, "/metrics");
        let cfg = DaemonConfig::load_from_str(
            "metrics:\n  path: /internal/metrics\ntopics:\n  - name: T\n    qos:\n      history: { kind: keep_all, depth: 3 }\n",
        )
        .unwrap();
        assert_eq!(cfg.metrics_path, "/internal/metrics");
        assert!(cfg.topics[0].keep_all());
        assert_eq!(cfg.topics[0].history_depth, 3);
        for (yaml, setting) in [
            ("metrics:\n  path: metrics\n", "metrics.path"),
            ("metrics:\n  path: /healthz\n", "metrics.path"),
            (
                "topics:\n  - name: T\n    qos:\n      history:\n        kind: keep_some\n",
                "topics[0].qos.history.kind",
            ),
        ] {
            let err = DaemonConfig::load_from_str(yaml).unwrap_err();
            assert!(
                matches!(&err, ConfigError::Invalid { setting: s, .. } if s == setting),
                "{yaml}: {err}"
            );
        }
    }

    /// Spec §5.2 with §3: `catalog_on_connect`, false by default; a value
    /// that is no boolean prevents the start.
    #[test]
    fn wsbridge_5_2_catalog_on_connect_is_read() {
        assert!(!DaemonConfig::load_from_str("").unwrap().catalog_on_connect);
        assert!(
            DaemonConfig::load_from_str("catalog_on_connect: true\n")
                .unwrap()
                .catalog_on_connect
        );
        assert!(DaemonConfig::load_from_str("catalog_on_connect: often\n").is_err());
    }

    /// Spec §5.2: two topics with `idl_file` whose names have one slug
    /// would share a schema path; the second is refused, naming the first.
    /// Without `idl_file` a topic has no schema path and is not compared.
    #[test]
    fn wsbridge_5_2_two_topics_of_one_schema_slug_are_refused() {
        let e = DaemonConfig::load_from_str(
            "topics:\n  - name: A.B\n    ws_path: /a1\n    idl_file: /x.idl\n  - name: A_B\n    ws_path: /a2\n    idl_file: /y.idl\n",
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("/schema/a_b") && e.contains("`A.B`"), "{e}");
        assert!(
            DaemonConfig::load_from_str(
                "topics:\n  - name: A.B\n    ws_path: /a1\n    idl_file: /x.idl\n  - name: A_B\n    ws_path: /a2\n",
            )
            .is_ok()
        );
    }

    /// Spec §9.2 with §3: `shutdown_drain_ms`, 30 s by default, at least 1.
    #[test]
    fn wsbridge_9_2_shutdown_drain_ms_is_read() {
        let cfg = DaemonConfig::load_from_str("topics:\n  - name: T\n").unwrap();
        assert_eq!(cfg.shutdown_drain_ms, 30_000);
        let cfg = DaemonConfig::load_from_str("shutdown_drain_ms: 1500\n").unwrap();
        assert_eq!(cfg.shutdown_drain_ms, 1500);
        for bad in [
            "shutdown_drain_ms: 0\n",
            "shutdown_drain_ms: 2147483648\n",
            "shutdown_drain_ms: soon\n",
        ] {
            let err = DaemonConfig::load_from_str(bad).unwrap_err();
            assert!(
                matches!(&err, ConfigError::Invalid { setting, .. } if setting == "shutdown_drain_ms"),
                "{bad}: {err}"
            );
        }
    }

    /// Spec §6 with §3: the keys of the QoS map, their defaults and their
    /// ranges.
    #[test]
    fn wsbridge_6_the_qos_keys_are_read() {
        let cfg = DaemonConfig::load_from_str("topics:\n  - name: T\n").unwrap();
        assert_eq!(cfg.max_send_queue_per_connection, 1024);
        assert_eq!(cfg.max_burst_samples, 1000);
        assert_eq!(cfg.ping_interval_ms, 30_000);
        assert_eq!(cfg.topics[0].deadline_ms, None);
        assert_eq!(cfg.topics[0].liveliness_lease_ms, None);
        assert!(cfg.topics[0].partition.is_empty());
        let cfg = DaemonConfig::load_from_str(
            "max_send_queue_per_connection: 8\nmax_burst_samples: 3\nping_interval_ms: 0\n\
             topics:\n  - name: T\n    qos:\n      deadline_ms: 250\n      liveliness_lease_ms: 2000\n      partition: [\"plant-a\", \"line*\"]\n",
        )
        .unwrap();
        assert_eq!(cfg.max_send_queue_per_connection, 8);
        assert_eq!(cfg.max_burst_samples, 3);
        assert_eq!(cfg.ping_interval_ms, 0);
        let t = &cfg.topics[0];
        assert_eq!(t.deadline_ms, Some(250));
        assert_eq!(t.liveliness_lease_ms, Some(2000));
        assert_eq!(t.partition, ["plant-a", "line*"]);
        assert_eq!(
            cfg.queue_limits(),
            crate::daemon::qos_translation::QueueLimits {
                max_send_queue: 8,
                max_burst: 3,
            }
        );
        for (yaml, setting) in [
            (
                "max_send_queue_per_connection: 0\n",
                "max_send_queue_per_connection",
            ),
            ("max_burst_samples: 0\n", "max_burst_samples"),
            ("ping_interval_ms: 50\n", "ping_interval_ms"),
            (
                "topics:\n  - name: T\n    qos:\n      deadline_ms: 0\n",
                "topics[0].qos.deadline_ms",
            ),
            (
                "topics:\n  - name: T\n    qos:\n      liveliness_lease_ms: 4294967296\n",
                "topics[0].qos.liveliness_lease_ms",
            ),
            (
                "topics:\n  - name: T\n    qos:\n      lifespan_ms: 5\n",
                "topics[0].qos.lifespan_ms",
            ),
        ] {
            let err = DaemonConfig::load_from_str(yaml).unwrap_err();
            assert!(err.to_string().contains(setting), "{yaml}: {err}");
        }
    }

    /// Spec §3: the example of the specification itself loads, with the
    /// values it names.
    #[test]
    fn wsbridge_3_the_example_of_the_spec_loads() {
        let spec = include_str!("../../../../docs/specs/zerodds-ws-bridge-1.0.md");
        let section = &spec[spec.find("## §3 Config file format").expect("§3")..];
        let start = section.find("```yaml\n").expect("the example") + "```yaml\n".len();
        let yaml = &section[start..start + section[start..].find("```").expect("its end")];
        // `${TOKEN}` of the example is taken from the environment; the
        // default form keeps the test independent of it.
        let yaml = yaml.replace("${TOKEN}", "${ZERODDS_WS_SPEC_EXAMPLE_TOKEN:-secret}");
        let cfg = DaemonConfig::load_from_str(&yaml).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(cfg.listen, "127.0.0.1:8080");
        assert_eq!(cfg.domain, 0);
        assert_eq!(cfg.log_level, "info");
        assert!(!cfg.tls_enabled);
        assert_eq!(cfg.tls_cert_file, "/etc/zerodds/cert.pem");
        assert_eq!(cfg.tls_key_file, "/etc/zerodds/key.pem");
        assert_eq!(cfg.tls_client_ca_file, "/etc/zerodds/clients-ca.pem");
        assert_eq!(cfg.auth_mode, "none");
        assert_eq!(cfg.auth_bearer_token.as_deref(), Some("secret"));
        assert_eq!(cfg.auth_bearer_subject.as_deref(), Some("service-account"));
        assert_eq!(cfg.auth_jwt.public_key, "/etc/zerodds/jwt-pub.pem");
        assert_eq!(
            cfg.auth_jwt.issuer.as_deref(),
            Some("https://idp.example.org")
        );
        assert_eq!(cfg.auth_jwt.audience.as_deref(), Some("zerodds-bridge"));
        assert_eq!(
            cfg.auth_jwt.required_claim.as_deref(),
            Some("scope=dds.read")
        );
        assert_eq!(cfg.topics.len(), 2);
        let chat = &cfg.topics[0];
        assert_eq!(chat.name, "Chat::Message");
        assert_eq!(chat.idl_file, "/etc/zerodds/chat.idl");
        assert_eq!(chat.direction, "bidir");
        assert_eq!(chat.reliability, "reliable");
        assert_eq!(chat.durability, "volatile");
        assert_eq!(chat.history_kind, "keep_last");
        assert_eq!(chat.history_depth, 10);
        assert_eq!(chat.ws_path, "/topics/chat");
        assert_eq!(
            cfg.topic_acl.get("Chat::Message"),
            Some(&(
                vec!["*".to_string()],
                vec!["alice".to_string(), "bob".to_string()]
            ))
        );
        let sensor = &cfg.topics[1];
        assert_eq!(
            (sensor.direction.as_str(), sensor.ws_path.as_str()),
            ("out", "/topics/sensor")
        );
        assert!(cfg.metrics_enabled);
        assert_eq!(cfg.metrics_addr, "127.0.0.1:9091");
        assert_eq!(cfg.metrics_path, "/metrics");
        assert_eq!(cfg.max_frame_size, 16_777_216);
        assert_eq!(cfg.max_message_size, 16_777_216);
        assert_eq!(cfg.origin_allowed, ["https://app.example.org"]);
        assert!(!cfg.origin_required);
        assert_eq!(cfg.max_send_queue_per_connection, 1024);
        assert_eq!(cfg.max_burst_samples, 1000);
        assert_eq!(cfg.ping_interval_ms, 30_000);
        assert_eq!(cfg.shutdown_drain_ms, 30_000);
        assert_eq!(chat.deadline_ms, Some(1000));
        assert_eq!(chat.liveliness_lease_ms, Some(10_000));
        assert_eq!(chat.partition, ["chat"]);
        // §1 L6: the tenant of the example.
        assert_eq!(cfg.tenants.len(), 1);
        let tenant = &cfg.tenants[0];
        assert_eq!(tenant.name, "plant-a");
        assert_eq!(tenant.domain, 7);
        assert_eq!(tenant.subjects, ["alice", "CN=gw-7,O=Example"]);
        assert_eq!(tenant.topics.len(), 1);
        assert_eq!(tenant.topics[0].name, "Plant::Status");
        assert_eq!(tenant.topics[0].direction, "out");
        assert_eq!(
            cfg.topic_acl
                .get(&tenant_acl_topic("plant-a", "Plant::Status")),
            Some(&(vec!["alice".to_string()], Vec::new()))
        );
        // Every key of the daemon is in the example by its full dotted
        // path, and the example names no key besides (the keys of a topic
        // entry are the topic's own, below `topics`).
        let named = dotted_keys(&yaml);
        for key in KEYS {
            assert!(
                named.iter().any(|(path, _)| path == key),
                "the example of §3 does not name `{key}`"
            );
        }
        for (path, leaf) in &named {
            // The keys of a topic entry and of a tenant entry are their own.
            if path.starts_with("topics.") || path.starts_with("tenants.") {
                continue;
            }
            if *leaf {
                assert!(is_key(path), "the example of §3 names `{path}`, no key");
            } else {
                assert!(
                    KEYS.iter()
                        .any(|k| *k == path || k.starts_with(&format!("{path}."))),
                    "the example of §3 names the section `{path}`, which has no key"
                );
            }
        }
    }

    /// The mapping keys of a YAML block by their dotted path from the
    /// indentation, and whether each carries a value on its line (a leaf)
    /// or opens a block. A list item (`- `) adds no key of its own.
    fn dotted_keys(yaml: &str) -> Vec<(String, bool)> {
        let mut stack: Vec<(usize, String)> = Vec::new();
        let mut out = Vec::new();
        for line in yaml.lines() {
            let text = line.split(" #").next().unwrap_or("").trim_end();
            let body = text.trim_start();
            if body.is_empty() || body.starts_with('#') {
                continue;
            }
            let indent = text.len() - body.len();
            while stack.last().is_some_and(|(i, _)| *i >= indent) {
                stack.pop();
            }
            let Some((key, value)) = body.split_once(':') else {
                continue;
            };
            if key.starts_with("- ") || key.contains(' ') {
                continue;
            }
            let mut path: Vec<&str> = stack.iter().map(|(_, k)| k.as_str()).collect();
            path.push(key);
            out.push((path.join("."), !value.trim().is_empty()));
            stack.push((indent, key.to_string()));
        }
        out
    }

    #[test]
    fn a_bare_topic_name_is_a_topic_entry() {
        let cfg = DaemonConfig::load_from_str("topics:\n  - Chat::Message\n").unwrap();
        assert_eq!(cfg.topics[0].name, "Chat::Message");
        assert_eq!(cfg.topics[0].direction, "bidir");
        assert_eq!(cfg.topics[0].ws_path, "/topics/chat/message");
    }

    /// Spec §1 L6, §3: the tenants, their topics with an ACL of their own
    /// under `<tenant>/<topic>`, and what their entries refuse.
    #[test]
    fn wsbridge_1_l6_the_tenants() {
        let yaml = "domain: 3\ntopics:\n  - name: Chat\n    acl:\n      write: [ops]\ntenants:\n  - name: plant-a\n    domain: 9\n    subjects: [alice, \"mtls:ab12\"]\n    topics:\n      - name: Chat\n        acl:\n          read: [alice]\n          write: [alice]\n  - name: plant-b\n    subjects: [bob]\n";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert_eq!(cfg.tenants.len(), 2);
        assert_eq!(cfg.tenants[0].name, "plant-a");
        assert_eq!(cfg.tenants[0].domain, 9);
        assert_eq!(cfg.tenants[0].topics[0].name, "Chat");
        assert_eq!(cfg.tenants[0].topics[0].ws_path, "/topics/chat");
        assert_eq!(
            cfg.tenants[1].domain, 3,
            "the domain of the daemon by default"
        );
        assert!(cfg.tenants[1].topics.is_empty());
        // The top-level topic and the tenant's topic of one name keep
        // their own ACL.
        assert_eq!(
            cfg.topic_acl.get("Chat"),
            Some(&(Vec::new(), vec!["ops".to_string()]))
        );
        assert_eq!(
            cfg.topic_acl.get(&tenant_acl_topic("plant-a", "Chat")),
            Some(&(vec!["alice".to_string()], vec!["alice".to_string()]))
        );
        // A subject binds by its name or by its mTLS fingerprint.
        let alice = zerodds_bridge_security::AuthSubject::new("alice");
        let mut cert = zerodds_bridge_security::AuthSubject::new("CN=x");
        cert.fingerprint = Some("mtls:ab12".to_string());
        let carol = zerodds_bridge_security::AuthSubject::new("carol");
        assert!(cfg.tenants[0].binds(&alice));
        assert!(cfg.tenants[0].binds(&cert));
        assert!(!cfg.tenants[0].binds(&carol));
        assert!(!cfg.tenants[1].binds(&alice));
        for (yaml, setting) in [
            ("tenants:\n  - subjects: [x]\n", "tenants[0].name"),
            (
                "tenants:\n  - name: a/b\n    subjects: [x]\n",
                "tenants[0].name",
            ),
            ("tenants:\n  - name: a\n", "tenants[0].subjects"),
            (
                "tenants:\n  - name: a\n    subjects: [x]\n  - name: a\n    subjects: [y]\n",
                "tenants[1].name",
            ),
            (
                "tenants:\n  - name: a\n    subjects: [x]\n  - name: b\n    subjects: [x]\n",
                "tenants[1].subjects",
            ),
            (
                "tenants:\n  - name: a\n    subjects: [x]\n    listen: z\n",
                "tenants[0].listen",
            ),
            (
                "tenants:\n  - name: a\n    subjects: [x]\n    topics:\n      - name: A\n        ws_path: /p\n      - name: B\n        ws_path: /p\n",
                "tenants[0].topics[1].ws_path",
            ),
            // A top-level topic named `<tenant>/<topic>` of a tenant's
            // topic would have its ACL and its metric label, with or
            // without an `acl` of its own.
            (
                "topics:\n  - name: plant-a/Chat\n    acl:\n      read: [ops]\ntenants:\n  - name: plant-a\n    subjects: [x]\n    topics:\n      - name: Chat\n        acl:\n          read: [alice]\n",
                "tenants[0].topics[0].name",
            ),
            (
                "topics:\n  - plant-a/Chat\ntenants:\n  - name: plant-a\n    subjects: [x]\n    topics:\n      - name: Other\n      - name: Chat\n        acl:\n          read: [alice]\n",
                "tenants[0].topics[1].name",
            ),
            // Two entries of one topic with an `acl` each.
            (
                "topics:\n  - name: A\n    ws_path: /a\n    acl:\n      read: [x]\n  - name: A\n    ws_path: /b\n    acl:\n      read: [y]\n",
                "topics[1].acl",
            ),
        ] {
            let err = DaemonConfig::load_from_str(yaml).unwrap_err();
            assert!(
                matches!(&err, ConfigError::Invalid { setting: s, .. } if s == setting),
                "{yaml}: {err}"
            );
        }
        // A `/` in a top-level topic's name is no collision where no
        // tenant's topic has the key.
        let cfg = DaemonConfig::load_from_str(
            "topics:\n  - name: plant-b/Chat\n    acl:\n      read: [ops]\n  - name: plant-a/Other\ntenants:\n  - name: plant-a\n    subjects: [x]\n    topics:\n      - name: Chat\n        acl:\n          read: [alice]\n",
        )
        .expect("no collision");
        assert_eq!(
            cfg.topic_acl.get("plant-b/Chat"),
            Some(&(vec!["ops".to_string()], Vec::new()))
        );
        assert_eq!(
            cfg.topic_acl.get(&tenant_acl_topic("plant-a", "Chat")),
            Some(&(vec!["alice".to_string()], Vec::new()))
        );
    }
}
