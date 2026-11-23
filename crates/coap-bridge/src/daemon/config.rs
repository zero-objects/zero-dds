// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Config for `zerodds-coap-bridged`. Spec §3.
//!
//! Read by the shared layered loader (`zerodds-config`): base file,
//! `coap-bridged.d/` drop-ins, includes, `${VAR}`, and the precedence
//! defaults < environment < files < `--set`. The DDS runtime's keys may sit
//! in the same file. [`DaemonConfig::from_config`] reads this daemon's keys
//! and refuses an unknown key inside a known section, and a value of the
//! wrong shape or type, with its file and line.

use std::path::Path;
use std::string::{String, ToString};
use std::vec::Vec;

pub use zerodds_config::ConfigError;
use zerodds_config::read::{Section, first_origin};

/// The base name of the daemon's file: `coap-bridged.yaml`, `coap-bridged.d/`.
pub const CONFIG_STEM: &str = "coap-bridged";

/// Every key this daemon reads; list-valued keys stop at the list.
pub const KEYS: &[&str] = &[
    "domain",
    "log_level",
    "coap.bind",
    "coap.bind_dtls",
    "coap.max_message_size",
    "coap.ack_timeout_ms",
    "coap.max_retransmit",
    "coap.observe_max_age_secs",
    "coap.block_size",
    "coap.max_body_size",
    "coap.keep_alive_secs",
    "coap.multicast.enabled",
    "coap.multicast.groups",
    "coap.multicast.interface_v4",
    "coap.multicast.interface_v6",
    "coap.multicast.leisure_ms",
    "coap.multicast.resources",
    "coap.multicast.unauthenticated_resources",
    "coap.dtls.enabled",
    "coap.dtls.cert_file",
    "coap.dtls.key_file",
    "coap.dtls.ca_file",
    "coap.dtls.cipher_suites",
    "coap.dtls.psk",
    "coap.dtls.rpk_key_file",
    "coap.dtls.rpk_trusted",
    "auth.mode",
    "auth.bearer_token",
    "auth.bearer_subject",
    "auth.jwt.public_key",
    "auth.jwt.issuer",
    "auth.jwt.audience",
    "auth.jwt.required_claim",
    "acl.default_deny",
    "acl.rules",
    "metrics.enabled",
    "metrics.listen",
    "metrics.path",
    "content_format.cdr2_le_id",
    "content_format.cdr2_be_id",
    "content_format.cdr1_le_id",
    "content_format.json_id",
    "content_format.cbor_id",
    "oscore.enabled",
    "oscore.master_secret_hex",
    "oscore.master_salt_hex",
    "oscore.id_context_hex",
    "oscore.sender_id_hex",
    "oscore.replay_window",
    "proxy.forward",
    "proxy.schemes",
    "proxy.names",
    "proxy.timeout_ms",
    "proxy.reverse",
    "proxy.cache_entries",
    "proxy.require_dtls_for_coaps",
    "proxy.max_hops",
    "proxy.multicast_wait_ms",
    "proxy.dtls.ca_file",
    "proxy.dtls.cert_file",
    "proxy.dtls.key_file",
    "proxy.dtls.psk_identity",
    "proxy.dtls.psk_secret_hex",
    "proxy.dtls.rpk_key_file",
    "proxy.dtls.rpk_trusted",
    "topics",
    "tenants",
];

/// Whether `path` is one of [`KEYS`].
#[must_use]
pub fn is_key(path: &str) -> bool {
    KEYS.contains(&path)
}

/// Every entry of `names` must be an RFC 6920 name (`ni:`, `nih:` or the
/// hex of the binary format) of a raw public key (RFC 7252 §9.1.3.2.1);
/// without the `dtls` feature only its form is checked by the start, which
/// refuses DTLS anyway.
fn check_key_names(
    names: &[String],
    setting: String,
    node: Option<&zerodds_config::Node>,
) -> Result<(), ConfigError> {
    #[cfg(feature = "dtls")]
    for name in names {
        if let Err(e) = crate::dtls_transport::KeyName::parse(name) {
            return Err(invalid(
                setting,
                node.map_or(zerodds_config::Origin::Default, first_origin),
                format!("`{name}`: {e}"),
            ));
        }
    }
    #[cfg(not(feature = "dtls"))]
    let _ = (names, setting, node);
    Ok(())
}

fn invalid(setting: String, origin: zerodds_config::Origin, message: String) -> ConfigError {
    ConfigError::Invalid {
        setting,
        origin,
        message,
    }
}

/// Daemon config.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// `domain:`.
    pub domain: i32,
    /// `log_level:`.
    pub log_level: String,
    /// `coap.bind:`.
    pub bind: String,
    /// `coap.bind_dtls:` — the `coaps://` bind address (Spec §7.1).
    pub bind_dtls: Option<String>,
    /// `coap.max_message_size:`.
    pub max_message_size: usize,
    /// `coap.ack_timeout_ms:`.
    pub ack_timeout_ms: u32,
    /// `coap.max_retransmit:`.
    pub max_retransmit: u8,
    /// `coap.observe_max_age_secs:` — the longest an observer goes without
    /// acknowledging a notification (spec §4.3; RFC 7641 §4.5 sets 24 h as
    /// the bound): then a notification is Confirmable, and an observer that
    /// does not acknowledge it is removed.
    pub observe_max_age_secs: u32,
    /// `coap.block_size:` — the block size of Block2 for a representation
    /// larger than it, and of the daemon's Block1 answers (RFC 7959; 16 to
    /// 1024, a power of two).
    pub block_size: u16,
    /// `coap.max_body_size:` — the largest body a Block1 transfer may
    /// assemble; beyond it `4.13` with Size1 (RFC 7959 §2.9.3).
    pub max_body_size: usize,
    /// `coap.keep_alive_secs:` — the period of the daemon's CoAP ping to
    /// each observer (spec §6, Liveliness); 0 sends none.
    pub keep_alive_secs: u32,
    /// `content_format:` — the Content-Format number of each form of §4.5.
    pub content_formats: super::representation::ContentFormats,
    /// `topics:`.
    pub topics: Vec<TopicConfig>,
    /// `coap.dtls.enabled` — serve `coaps://` (Spec §7.1).
    pub dtls_enabled: bool,
    /// `coap.dtls.cert_file` / `key_file` / `ca_file` / `psk` (Spec §7.1).
    pub dtls: crate::dtls::DtlsCredentials,
    /// Spec §7.2 — auth mode `none|bearer|jwt|mtls`.
    pub auth_mode: String,
    /// `auth.bearer_token:`.
    pub auth_bearer_token: Option<String>,
    /// `auth.bearer_subject:`.
    pub auth_bearer_subject: Option<String>,
    /// `auth.jwt.*` — for `auth.mode: jwt`, the token in option 65000.
    pub auth_jwt: zerodds_bridge_security::JwtSettings,
    /// Spec §7.3 — ACL per CoAP resource path, built from the
    /// subject-keyed `acl.rules` of Spec §3.
    pub topic_acl: std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
    /// `acl.default_deny` — Spec §7.3. `false` opens every resource no rule
    /// names. `true` denies them — as the key's default only once there is
    /// a rule or an auth mode: without either, an ACL nobody configured
    /// stays open (`zerodds_bridge_security::build_ctx`), so a daemon without
    /// an `acl:` section keeps working.
    pub acl_default_deny: bool,
    /// Whether `acl.default_deny` was given. An explicit `true` denies every
    /// unnamed resource even with no rule and `auth: none`; it used to be
    /// overridden by the open fallback without a word.
    pub acl_default_deny_set: bool,
    /// `metrics.enabled` — toggles the Prometheus endpoint (§8.2).
    pub metrics_enabled: bool,
    /// `metrics.listen` — bind address for catalog/healthz/metrics.
    pub metrics_addr: String,
    /// `metrics.path` — the path of the Prometheus page, `/metrics`.
    pub metrics_path: String,
    /// Spec §7.2 — OSCORE (RFC 8613) settings.
    pub oscore: OscoreSettings,
    /// `coap.multicast` — requests to IP multicast groups (RFC 7252 §8).
    pub multicast: MulticastSettings,
    /// `proxy` — the daemon as a CoAP-to-CoAP proxy (RFC 7252 §5.7).
    pub proxy: ProxySettings,
    /// `tenants` (spec §1 L6): DomainParticipants beside the one of
    /// `domain`, each with its topics; a DTLS session of one of a tenant's
    /// subjects is bound to it.
    pub tenants: Vec<TenantConfig>,
}

/// A tenant (spec §1 L6, §3 `tenants`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TenantConfig {
    /// `name:` — unique, in the log and the metrics.
    pub name: String,
    /// `domain:` — the DDS domain of its participant.
    pub domain: i32,
    /// `subjects:` — the DTLS subjects bound to it (`psk:<identity>`, the
    /// certificate's subject DN, `rpk:<ni URI>`, spec §7.1).
    pub subjects: Vec<String>,
    /// `topics:` — as the top-level `topics`.
    pub topics: Vec<TopicConfig>,
}

/// `proxy` (RFC 7252 §5.7): the daemon as a forward proxy for requests
/// with Proxy-Uri or Proxy-Scheme and as a reverse proxy for configured
/// resource paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxySettings {
    /// `proxy.forward`: act as a forward proxy (§5.7.2); otherwise a proxy
    /// request is `5.05 Proxying Not Supported`.
    pub forward: bool,
    /// `proxy.schemes`: the schemes a forward request may name; `coap` and
    /// `coaps` by default.
    pub schemes: Vec<String>,
    /// `proxy.names`: host names that name this endpoint itself (§5.7.2:
    /// such a request is a local one).
    pub names: Vec<String>,
    /// `proxy.timeout_ms`: after it a request to the destination has timed
    /// out, `5.04 Gateway Timeout` (§5.7.1).
    pub timeout_ms: u64,
    /// `proxy.reverse`: `{path, origin}` — the resources under `path` are
    /// those under the `origin` URI (§5.7.3).
    pub reverse: Vec<(String, String)>,
    /// `proxy.cache_entries`: the most responses the cache keeps.
    pub cache_entries: usize,
    /// `proxy.require_dtls_for_coaps`: a request for a `coaps` destination
    /// must come over DTLS (§5.7, §11.2); true by default.
    pub require_dtls_for_coaps: bool,
    /// `proxy.max_hops`: the Hop-Limit a forwarded request starts with
    /// when it has none (RFC 8768 §3); 16 by default.
    pub max_hops: u8,
    /// `proxy.multicast_wait_ms`: how long a request to a multicast
    /// destination collects responses (§8.2.2); 5000 by default.
    pub multicast_wait_ms: u64,
    /// `proxy.dtls.ca_file`: the trust anchors a `coaps` destination's
    /// certificate is checked against.
    pub dtls_ca_file: Option<String>,
    /// `proxy.dtls.cert_file`, `key_file`: the certificate the proxy shows a
    /// `coaps` destination.
    pub dtls_cert_file: Option<String>,
    /// See [`Self::dtls_cert_file`].
    pub dtls_key_file: Option<String>,
    /// `proxy.dtls.psk_identity`, `psk_secret_hex`: a pre-shared key for `coaps`
    /// destinations instead of certificates.
    pub dtls_psk: Option<(String, Vec<u8>)>,
    /// `proxy.dtls.rpk_key_file`: the private key whose public key the proxy
    /// shows a `coaps` destination as its raw public key (RFC 7250).
    pub dtls_rpk_key_file: Option<String>,
    /// `proxy.dtls.rpk_trusted`: the RFC 6920 names of the destinations'
    /// raw public keys the proxy accepts (RFC 7252 §9.1.3.2.1).
    pub dtls_rpk_trusted: Vec<String>,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            forward: false,
            schemes: vec!["coap".to_string(), "coaps".to_string()],
            names: Vec::new(),
            timeout_ms: 10_000,
            reverse: Vec::new(),
            cache_entries: 256,
            require_dtls_for_coaps: true,
            max_hops: 16,
            multicast_wait_ms: 5_000,
            dtls_ca_file: None,
            dtls_cert_file: None,
            dtls_key_file: None,
            dtls_psk: None,
            dtls_rpk_key_file: None,
            dtls_rpk_trusted: Vec::new(),
        }
    }
}

/// `coap.multicast` (RFC 7252 §8, §11.3): the groups the daemon joins and
/// the resources that answer a request to one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MulticastSettings {
    /// `coap.multicast.enabled`.
    pub enabled: bool,
    /// `coap.multicast.groups`: the all-CoAP-nodes groups of §12.8 by
    /// default where the bind address has their family.
    pub groups: Vec<std::net::IpAddr>,
    /// `coap.multicast.interface_v4`: the interface address an IPv4 group
    /// is joined on (`0.0.0.0` lets the system choose).
    pub interface_v4: std::net::Ipv4Addr,
    /// `coap.multicast.interface_v6`: the interface index an IPv6 group is
    /// joined on (0 lets the system choose).
    pub interface_v6: u32,
    /// `coap.multicast.leisure_ms` (§8.2): the period a response is sent
    /// in; `DEFAULT_LEISURE`, 5 s, by default.
    pub leisure_ms: u64,
    /// `coap.multicast.resources` (§11.3): the resources that answer a
    /// multicast request; `.well-known/core` by default.
    pub resources: Vec<String>,
    /// `coap.multicast.unauthenticated_resources` (§11.3): those of
    /// `resources` that answer a request without OSCORE; `.well-known/core`
    /// by default.
    pub unauthenticated_resources: Vec<String>,
}

impl Default for MulticastSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            groups: Vec::new(),
            interface_v4: std::net::Ipv4Addr::UNSPECIFIED,
            interface_v6: 0,
            leisure_ms: 5_000,
            resources: vec![".well-known/core".to_string()],
            unauthenticated_resources: vec![".well-known/core".to_string()],
        }
    }
}

/// Spec §7.2 — the `oscore:` config block (RFC 8613). The daemon acts as the
/// OSCORE server: per client `kid` a Sender/Recipient context is derived by
/// HKDF from the shared Master Secret/Salt (+ optional ID Context), with the
/// daemon's own Sender ID as the response/notification sender.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OscoreSettings {
    /// `oscore.enabled`.
    pub enabled: bool,
    /// `oscore.master_secret_hex` (decoded).
    pub master_secret: Vec<u8>,
    /// `oscore.master_salt_hex` (decoded; may be empty).
    pub master_salt: Vec<u8>,
    /// `oscore.id_context_hex` (decoded; empty string → none).
    pub id_context: Option<Vec<u8>>,
    /// `oscore.sender_id_hex` — the daemon's Sender ID (default `01`); clients
    /// must configure it as their Recipient ID.
    pub sender_id: Vec<u8>,
    /// `oscore.replay_window` — anti-replay window width (default 32,
    /// RFC 8613 §3.2.2).
    pub replay_window: u64,
}

impl Default for OscoreSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            master_secret: Vec::new(),
            master_salt: Vec::new(),
            id_context: None,
            sender_id: vec![0x01],
            replay_window: 32,
        }
    }
}

/// Topic entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
    /// `qos.history.kind:` — `keep_last` (the default) or `keep_all`.
    pub history_kind: String,
    /// `qos.lifespan_ms:` — how long a sample stays valid: the Max-Age of
    /// its representation (spec §6).
    pub lifespan_ms: Option<u64>,
    /// `qos.deadline_ms:` — the longest a topic goes without a sample;
    /// beyond it `5.03` with Max-Age 0 (spec §6).
    pub deadline_ms: Option<u64>,
    /// `qos.liveliness_lease_ms:` — the lease of the topic's DDS endpoints
    /// (DDS 1.4 §2.2.3.11, AUTOMATIC).
    pub liveliness_lease_ms: Option<u64>,
    /// `qos.partition:` — the partitions of the topic's DDS endpoints; a
    /// sample of another partition does not reach an observer (spec §6).
    pub partition: Vec<String>,
    /// `idl_file:` — the IDL file that declares `dds_type`: with it the
    /// topic takes and gives every form of §4.5 and `GET /schema/<slug>`
    /// returns it (§5.2).
    pub idl_file: String,
    /// `extensibility:` — `final`, `appendable` (the default, DDS-XTypes 1.3
    /// §7.3.1.2.1.8) or `mutable`: the XCDR2 header of a topic without
    /// `idl_file`, whose samples pass through.
    pub extensibility: String,
    /// `data_representation:` — what the topic's DDS writer sends: `xcdr2`
    /// (the default) or `xcdr1` (DDS-XTypes 1.3 §7.6.3.1.1), for readers
    /// that accept only XCDR1; `xcdr1` needs `idl_file`.
    pub data_representation: String,
}

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
            block_size: 1024,
            max_body_size: 65_536,
            keep_alive_secs: 0,
            content_formats: super::representation::ContentFormats::default(),
            topics: Vec::new(),
            dtls_enabled: false,
            dtls: crate::dtls::DtlsCredentials::default(),
            auth_mode: "none".to_string(),
            auth_bearer_token: None,
            auth_bearer_subject: None,
            auth_jwt: zerodds_bridge_security::JwtSettings::default(),
            topic_acl: std::collections::HashMap::new(),
            acl_default_deny: true,
            acl_default_deny_set: false,
            metrics_enabled: false,
            metrics_addr: String::new(),
            metrics_path: "/metrics".to_string(),
            oscore: OscoreSettings::default(),
            multicast: MulticastSettings::default(),
            proxy: ProxySettings::default(),
            tenants: Vec::new(),
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

        let coap = root.section("coap")?;
        coap.only(&[
            "bind",
            "bind_dtls",
            "max_message_size",
            "ack_timeout_ms",
            "max_retransmit",
            "observe_max_age_secs",
            "block_size",
            "max_body_size",
            "keep_alive_secs",
            "dtls",
            "multicast",
        ])?;
        if let Some(v) = coap.string("bind")? {
            out.bind = v;
        }
        out.bind_dtls = coap.string("bind_dtls")?;
        if let Some(v) = coap.parse::<usize>("max_message_size", "bytes, an integer")? {
            out.max_message_size = v;
        }
        if let Some(v) = coap.parse::<u32>("ack_timeout_ms", "milliseconds, an integer")? {
            out.ack_timeout_ms = v;
        }
        if let Some(v) = coap.parse::<u8>("max_retransmit", "an integer in 0..=255")? {
            out.max_retransmit = v;
        }
        if let Some(v) = coap.parse::<u32>("observe_max_age_secs", "seconds, an integer")? {
            out.observe_max_age_secs = v;
        }
        if let Some(v) = coap.parse::<u16>("block_size", "16, 32, 64, 128, 256, 512 or 1024")? {
            if !(16..=1024).contains(&v) || !v.is_power_of_two() {
                return Err(invalid(
                    coap.path_of("block_size"),
                    coap.node("block_size")
                        .map_or(zerodds_config::Origin::Default, first_origin),
                    format!("got `{v}`, expected 16, 32, 64, 128, 256, 512 or 1024"),
                ));
            }
            out.block_size = v;
        }
        if let Some(v) = coap.parse::<usize>("max_body_size", "bytes, an integer")? {
            out.max_body_size = v;
        }
        if let Some(v) = coap.parse::<u32>("keep_alive_secs", "seconds, an integer")? {
            out.keep_alive_secs = v;
        }
        let multicast = coap.section("multicast")?;
        multicast.only(&[
            "enabled",
            "groups",
            "interface_v4",
            "interface_v6",
            "leisure_ms",
            "resources",
            "unauthenticated_resources",
        ])?;
        if let Some(v) = multicast.boolean("enabled")? {
            out.multicast.enabled = v;
        }
        if let Some(groups) = multicast.strings("groups")? {
            let mut parsed = Vec::with_capacity(groups.len());
            for group in groups {
                let addr = group
                    .parse::<std::net::IpAddr>()
                    .ok()
                    .filter(std::net::IpAddr::is_multicast)
                    .ok_or_else(|| {
                        invalid(
                            "coap.multicast.groups".to_string(),
                            multicast
                                .node("groups")
                                .map_or(zerodds_config::Origin::Default, first_origin),
                            format!("`{group}` is no IP multicast address"),
                        )
                    })?;
                parsed.push(addr);
            }
            out.multicast.groups = parsed;
        }
        if let Some(v) = multicast.parse::<std::net::Ipv4Addr>("interface_v4", "an IPv4 address")? {
            out.multicast.interface_v4 = v;
        }
        if let Some(v) = multicast.parse::<u32>("interface_v6", "an interface index")? {
            out.multicast.interface_v6 = v;
        }
        if let Some(v) = multicast.parse::<u64>("leisure_ms", "milliseconds, an integer")? {
            out.multicast.leisure_ms = v;
        }
        if let Some(v) = multicast.strings("resources")? {
            out.multicast.resources = v;
        }
        if let Some(v) = multicast.strings("unauthenticated_resources")? {
            out.multicast.unauthenticated_resources = v;
        }

        let dtls = coap.section("dtls")?;
        dtls.only(&[
            "enabled",
            "cert_file",
            "key_file",
            "ca_file",
            "cipher_suites",
            "psk",
            "rpk_key_file",
            "rpk_trusted",
        ])?;
        if let Some(v) = dtls.boolean("enabled")? {
            out.dtls_enabled = v;
        }
        out.dtls.rpk_key_file = dtls.string("rpk_key_file")?;
        if let Some(names) = dtls.strings("rpk_trusted")? {
            check_key_names(
                &names,
                dtls.path_of("rpk_trusted"),
                dtls.node("rpk_trusted"),
            )?;
            out.dtls.rpk_trusted = names;
        }
        out.dtls.cert_file = dtls.string("cert_file")?;
        out.dtls.key_file = dtls.string("key_file")?;
        out.dtls.ca_file = dtls.string("ca_file")?;
        if let Some(names) = dtls.strings("cipher_suites")? {
            for name in names {
                // An unknown suite name is a config error the operator must
                // see, not a silently dropped line.
                let suite = crate::dtls::DtlsCipherSuite::from_name(&name).ok_or_else(|| {
                    invalid(
                        dtls.path_of("cipher_suites"),
                        dtls.node("cipher_suites")
                            .map_or(zerodds_config::Origin::Default, first_origin),
                        format!("unknown suite `{name}`"),
                    )
                })?;
                out.dtls.cipher_suites.push(suite);
            }
        }
        for (path, item) in dtls.items("psk")? {
            let origin = first_origin(item);
            let entry = Section::of(
                if matches!(item, zerodds_config::Node::Scalar(..)) {
                    None
                } else {
                    Some(item)
                },
                path.clone(),
            )?;
            // `identity=secret_hex`, the form `--dtls-psk-id`/`--dtls-psk`
            // reach the flag layer in; a map entry otherwise.
            let (identity, hex) = if let zerodds_config::Node::Scalar(text, _) = item {
                let (id, hex) = text.rsplit_once('=').ok_or_else(|| {
                    invalid(
                        path.clone(),
                        origin.clone(),
                        "expected identity=secret_hex or a map".to_string(),
                    )
                })?;
                (id.trim().to_string(), hex.trim().to_string())
            } else {
                entry.only(&["identity", "secret_hex"])?;
                let identity = entry.string("identity")?.ok_or_else(|| {
                    invalid(
                        entry.path_of("identity"),
                        origin.clone(),
                        "a psk entry needs an identity".to_string(),
                    )
                })?;
                let hex = entry.string("secret_hex")?.ok_or_else(|| {
                    invalid(
                        entry.path_of("secret_hex"),
                        origin.clone(),
                        "a psk entry needs a secret_hex".to_string(),
                    )
                })?;
                (identity, hex)
            };
            let secret = crate::dtls::parse_secret_hex(&hex).map_err(|e| {
                invalid(entry.path_of("secret_hex"), origin.clone(), format!("{e}"))
            })?;
            out.dtls
                .psk
                .add(&identity, secret)
                .map_err(|e| invalid(entry.path_of("identity"), origin.clone(), format!("{e}")))?;
        }

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
            if !v.starts_with('/') {
                return Err(invalid(
                    metrics.path_of("path"),
                    metrics
                        .node("path")
                        .map_or(zerodds_config::Origin::Default, first_origin),
                    format!("`{v}` is no absolute path"),
                ));
            }
            out.metrics_path = v;
        }

        // Spec §3, §4.5: the Content-Format number of each form.
        let formats = root.section("content_format")?;
        formats.only(&[
            "cdr2_le_id",
            "cdr2_be_id",
            "cdr1_le_id",
            "json_id",
            "cbor_id",
        ])?;
        for (key, slot) in [
            ("cdr2_le_id", &mut out.content_formats.cdr2_le),
            ("cdr2_be_id", &mut out.content_formats.cdr2_be),
            ("cdr1_le_id", &mut out.content_formats.cdr1_le),
            ("json_id", &mut out.content_formats.json),
            ("cbor_id", &mut out.content_formats.cbor),
        ] {
            if let Some(v) = formats.parse::<u16>(key, "a Content-Format number, 0 to 65535")? {
                *slot = v;
            }
        }
        if out.content_formats.has_duplicates() {
            return Err(invalid(
                "content_format".to_string(),
                zerodds_config::Origin::Default,
                "two forms under one Content-Format number".to_string(),
            ));
        }

        let auth = root.section("auth")?;
        auth.only(&["mode", "bearer_token", "bearer_subject", "jwt"])?;
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

        // Spec §3/§7.3: `acl: { default_deny, rules: [ { subject,
        // allow_post: [...], allow_observe: [...] } ] }`. `allow_post` =
        // write, `allow_observe` = read; inverted into the resource-keyed
        // map the security layer consumes.
        let acl = root.section("acl")?;
        acl.only(&["default_deny", "rules"])?;
        if let Some(v) = acl.boolean("default_deny")? {
            out.acl_default_deny = v;
            out.acl_default_deny_set = true;
        }
        for (path, rule) in acl.items("rules")? {
            let entry = Section::of(Some(rule), path)?;
            entry.only(&["subject", "allow_post", "allow_observe"])?;
            let subject = entry.string("subject")?.ok_or_else(|| {
                invalid(
                    entry.path_of("subject"),
                    first_origin(rule),
                    "an acl rule needs a subject".to_string(),
                )
            })?;
            for pat in entry.strings("allow_observe")?.unwrap_or_default() {
                out.topic_acl
                    .entry(pat)
                    .or_default()
                    .0
                    .push(subject.clone());
            }
            for pat in entry.strings("allow_post")?.unwrap_or_default() {
                out.topic_acl
                    .entry(pat)
                    .or_default()
                    .1
                    .push(subject.clone());
            }
        }

        let proxy = root.section("proxy")?;
        proxy.only(&[
            "forward",
            "schemes",
            "names",
            "timeout_ms",
            "reverse",
            "cache_entries",
            "require_dtls_for_coaps",
            "max_hops",
            "multicast_wait_ms",
            "dtls",
        ])?;
        if let Some(v) = proxy.boolean("forward")? {
            out.proxy.forward = v;
        }
        if let Some(v) = proxy.strings("schemes")? {
            for scheme in &v {
                if scheme != "coap" && scheme != "coaps" {
                    return Err(invalid(
                        proxy.path_of("schemes"),
                        proxy
                            .node("schemes")
                            .map_or(zerodds_config::Origin::Default, first_origin),
                        format!("`{scheme}` is no scheme this proxy speaks (coap, coaps)"),
                    ));
                }
            }
            out.proxy.schemes = v;
        }
        if let Some(v) = proxy.strings("names")? {
            out.proxy.names = v;
        }
        if let Some(v) = proxy.parse::<u64>("timeout_ms", "milliseconds, an integer")? {
            out.proxy.timeout_ms = v;
        }
        for (path, item) in proxy.items("reverse")? {
            let route = Section::of(Some(item), path.clone())?;
            route.only(&["path", "origin"])?;
            let (Some(local), Some(origin)) = (route.string("path")?, route.string("origin")?)
            else {
                return Err(invalid(
                    path,
                    first_origin(item),
                    "a reverse route needs path and origin".to_string(),
                ));
            };
            if crate::uri::parse_coap_uri(&origin).is_err() {
                return Err(invalid(
                    route.path_of("origin"),
                    first_origin(item),
                    format!("`{origin}` is no coap or coaps URI"),
                ));
            }
            out.proxy
                .reverse
                .push((local.trim_matches('/').to_string(), origin));
        }
        if let Some(v) = proxy.parse::<usize>("cache_entries", "an integer")? {
            out.proxy.cache_entries = v;
        }
        if let Some(v) = proxy.boolean("require_dtls_for_coaps")? {
            out.proxy.require_dtls_for_coaps = v;
        }
        if let Some(v) = proxy.parse::<u8>("max_hops", "an integer in 1..=255")? {
            out.proxy.max_hops = v.max(1);
        }
        if let Some(v) = proxy.parse::<u64>("multicast_wait_ms", "milliseconds, an integer")? {
            out.proxy.multicast_wait_ms = v;
        }
        let proxy_dtls = proxy.section("dtls")?;
        proxy_dtls.only(&[
            "ca_file",
            "cert_file",
            "key_file",
            "psk_identity",
            "psk_secret_hex",
            "rpk_key_file",
            "rpk_trusted",
        ])?;
        out.proxy.dtls_ca_file = proxy_dtls.string("ca_file")?;
        out.proxy.dtls_rpk_key_file = proxy_dtls.string("rpk_key_file")?;
        if let Some(names) = proxy_dtls.strings("rpk_trusted")? {
            check_key_names(
                &names,
                proxy_dtls.path_of("rpk_trusted"),
                proxy_dtls.node("rpk_trusted"),
            )?;
            out.proxy.dtls_rpk_trusted = names;
        }
        out.proxy.dtls_cert_file = proxy_dtls.string("cert_file")?;
        out.proxy.dtls_key_file = proxy_dtls.string("key_file")?;
        match (
            proxy_dtls.string("psk_identity")?,
            proxy_dtls.string("psk_secret_hex")?,
        ) {
            (Some(identity), Some(hex)) => {
                let key = crate::dtls::parse_secret_hex(&hex).map_err(|e| {
                    invalid(
                        proxy_dtls.path_of("psk_secret_hex"),
                        proxy_dtls
                            .node("psk_secret_hex")
                            .map_or(zerodds_config::Origin::Default, first_origin),
                        format!("{e}"),
                    )
                })?;
                out.proxy.dtls_psk = Some((identity, key));
            }
            (None, None) => {}
            _ => {
                return Err(invalid(
                    proxy_dtls.path_of("psk_identity"),
                    zerodds_config::Origin::Default,
                    "psk_identity and psk_secret_hex go together".to_string(),
                ));
            }
        }

        let oscore = root.section("oscore")?;
        oscore.only(&[
            "enabled",
            "master_secret_hex",
            "master_salt_hex",
            "id_context_hex",
            "sender_id_hex",
            "replay_window",
        ])?;
        if let Some(v) = oscore.boolean("enabled")? {
            out.oscore.enabled = v;
        }
        if let Some(b) = hex_field(&oscore, "master_secret_hex")? {
            out.oscore.master_secret = b;
        }
        if let Some(b) = hex_field(&oscore, "master_salt_hex")? {
            out.oscore.master_salt = b;
        }
        if let Some(b) = hex_field(&oscore, "id_context_hex")? {
            out.oscore.id_context = if b.is_empty() { None } else { Some(b) };
        }
        if let Some(b) = hex_field(&oscore, "sender_id_hex")? {
            out.oscore.sender_id = b;
        }
        if let Some(v) = oscore.parse::<u64>("replay_window", "an integer")? {
            out.oscore.replay_window = v;
        }

        for (path, item) in root.items("topics")? {
            out.topics.push(read_topic(&path, item)?);
        }

        // Spec §1 L6: the tenants, each a participant with its topics.
        for (path, item) in root.items("tenants")? {
            let entry = Section::of(Some(item), path.clone())?;
            entry.only(&["name", "domain", "subjects", "topics"])?;
            let origin = first_origin(item);
            let name = entry
                .string("name")?
                .filter(|n| !n.is_empty())
                .ok_or_else(|| {
                    invalid(
                        entry.path_of("name"),
                        origin.clone(),
                        "a tenant needs a name".to_string(),
                    )
                })?;
            if out.tenants.iter().any(|t| t.name == name) {
                return Err(invalid(
                    entry.path_of("name"),
                    origin,
                    format!("a second tenant `{name}`"),
                ));
            }
            let domain = entry
                .parse::<i32>("domain", "a DDS domain id")?
                .unwrap_or(out.domain);
            let subjects = entry.strings("subjects")?.unwrap_or_default();
            if subjects.is_empty() {
                return Err(invalid(
                    entry.path_of("subjects"),
                    origin,
                    "a tenant needs the DTLS subjects bound to it".to_string(),
                ));
            }
            if let Some(s) = subjects
                .iter()
                .find(|s| out.tenants.iter().any(|t| t.subjects.contains(s)))
            {
                return Err(invalid(
                    entry.path_of("subjects"),
                    origin,
                    format!("`{s}` is bound to another tenant"),
                ));
            }
            let mut topics = Vec::new();
            for (topic_path, topic) in entry.items("topics")? {
                topics.push(read_topic(&topic_path, topic)?);
            }
            out.tenants.push(TenantConfig {
                name,
                domain,
                subjects,
                topics,
            });
        }

        // Spec §7.1: `coaps://` needs something to authenticate with. A
        // cert_file without its key (or the other way round) would fall back
        // to a self-signed certificate the operator never asked for.
        if out.dtls_enabled && out.dtls.cert_file.is_some() != out.dtls.key_file.is_some() {
            return Err(invalid(
                "coap.dtls".to_string(),
                dtls.node("cert_file")
                    .or_else(|| dtls.node("key_file"))
                    .map_or(zerodds_config::Origin::Default, first_origin),
                "cert_file and key_file must be set together".to_string(),
            ));
        }
        if out.oscore.enabled && out.oscore.master_secret.is_empty() {
            return Err(invalid(
                "oscore.master_secret_hex".to_string(),
                oscore
                    .node("enabled")
                    .map_or(zerodds_config::Origin::Default, first_origin),
                "required when oscore.enabled".to_string(),
            ));
        }
        Ok(out)
    }
}

/// A hex field (whitespace tolerated), decoded.
fn hex_field(section: &Section<'_>, key: &str) -> Result<Option<Vec<u8>>, ConfigError> {
    let Some((text, origin)) = section.scalar(key)? else {
        return Ok(None);
    };
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let bad = || {
        invalid(
            section.path_of(key),
            origin.clone(),
            "expected an even number of hex digits".to_string(),
        )
    };
    if !compact.len().is_multiple_of(2) {
        return Err(bad());
    }
    (0..compact.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&compact[i..i + 2], 16).map_err(|_| bad()))
        .collect::<Result<Vec<u8>, _>>()
        .map(Some)
}

/// One `topics[]` entry; a bare `DDS[:URI]` text (`--topic`) is
/// `{dds_name: DDS, coap_uri_path: URI}`.
fn read_topic(path: &str, item: &zerodds_config::Node) -> Result<TopicConfig, ConfigError> {
    let mut t = TopicConfig {
        direction: "bidir".to_string(),
        history_depth: 10,
        history_kind: "keep_last".to_string(),
        extensibility: "appendable".to_string(),
        data_representation: "xcdr2".to_string(),
        ..TopicConfig::default()
    };
    if let zerodds_config::Node::Scalar(text, _) = item {
        let (dds, uri) = zerodds_config::read::split_topic_shorthand(text);
        t.dds_name = dds;
        t.coap_uri_path = uri.unwrap_or_default();
    } else {
        let entry = Section::of(Some(item), path)?;
        entry.only(&[
            "dds_name",
            "dds_type",
            "coap_uri_path",
            "direction",
            "qos",
            "idl_file",
            "extensibility",
            "data_representation",
        ])?;
        t.dds_name = entry.string("dds_name")?.unwrap_or_default();
        t.idl_file = entry.string("idl_file")?.unwrap_or_default();
        if let Some(v) = entry.choice("extensibility", &["final", "appendable", "mutable"])? {
            t.extensibility = v;
        }
        if let Some(v) = entry.choice("data_representation", &["xcdr2", "xcdr1"])? {
            if v == "xcdr1" && t.idl_file.is_empty() {
                return Err(invalid(
                    entry.path_of("data_representation"),
                    first_origin(item),
                    "xcdr1 needs idl_file: the daemon converts the samples to it".to_string(),
                ));
            }
            t.data_representation = v;
        }
        t.dds_type = entry.string("dds_type")?.unwrap_or_default();
        t.coap_uri_path = entry.string("coap_uri_path")?.unwrap_or_default();
        if let Some(v) = entry.choice("direction", &["in", "out", "bidir"])? {
            t.direction = v;
        }
        let qos = entry.section("qos")?;
        qos.only(&[
            "reliability",
            "durability",
            "history",
            "lifespan_ms",
            "deadline_ms",
            "liveliness_lease_ms",
            "partition",
        ])?;
        t.lifespan_ms = qos.parse::<u64>("lifespan_ms", "milliseconds, an integer")?;
        t.deadline_ms = qos.parse::<u64>("deadline_ms", "milliseconds, an integer")?;
        t.liveliness_lease_ms =
            qos.parse::<u64>("liveliness_lease_ms", "milliseconds, an integer")?;
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
        if let Some(v) = history.choice("kind", &["keep_last", "keep_all"])? {
            t.history_kind = v;
        }
        if let Some(depth) = history.parse::<i32>("depth", "an integer >= 1")? {
            if depth < 1 {
                return Err(invalid(
                    history.path_of("depth"),
                    first_origin(item),
                    format!("got `{depth}`, expected an integer >= 1"),
                ));
            }
            t.history_depth = depth;
        }
    }
    if t.dds_name.is_empty() {
        return Err(invalid(
            format!("{path}.dds_name"),
            first_origin(item),
            "a topic entry needs a dds_name".to_string(),
        ));
    }
    if t.dds_type.is_empty() {
        t.dds_type = t.dds_name.clone();
    }
    if t.coap_uri_path.is_empty() {
        t.coap_uri_path = default_coap_path(&t.dds_name);
    }
    Ok(t)
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
            ConfigError::Invalid { .. }
        ));
    }

    #[test]
    fn parses_oscore_block() {
        let yaml = "\
domain: 0
oscore:
  enabled: true
  master_secret_hex: \"0102030405060708090a0b0c0d0e0f10\"
  master_salt_hex: \"9e7ca92223786340\"
  id_context_hex: \"\"
  sender_id_hex: \"01\"
  replay_window: 32
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert!(cfg.oscore.enabled);
        assert_eq!(cfg.oscore.master_secret.len(), 16);
        assert_eq!(cfg.oscore.master_salt.len(), 8);
        assert_eq!(cfg.oscore.id_context, None);
        assert_eq!(cfg.oscore.sender_id, vec![0x01]);
        assert_eq!(cfg.oscore.replay_window, 32);
    }

    #[test]
    fn oscore_enabled_without_master_secret_is_refused() {
        let yaml = "\
oscore:
  enabled: true
";
        assert!(matches!(
            DaemonConfig::load_from_str(yaml).unwrap_err(),
            ConfigError::Invalid { .. }
        ));
    }

    #[test]
    fn oscore_bad_hex_is_refused() {
        let yaml = "\
oscore:
  enabled: true
  master_secret_hex: \"zz\"
";
        assert!(matches!(
            DaemonConfig::load_from_str(yaml).unwrap_err(),
            ConfigError::Invalid { .. }
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

    #[test]
    fn a_psk_given_as_identity_equals_hex_is_a_psk_entry() {
        let yaml = "coap:\n  dtls:\n    psk:\n      - client-1=00112233445566778899aabbccddeeff\n";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert!(!cfg.dtls.psk.is_empty());
        let err =
            DaemonConfig::load_from_str("coap:\n  dtls:\n    psk:\n      - nohex\n").unwrap_err();
        assert!(err.to_string().contains("coap.dtls.psk[0]"), "{err}");
    }

    #[test]
    fn what_the_old_parser_turned_into_defaults_is_refused() {
        for (yaml, setting) in [
            ("coap:\n  ack_timeout_ms: soon\n", "coap.ack_timeout_ms"),
            ("coap:\n  max_retransmit: 300\n", "coap.max_retransmit"),
            ("coap:\n  dtls:\n    enabled: yes\n", "coap.dtls.enabled"),
            ("oscore:\n  replay_windw: 4\n", "oscore.replay_windw"),
            (
                "topics:\n  - dds_name: T\n    direction: sideways\n",
                "topics[0].direction",
            ),
        ] {
            let err = DaemonConfig::load_from_str(yaml).unwrap_err();
            assert!(
                matches!(&err, ConfigError::Invalid { setting: s, .. } if s == setting),
                "{yaml}: {err}"
            );
        }
    }

    /// Spec §3: the keys of §3 and §6 and what each refuses.
    #[test]
    fn coapbridge_3_the_keys_of_the_forms_the_blocks_and_the_qos() {
        let yaml = "coap:\n  block_size: 64\n  max_body_size: 4096\n  keep_alive_secs: 30\n\
                    content_format:\n  json_id: 51\nmetrics:\n  path: /prom\n\
                    topics:\n  - dds_name: T\n    idl_file: t.idl\n    data_representation: xcdr1\n\
                    \x20   extensibility: mutable\n    qos:\n      history: { kind: keep_all, depth: 2 }\n\
                    \x20     lifespan_ms: 100\n      deadline_ms: 200\n      liveliness_lease_ms: 300\n\
                    \x20     partition: [a, b]\n";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert_eq!(
            (cfg.block_size, cfg.max_body_size, cfg.keep_alive_secs),
            (64, 4096, 30)
        );
        assert_eq!(cfg.content_formats.json, 51);
        assert_eq!(cfg.metrics_path, "/prom");
        let t = &cfg.topics[0];
        assert_eq!(
            (
                t.idl_file.as_str(),
                t.data_representation.as_str(),
                t.extensibility.as_str()
            ),
            ("t.idl", "xcdr1", "mutable")
        );
        assert_eq!((t.history_kind.as_str(), t.history_depth), ("keep_all", 2));
        assert_eq!(
            (t.lifespan_ms, t.deadline_ms, t.liveliness_lease_ms),
            (Some(100), Some(200), Some(300))
        );
        assert_eq!(t.partition, vec!["a".to_string(), "b".to_string()]);
        for (yaml, setting) in [
            ("coap:\n  block_size: 100\n", "coap.block_size"),
            ("coap:\n  block_size: 2048\n", "coap.block_size"),
            ("metrics:\n  path: metrics\n", "metrics.path"),
            ("content_format:\n  json_id: 65000\n", "content_format"),
            (
                "topics:\n  - dds_name: T\n    data_representation: xcdr1\n",
                "topics[0].data_representation",
            ),
            (
                "topics:\n  - dds_name: T\n    extensibility: open\n",
                "topics[0].extensibility",
            ),
        ] {
            let err = DaemonConfig::load_from_str(yaml).unwrap_err();
            assert!(
                matches!(&err, ConfigError::Invalid { setting: s, .. } if s == setting),
                "{yaml}: {err}"
            );
        }
    }

    /// Spec §1 L6, §3: the tenants and what their entries refuse.
    #[test]
    fn coapbridge_1_l6_the_tenants() {
        let yaml = "domain: 3\ntenants:\n  - name: plant-a\n    domain: 9\n    subjects: [\"psk:gw\"]\n    topics:\n      - dds_name: A::B\n  - name: plant-b\n    subjects: [\"CN=b\"]\n";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert_eq!(cfg.tenants.len(), 2);
        assert_eq!(cfg.tenants[0].domain, 9);
        assert_eq!(cfg.tenants[0].topics[0].coap_uri_path, "a/b");
        assert_eq!(
            cfg.tenants[1].domain, 3,
            "the domain of the daemon by default"
        );
        for (yaml, setting) in [
            ("tenants:\n  - subjects: [x]\n", "tenants[0].name"),
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
                "tenants:\n  - name: a\n    subjects: [x]\n    bind: z\n",
                "tenants[0].bind",
            ),
        ] {
            let err = DaemonConfig::load_from_str(yaml).unwrap_err();
            assert!(
                matches!(&err, ConfigError::Invalid { setting: s, .. } if s == setting),
                "{yaml}: {err}"
            );
        }
    }
}
