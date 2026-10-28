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
    "topics",
];

/// Whether `path` is one of [`KEYS`].
#[must_use]
pub fn is_key(path: &str) -> bool {
    KEYS.contains(&path)
}

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
}

/// Single topic map entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
        metrics.only(&["enabled", "listen"])?;
        if let Some(v) = metrics.boolean("enabled")? {
            out.metrics_enabled = v;
        }
        // `listen`, as §3 of every bridge spec spells it.
        if let Some(v) = metrics.string("listen")? {
            out.metrics_addr = v;
        }

        for (path, item) in root.items("topics")? {
            let t = read_topic(&path, item, &mut out.topic_acl)?;
            out.topics.push(t);
        }
        Ok(out)
    }
}

/// One `topics[]` entry. A bare name (`- Chat::Message`, or
/// `--topic Chat::Message`) is the entry `{name: Chat::Message}`.
fn read_topic(
    path: &str,
    item: &zerodds_config::Node,
    acl: &mut std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
) -> Result<TopicConfig, ConfigError> {
    let mut t = TopicConfig {
        direction: "bidir".to_string(),
        history_depth: 10,
        ..TopicConfig::default()
    };
    if let zerodds_config::Node::Scalar(name, _) = item {
        t.name = name.trim().to_string();
    } else {
        let entry = Section::of(Some(item), path)?;
        entry.only(&["name", "type", "direction", "ws_path", "qos", "acl"])?;
        t.name = entry.string("name")?.unwrap_or_default();
        t.type_name = entry.string("type")?.unwrap_or_default();
        if let Some(v) = entry.choice("direction", &["in", "out", "bidir"])? {
            t.direction = v;
        }
        t.ws_path = entry.string("ws_path")?.unwrap_or_default();
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
    Ok(t)
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
        let cfg = DaemonConfig::load_from_str(
            "listen: ${ZERODDS_PROBABLY_UNSET_VAR_e2afb0b9_test:-10.0.0.1:1}\n",
        )
        .unwrap();
        assert_eq!(cfg.listen, "10.0.0.1:1");
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
    fn a_bare_topic_name_is_a_topic_entry() {
        let cfg = DaemonConfig::load_from_str("topics:\n  - Chat::Message\n").unwrap();
        assert_eq!(cfg.topics[0].name, "Chat::Message");
        assert_eq!(cfg.topics[0].direction, "bidir");
        assert_eq!(cfg.topics[0].ws_path, "/topics/chat/message");
    }
}
