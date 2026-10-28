// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Config for `zerodds-zenoh-bridged`.
//!
//! Read by the shared layered loader (`zerodds-config`): base file,
//! `zenoh-bridged.d/` drop-ins, includes, `${VAR}`, and the precedence
//! defaults < environment < files < `--set`. [`DaemonConfig::from_config`]
//! reads this daemon's keys and refuses an unknown key inside a known
//! section, and a value of the wrong shape or type, with its file and line.

use std::path::Path;
use std::string::{String, ToString};
use std::vec::Vec;

pub use zerodds_config::ConfigError;
use zerodds_config::read::{Section, first_origin};

/// The base name of the daemon's file: `zenoh-bridged.yaml`, `zenoh-bridged.d/`.
pub const CONFIG_STEM: &str = "zenoh-bridged";

/// Every key this daemon reads; list-valued keys stop at the list.
pub const KEYS: &[&str] = &[
    "domain",
    "log_level",
    "zenoh.mode",
    "zenoh.connect",
    "zenoh.listen",
    "zenoh.scouting.multicast.enabled",
    "zenoh.scope",
    "zenoh.reliable_routes_blocking",
    "zenoh.queries_timeout_ms",
    "zenoh.config_file",
    "discovery.auto_bridge",
    "discovery.poll_interval_ms",
    "discovery.allow",
    "discovery.deny",
    "limits.max_payload_bytes",
    "metrics.enabled",
    "metrics.listen",
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

/// `zenoh:` config block — the Zenoh session's own settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZenohConfig {
    /// `zenoh.mode` — `peer` (default) | `client` | `router`.
    pub mode: String,
    /// `zenoh.connect` — endpoints to connect to, e.g. `tcp/127.0.0.1:7447`.
    pub connect: Vec<String>,
    /// `zenoh.listen` — endpoints to listen on.
    pub listen: Vec<String>,
    /// `zenoh.scouting.multicast.enabled` — default `true` (Zenoh's own
    /// default); `false` disables UDP multicast scouting, matching
    /// `zenoh-bridge-dds --no-multicast-scouting`.
    pub multicast_scouting: bool,
    /// `zenoh.scope` — the key-expression scope (`crate::mapping::
    /// key_expr_for_topic`'s `scope`). Empty = no scope segment, matching
    /// `zenoh-plugin-dds`'s own default.
    pub scope: String,
    /// `zenoh.reliable_routes_blocking` — default `true` (the plugin's
    /// default). See `crate::mapping::congestion_control`.
    pub reliable_routes_blocking: bool,
    /// `zenoh.queries_timeout_ms` — default 5000 (the plugin's default
    /// `queries_timeout` is 5.0 s). TRANSIENT_LOCAL late-joiner query
    /// timeout.
    pub queries_timeout_ms: u64,
    /// `zenoh.config_file` — an optional full Zenoh JSON5/YAML5 config file
    /// (TLS, auth, advanced transport settings) loaded as the base
    /// `zenoh::Config`, with `mode`/`connect`/`listen`/`scouting` above
    /// applied over it. `None` = start from `zenoh::Config::default()`.
    pub config_file: Option<String>,
}

impl Default for ZenohConfig {
    fn default() -> Self {
        Self {
            mode: "peer".to_string(),
            connect: Vec::new(),
            listen: Vec::new(),
            multicast_scouting: true,
            scope: String::new(),
            reliable_routes_blocking: true,
            queries_timeout_ms: 5000,
            config_file: None,
        }
    }
}

/// `discovery:` config block — auto-bridging of DDS topics the daemon
/// discovers (via `DcpsRuntime::discovered_publication_endpoints` /
/// `discovered_subscription_endpoints`, the DCPS built-in topics), the way
/// `zenoh-plugin-dds` bridges every DDS topic it sees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryConfig {
    /// `discovery.auto_bridge` — default `true`.
    pub auto_bridge: bool,
    /// `discovery.poll_interval_ms` — the longest time between two passes
    /// of the discovery loop. A pass also runs on every discovery change
    /// the runtime reports (a new or removed endpoint, a TypeLookup reply).
    /// Default 1000.
    pub poll_interval_ms: u64,
    /// `discovery.allow` — glob patterns matched against the derived key
    /// expression (`crate::mapping::topic_allowed`). Empty = everything
    /// allowed (matching `zenoh-plugin-dds`'s `allow: None` default).
    pub allow: Vec<String>,
    /// `discovery.deny` — glob patterns; deny wins over allow.
    pub deny: Vec<String>,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            auto_bridge: true,
            poll_interval_ms: 1000,
            allow: Vec::new(),
            deny: Vec::new(),
        }
    }
}

/// `limits:` config block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitsConfig {
    /// `limits.max_payload_bytes` — a DDS sample larger than this is
    /// refused (logged + counted, per the daemon's forward-error path),
    /// not silently truncated. Default 1 MiB — large enough for the
    /// crate's own 1 MiB payload test, small enough to bound one
    /// misbehaving publisher's memory footprint.
    pub max_payload_bytes: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_payload_bytes: 1024 * 1024,
        }
    }
}

/// One `topics[]` entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TopicConfig {
    /// `dds_name:`.
    pub dds_name: String,
    /// `dds_type:`.
    pub dds_type: String,
    /// `direction:` — `in` (Zenoh→DDS) | `out` (DDS→Zenoh) | `bidir`.
    pub direction: String,
    /// `partition:` — empty = default partition.
    pub partition: String,
    /// `key_expr:` — overrides the derived key expression.
    pub key_expr: String,
    /// `keyed:` — has one or more `@key` fields.
    pub keyed: bool,
    /// `type_file:` — path to a `CompleteTypeObject` blob (see
    /// `crates/dcps/src/serialized_type.rs`), read at registration time.
    /// Required when `keyed` and `direction` is `in` or `bidir` (see
    /// `crate::runtime::TopicSpec::keyed`).
    pub type_file: String,
    /// `qos.reliability:`.
    pub reliability: String,
    /// `qos.durability:`.
    pub durability: String,
    /// `qos.extensibility:` — `final` (default) | `appendable` | `mutable`.
    pub extensibility: String,
    /// `qos.history.depth:` — `PublicationCache`/`FetchingSubscriber`
    /// history depth for a `TransientLocal` topic.
    pub history_depth: usize,
}

/// Daemon config.
#[derive(Debug, Clone, PartialEq)]
pub struct DaemonConfig {
    /// `domain:`.
    pub domain: i32,
    /// `log_level:`.
    pub log_level: String,
    /// `zenoh:`.
    pub zenoh: ZenohConfig,
    /// `discovery:`.
    pub discovery: DiscoveryConfig,
    /// `limits:`.
    pub limits: LimitsConfig,
    /// `metrics.enabled`.
    pub metrics_enabled: bool,
    /// `metrics.listen`.
    pub metrics_addr: String,
    /// `topics:`.
    pub topics: Vec<TopicConfig>,
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
            zenoh: ZenohConfig::default(),
            discovery: DiscoveryConfig::default(),
            limits: LimitsConfig::default(),
            metrics_enabled: false,
            metrics_addr: String::new(),
            topics: Vec::new(),
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

        let zenoh = root.section("zenoh")?;
        zenoh.only(&[
            "mode",
            "connect",
            "listen",
            "scouting",
            "scope",
            "reliable_routes_blocking",
            "queries_timeout_ms",
            "config_file",
        ])?;
        if let Some(v) = zenoh.choice("mode", &["peer", "client", "router"])? {
            out.zenoh.mode = v;
        }
        if let Some(v) = zenoh.strings("connect")? {
            out.zenoh.connect = v;
        }
        if let Some(v) = zenoh.strings("listen")? {
            out.zenoh.listen = v;
        }
        let scouting = zenoh.section("scouting")?;
        scouting.only(&["multicast"])?;
        let multicast = scouting.section("multicast")?;
        multicast.only(&["enabled"])?;
        if let Some(v) = multicast.boolean("enabled")? {
            out.zenoh.multicast_scouting = v;
        }
        if let Some(v) = zenoh.string("scope")? {
            out.zenoh.scope = v;
        }
        if let Some(v) = zenoh.boolean("reliable_routes_blocking")? {
            out.zenoh.reliable_routes_blocking = v;
        }
        if let Some(v) = zenoh.parse::<u64>("queries_timeout_ms", "milliseconds, an integer")? {
            out.zenoh.queries_timeout_ms = v;
        }
        out.zenoh.config_file = zenoh.string("config_file")?;

        let discovery = root.section("discovery")?;
        discovery.only(&["auto_bridge", "poll_interval_ms", "allow", "deny"])?;
        if let Some(v) = discovery.boolean("auto_bridge")? {
            out.discovery.auto_bridge = v;
        }
        if let Some(v) = discovery.parse::<u64>("poll_interval_ms", "milliseconds, an integer")? {
            out.discovery.poll_interval_ms = v;
        }
        if let Some(v) = discovery.strings("allow")? {
            out.discovery.allow = v;
        }
        if let Some(v) = discovery.strings("deny")? {
            out.discovery.deny = v;
        }

        let limits = root.section("limits")?;
        limits.only(&["max_payload_bytes"])?;
        if let Some(v) = limits.parse::<usize>("max_payload_bytes", "bytes, an integer")? {
            out.limits.max_payload_bytes = v;
        }

        let metrics = root.section("metrics")?;
        metrics.only(&["enabled", "listen"])?;
        if let Some(v) = metrics.boolean("enabled")? {
            out.metrics_enabled = v;
        }
        if let Some(v) = metrics.string("listen")? {
            out.metrics_addr = v;
        }

        for (path, item) in root.items("topics")? {
            out.topics.push(read_topic(&path, item)?);
        }

        // A keyed topic bridged Zenoh->DDS needs a type_file to derive the
        // real per-instance key (crate::runtime::TopicSpec::keyed); refuse
        // at config time rather than letting every sample collapse onto one
        // instance at runtime.
        for t in &out.topics {
            if t.keyed && matches!(t.direction.as_str(), "in" | "bidir") && t.type_file.is_empty() {
                return Err(invalid(
                    "topics[].type_file".to_string(),
                    zerodds_config::Origin::Default,
                    format!(
                        "keyed topic '{}' is bridged Zenoh->DDS (direction {}) and needs a type_file",
                        t.dds_name, t.direction
                    ),
                ));
            }
        }

        Ok(out)
    }
}

/// One `topics[]` entry; a bare `DDS` text is `{dds_name: DDS}`.
fn read_topic(path: &str, item: &zerodds_config::Node) -> Result<TopicConfig, ConfigError> {
    let mut t = TopicConfig {
        direction: "bidir".to_string(),
        extensibility: "final".to_string(),
        history_depth: 1,
        ..TopicConfig::default()
    };
    if let zerodds_config::Node::Scalar(text, _) = item {
        t.dds_name = text.trim().to_string();
    } else {
        let entry = Section::of(Some(item), path)?;
        entry.only(&[
            "dds_name",
            "dds_type",
            "direction",
            "partition",
            "key_expr",
            "keyed",
            "type_file",
            "qos",
        ])?;
        t.dds_name = entry.string("dds_name")?.unwrap_or_default();
        t.dds_type = entry.string("dds_type")?.unwrap_or_default();
        if let Some(v) = entry.choice("direction", &["in", "out", "bidir"])? {
            t.direction = v;
        }
        t.partition = entry.string("partition")?.unwrap_or_default();
        t.key_expr = entry.string("key_expr")?.unwrap_or_default();
        if let Some(v) = entry.boolean("keyed")? {
            t.keyed = v;
        }
        t.type_file = entry.string("type_file")?.unwrap_or_default();
        let qos = entry.section("qos")?;
        qos.only(&["reliability", "durability", "extensibility", "history"])?;
        t.reliability = qos
            .choice("reliability", &["reliable", "best_effort"])?
            .unwrap_or_default();
        t.durability = qos
            .choice(
                "durability",
                &["volatile", "transient_local", "transient", "persistent"],
            )?
            .unwrap_or_default();
        if let Some(v) = qos.choice("extensibility", &["final", "appendable", "mutable"])? {
            t.extensibility = v;
        }
        let history = qos.section("history")?;
        history.only(&["depth"])?;
        if let Some(depth) = history.parse::<i64>("depth", "an integer >= 1")? {
            if depth < 1 {
                return Err(invalid(
                    history.path_of("depth"),
                    first_origin(item),
                    format!("got `{depth}`, expected an integer >= 1"),
                ));
            }
            t.history_depth = depth as usize;
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
    Ok(t)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_config() {
        let yaml = "\
domain: 7
zenoh:
  mode: client
  connect: [\"tcp/127.0.0.1:7447\"]
topics:
  - dds_name: T
    direction: in
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert_eq!(cfg.domain, 7);
        assert_eq!(cfg.zenoh.mode, "client");
        assert_eq!(cfg.zenoh.connect, vec!["tcp/127.0.0.1:7447".to_string()]);
        assert_eq!(cfg.topics[0].direction, "in");
    }

    #[test]
    fn bare_topic_scalar_is_just_a_name() {
        let yaml = "topics:\n  - T\n";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert_eq!(cfg.topics[0].dds_name, "T");
        assert_eq!(cfg.topics[0].dds_type, "T");
        assert_eq!(cfg.topics[0].direction, "bidir");
    }

    #[test]
    fn rejects_topic_without_name() {
        let yaml = "topics:\n  - direction: in\n";
        assert!(matches!(
            DaemonConfig::load_from_str(yaml).unwrap_err(),
            ConfigError::Invalid { .. }
        ));
    }

    #[test]
    fn keyed_topic_bidir_without_type_file_is_refused() {
        let yaml = "\
topics:
  - dds_name: T
    keyed: true
    direction: bidir
";
        assert!(matches!(
            DaemonConfig::load_from_str(yaml).unwrap_err(),
            ConfigError::Invalid { .. }
        ));
    }

    #[test]
    fn keyed_topic_out_only_does_not_need_type_file() {
        let yaml = "\
topics:
  - dds_name: T
    keyed: true
    direction: out
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert!(cfg.topics[0].keyed);
    }

    #[test]
    fn keyed_topic_with_type_file_is_accepted() {
        let yaml = "\
topics:
  - dds_name: T
    keyed: true
    direction: bidir
    type_file: /etc/zerodds/types/T.typeobject
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert_eq!(cfg.topics[0].type_file, "/etc/zerodds/types/T.typeobject");
    }

    #[test]
    fn default_scope_is_empty_matching_zenoh_plugin_dds() {
        let cfg = DaemonConfig::default_for_dev();
        assert_eq!(cfg.zenoh.scope, "");
        assert!(cfg.zenoh.reliable_routes_blocking);
        assert_eq!(cfg.zenoh.queries_timeout_ms, 5000);
    }

    #[test]
    fn discovery_defaults_to_auto_bridge_everything() {
        let cfg = DaemonConfig::default_for_dev();
        assert!(cfg.discovery.auto_bridge);
        assert!(cfg.discovery.allow.is_empty());
        assert!(cfg.discovery.deny.is_empty());
    }

    #[test]
    fn extensibility_default_is_final() {
        let yaml = "topics:\n  - dds_name: T\n";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert_eq!(cfg.topics[0].extensibility, "final");
    }

    #[test]
    fn unknown_zenoh_key_is_refused() {
        let yaml = "zenoh:\n  mde: peer\n";
        let err = DaemonConfig::load_from_str(yaml).unwrap_err();
        assert!(matches!(&err, ConfigError::Invalid { .. }), "{err}");
    }

    #[test]
    fn history_depth_zero_is_refused() {
        let yaml = "topics:\n  - dds_name: T\n    qos:\n      history:\n        depth: 0\n";
        assert!(matches!(
            DaemonConfig::load_from_str(yaml).unwrap_err(),
            ConfigError::Invalid { .. }
        ));
    }
}
