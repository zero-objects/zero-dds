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
    "coap.dtls.enabled",
    "coap.dtls.cert_file",
    "coap.dtls.key_file",
    "coap.dtls.ca_file",
    "coap.dtls.cipher_suites",
    "coap.dtls.psk",
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
    "oscore.enabled",
    "oscore.master_secret_hex",
    "oscore.master_salt_hex",
    "oscore.id_context_hex",
    "oscore.sender_id_hex",
    "oscore.replay_window",
    "topics",
];

/// Whether `path` is one of [`KEYS`].
#[must_use]
pub fn is_key(path: &str) -> bool {
    KEYS.contains(&path)
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
    /// `coap.observe_max_age_secs:`.
    pub observe_max_age_secs: u32,
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
    /// Spec §7.2 — OSCORE (RFC 8613) settings.
    pub oscore: OscoreSettings,
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
            oscore: OscoreSettings::default(),
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
            "dtls",
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
        let dtls = coap.section("dtls")?;
        dtls.only(&[
            "enabled",
            "cert_file",
            "key_file",
            "ca_file",
            "cipher_suites",
            "psk",
        ])?;
        if let Some(v) = dtls.boolean("enabled")? {
            out.dtls_enabled = v;
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
        metrics.only(&["enabled", "listen"])?;
        if let Some(v) = metrics.boolean("enabled")? {
            out.metrics_enabled = v;
        }
        // `listen`, as §3 of every bridge spec spells it.
        if let Some(v) = metrics.string("listen")? {
            out.metrics_addr = v;
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
    if compact.len() % 2 != 0 {
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
        ..TopicConfig::default()
    };
    if let zerodds_config::Node::Scalar(text, _) = item {
        let (dds, uri) = zerodds_config::read::split_topic_shorthand(text);
        t.dds_name = dds;
        t.coap_uri_path = uri.unwrap_or_default();
    } else {
        let entry = Section::of(Some(item), path)?;
        entry.only(&["dds_name", "dds_type", "coap_uri_path", "direction", "qos"])?;
        t.dds_name = entry.string("dds_name")?.unwrap_or_default();
        t.dds_type = entry.string("dds_type")?.unwrap_or_default();
        t.coap_uri_path = entry.string("coap_uri_path")?.unwrap_or_default();
        if let Some(v) = entry.choice("direction", &["in", "out", "bidir"])? {
            t.direction = v;
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
}
