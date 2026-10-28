// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The bridge daemon `zerodds-amqp-bridged`
//! (`docs/specs/zerodds-amqp-bridge-daemon-1.0.md`): a DDS participant on
//! one side, a client of an AMQP 1.0 broker on the other.
//!
//! Per topic of the configuration:
//!
//! * `out` / `bidir`: a DDS reader, and a sender link to the topic's AMQP
//!   address. Every sample becomes one message (§4.3): the Pass-Through
//!   body of DDS-AMQP 1.0 §8.1.1, the properties of §8.2, the `dds:*`
//!   application properties of §8.3.
//! * `in` / `bidir`: a receiver link from the address, and a DDS writer.
//!   Every message is written as a sample and settled (§4.4, §4.5):
//!   `accepted` once written, `rejected` for a body that is not a DDS
//!   sample, `released` when the write fails.
//!
//! A sample the broker has no credit for waits in the link's queue, bounded
//! by the topic's HISTORY (§6). Unsettled deliveries survive a reconnect and
//! are sent again on the new connection (§9.3). A message this bridge sent
//! itself, coming back on a receiver of the same address, is dropped by its
//! `dds:bridge-id` (DDS-AMQP 1.0 §8.4); a DDS sample this bridge wrote,
//! coming back on its own reader, is dropped by its writer GUID.

use std::collections::{BTreeMap, VecDeque};
use std::string::{String, ToString};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::vec::Vec;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::sections::MessageSection;
use zerodds_config::ConfigError;
use zerodds_config::read::{Section, first_origin};
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_rtps::wire_types::EntityId;

use crate::broker::{
    self, AmqpError, BrokerWriter, ConnectParams, Inbound, LinkSpec, Outcome, SaslCredentials,
};
use crate::coexistence::{CoexistenceConfig, InboundDecision, inspect_inbound, stamp_outbound};
use crate::daemon_runtime::BridgeMetrics;
use crate::properties::{
    DdsOperation, SampleHeader, app_keys, produce_application_properties, produce_properties,
};

// ============================================================================
// Configuration (spec §3).
// ============================================================================

/// Direction of a topic mapping (§4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// DDS → AMQP.
    Out,
    /// AMQP → DDS.
    In,
    /// Both.
    Bidir,
}

impl Direction {
    /// A sender link: DDS samples go to the broker.
    #[must_use]
    pub fn sends(self) -> bool {
        matches!(self, Self::Out | Self::Bidir)
    }
    /// A receiver link: broker messages go to DDS.
    #[must_use]
    pub fn receives(self) -> bool {
        matches!(self, Self::In | Self::Bidir)
    }
}

/// HISTORY of a topic (§6): the depth of the sender queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum History {
    /// KEEP_LAST(n).
    KeepLast(usize),
    /// KEEP_ALL, bounded by [`KEEP_ALL_QUEUE_CAP`].
    KeepAll,
}

/// The queue bound of a KEEP_ALL sender while the broker grants no credit.
/// Beyond it the oldest sample is dropped and counted.
pub const KEEP_ALL_QUEUE_CAP: usize = 65_536;

/// One `topics:` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicMapping {
    /// DDS topic name.
    pub dds_name: String,
    /// DDS type name; defaults to the topic name.
    pub dds_type: String,
    /// `true` for a type with `@key` (entity kind WithKey, RTPS §9.3.1.2).
    pub keyed: bool,
    /// `type_file`: a serialized COMPLETE TypeObject of `dds_type` (the
    /// format `zerodds-zenoh-bridged`'s `type_file` reads); empty for none.
    /// For a keyed topic bridged AMQP → DDS the writer reads instance keys
    /// with it: a dispose or unregister message names its instance by
    /// `dds:instance-handle` alone, and with the type the DDS marker carries
    /// the `serializedKey` Cyclone, Fast DDS and OpenDDS need to act on it.
    pub type_file: String,
    /// The AMQP node address.
    pub amqp_address: String,
    /// Direction.
    pub direction: Direction,
    /// `sender.settled`: settle on sending (AT_MOST_ONCE).
    pub settled: bool,
    /// `sender.durable`: terminus durability 0/1/2.
    pub durable: u32,
    /// `sender.snd_settle_mode`: 0 unsettled, 1 settled, 2 mixed.
    pub snd_settle_mode: u8,
    /// `sender.rcv_settle_mode`: 0 first, 1 second.
    pub rcv_settle_mode: u8,
    /// `receiver.credit`.
    pub credit: u32,
    /// `qos.reliability`: `true` = RELIABLE.
    pub reliable: bool,
    /// `qos.durability`.
    pub durability: zerodds_qos::DurabilityKind,
    /// `qos.history`.
    pub history: History,
}

/// `amqp.reconnect` (§9.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reconnect {
    /// First delay.
    pub initial_delay_ms: u64,
    /// Largest delay.
    pub max_delay_ms: u64,
    /// Growth per failed attempt.
    pub factor: f64,
}

/// The daemon's settings.
#[derive(Debug, Clone, PartialEq)]
pub struct DaemonConfig {
    /// DDS domain.
    pub domain: i32,
    /// `log_level`.
    pub log_level: String,
    /// `amqp.broker_url`: `amqp://` or `amqps://host[:port]`.
    pub broker_url: String,
    /// `amqp.container_id`.
    pub container_id: String,
    /// `amqp.channel_max`.
    pub channel_max: u16,
    /// `amqp.max_frame_size`.
    pub max_frame_size: u32,
    /// `amqp.idle_time_out_ms`.
    pub idle_time_out_ms: u32,
    /// `amqp.hostname`: `open.hostname`; the broker host when empty.
    pub hostname: String,
    /// `amqp.sasl.mechanism`.
    pub sasl_mechanism: String,
    /// `amqp.sasl.username`.
    pub sasl_username: String,
    /// `amqp.sasl.password` (for XOAUTH2 the bearer token).
    pub sasl_password: String,
    /// `amqp.tls.enabled`.
    pub tls_enabled: bool,
    /// `amqp.tls.ca_file`.
    pub tls_ca_file: String,
    /// `amqp.tls.cert_file`.
    pub tls_cert_file: String,
    /// `amqp.tls.key_file`.
    pub tls_key_file: String,
    /// `amqp.reconnect`.
    pub reconnect: Reconnect,
    /// `acl.default_deny`: a topic no rule names is closed.
    pub acl_default_deny: bool,
    /// `acl.rules`: per topic, the subjects allowed to read (DDS → AMQP)
    /// and write (AMQP → DDS).
    pub acl_rules: BTreeMap<String, (Vec<String>, Vec<String>)>,
    /// `metrics.enabled`.
    pub metrics_enabled: bool,
    /// `metrics.listen`.
    pub metrics_listen: String,
    /// `catalog.enabled`: publish the topic catalog on connect (§5.2).
    pub catalog_enabled: bool,
    /// `catalog.address`; `zerodds.bridge.<container-id>.catalog` when
    /// empty.
    pub catalog_address: String,
    /// Topic mappings.
    pub topics: Vec<TopicMapping>,
}

fn default_container_id() -> String {
    let host = std::env::var("HOSTNAME")
        .ok()
        .map(|h| {
            let mut s = h.replace('.', "-");
            s.truncate(32);
            s
        })
        .unwrap_or_else(|| format!("pid{}", std::process::id()));
    format!("zerodds-bridge-{host}")
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            domain: 0,
            log_level: "info".into(),
            broker_url: String::new(),
            container_id: default_container_id(),
            channel_max: 256,
            max_frame_size: 65_536,
            idle_time_out_ms: 30_000,
            hostname: String::new(),
            sasl_mechanism: "ANONYMOUS".into(),
            sasl_username: String::new(),
            sasl_password: String::new(),
            tls_enabled: false,
            tls_ca_file: String::new(),
            tls_cert_file: String::new(),
            tls_key_file: String::new(),
            reconnect: Reconnect {
                initial_delay_ms: 500,
                max_delay_ms: 30_000,
                factor: 2.0,
            },
            acl_default_deny: false,
            acl_rules: BTreeMap::new(),
            metrics_enabled: false,
            metrics_listen: String::new(),
            catalog_enabled: true,
            catalog_address: String::new(),
            topics: Vec::new(),
        }
    }
}

/// Every key the daemon reads (§3); `topics` and `acl.rules` stop at the
/// list.
pub const KEYS: &[&str] = &[
    "domain",
    "log_level",
    "amqp.broker_url",
    "amqp.container_id",
    "amqp.channel_max",
    "amqp.max_frame_size",
    "amqp.idle_time_out_ms",
    "amqp.hostname",
    "amqp.sasl.mechanism",
    "amqp.sasl.username",
    "amqp.sasl.password",
    "amqp.tls.enabled",
    "amqp.tls.ca_file",
    "amqp.tls.cert_file",
    "amqp.tls.key_file",
    "amqp.tls.verify_hostname",
    "amqp.tls.alpn",
    "amqp.reconnect.initial_delay_ms",
    "amqp.reconnect.max_delay_ms",
    "amqp.reconnect.factor",
    "acl.default_deny",
    "acl.rules",
    "metrics.enabled",
    "metrics.listen",
    "metrics.path",
    "catalog.enabled",
    "catalog.address",
    "topics",
];

/// Whether `path` is a key [`DaemonConfig::from_config`] reads.
#[must_use]
pub fn is_key(path: &str) -> bool {
    KEYS.contains(&path)
}

/// The daemon's own environment names (§2).
pub const ENV: &[(&str, &str)] = &[("AMQP_PASSWORD", "amqp.sasl.password")];

fn invalid(
    setting: impl Into<String>,
    origin: zerodds_config::Origin,
    message: impl Into<String>,
) -> ConfigError {
    ConfigError::Invalid {
        setting: setting.into(),
        origin,
        message: message.into(),
    }
}

impl DaemonConfig {
    /// Reads the daemon's keys out of a merged tree, over the defaults.
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] naming the setting and its origin.
    pub fn from_config(config: Option<&zerodds_config::Config>) -> Result<Self, ConfigError> {
        let root = Section::root(config.and_then(zerodds_config::Config::root))?;
        let mut cfg = Self::default();
        if let Some(v) = root.parse::<i32>("domain", "a DDS domain id")? {
            cfg.domain = v;
        }
        if let Some(v) = root.choice("log_level", &["trace", "debug", "info", "warn", "error"])? {
            cfg.log_level = v;
        }
        let amqp = root.section("amqp")?;
        amqp.only(&[
            "broker_url",
            "container_id",
            "channel_max",
            "max_frame_size",
            "idle_time_out_ms",
            "hostname",
            "sasl",
            "tls",
            "reconnect",
        ])?;
        if let Some((url, origin)) = amqp.scalar("broker_url")? {
            parse_broker_url(url).map_err(|m| invalid("amqp.broker_url", origin.clone(), m))?;
            cfg.broker_url = url.to_string();
        }
        if let Some(v) = amqp.string("container_id")? {
            cfg.container_id = v;
        }
        if let Some(v) = amqp.parse::<u16>("channel_max", "an integer in 0..=65535")? {
            cfg.channel_max = v;
        }
        if let Some((v, origin)) = amqp.scalar("max_frame_size")? {
            let n: u32 = v.parse().ok().filter(|n| *n >= 512).ok_or_else(|| {
                invalid(
                    "amqp.max_frame_size",
                    origin.clone(),
                    "bytes, at least 512 (AMQP Part 2 §2.7.1)",
                )
            })?;
            cfg.max_frame_size = n;
        }
        if let Some(v) = amqp.parse::<u32>("idle_time_out_ms", "milliseconds, 0 for none")? {
            cfg.idle_time_out_ms = v;
        }
        if let Some(v) = amqp.string("hostname")? {
            cfg.hostname = v;
        }
        let sasl = amqp.section("sasl")?;
        sasl.only(&["mechanism", "username", "password"])?;
        if let Some(v) = sasl.choice(
            "mechanism",
            &["PLAIN", "SCRAM-SHA-256", "EXTERNAL", "ANONYMOUS", "XOAUTH2"],
        )? {
            cfg.sasl_mechanism = v;
        }
        if let Some(v) = sasl.string("username")? {
            cfg.sasl_username = v;
        }
        if let Some(v) = sasl.string("password")? {
            cfg.sasl_password = v;
        }
        let tls = amqp.section("tls")?;
        tls.only(&[
            "enabled",
            "ca_file",
            "cert_file",
            "key_file",
            "verify_hostname",
            "alpn",
        ])?;
        if let Some(v) = tls.boolean("enabled")? {
            cfg.tls_enabled = v;
        }
        if let Some(v) = tls.string("ca_file")? {
            cfg.tls_ca_file = v;
        }
        if let Some(v) = tls.string("cert_file")? {
            cfg.tls_cert_file = v;
        }
        if let Some(v) = tls.string("key_file")? {
            cfg.tls_key_file = v;
        }
        if let Some((v, origin)) = tls.scalar("verify_hostname")? {
            if v != "true" {
                return Err(invalid(
                    "amqp.tls.verify_hostname",
                    origin.clone(),
                    "the broker certificate is always checked against the host; \
                     connect by the name the certificate carries",
                ));
            }
        }
        // §7.1 fixes ALPN to `amqp`; the key exists so the file can say so.
        for (path, item) in tls.items("alpn")? {
            if item.as_str() != Some("amqp") {
                return Err(invalid(
                    path,
                    first_origin(item),
                    "the AMQP ALPN protocol id is `amqp` (RFC 7301 registry)",
                ));
            }
        }
        let rc = amqp.section("reconnect")?;
        rc.only(&["initial_delay_ms", "max_delay_ms", "factor"])?;
        if let Some(v) = rc.parse::<u64>("initial_delay_ms", "milliseconds")? {
            cfg.reconnect.initial_delay_ms = v;
        }
        if let Some(v) = rc.parse::<u64>("max_delay_ms", "milliseconds")? {
            cfg.reconnect.max_delay_ms = v;
        }
        if let Some((v, origin)) = rc.scalar("factor")? {
            cfg.reconnect.factor = v
                .parse::<f64>()
                .ok()
                .filter(|f| *f >= 1.0 && f.is_finite())
                .ok_or_else(|| {
                    invalid("amqp.reconnect.factor", origin.clone(), "a number >= 1.0")
                })?;
        }
        let acl = root.section("acl")?;
        acl.only(&["default_deny", "rules"])?;
        if let Some(v) = acl.boolean("default_deny")? {
            cfg.acl_default_deny = v;
        }
        for (path, item) in acl.items("rules")? {
            let entry = Section::of(Some(item), path.clone())?;
            entry.only(&["topic", "read", "write"])?;
            let topic = entry.string("topic")?.ok_or_else(|| {
                invalid(
                    format!("{path}.topic"),
                    first_origin(item),
                    "an ACL rule names its topic",
                )
            })?;
            let list = |key: &str| -> Result<Vec<String>, ConfigError> {
                Ok(entry
                    .items(key)?
                    .into_iter()
                    .filter_map(|(_, n)| n.as_str().map(str::to_string))
                    .collect())
            };
            cfg.acl_rules.insert(topic, (list("read")?, list("write")?));
        }
        let metrics = root.section("metrics")?;
        metrics.only(&["enabled", "listen", "path"])?;
        if let Some(v) = metrics.boolean("enabled")? {
            cfg.metrics_enabled = v;
        }
        if let Some(v) = metrics.string("listen")? {
            cfg.metrics_listen = v;
        }
        if let Some((v, origin)) = metrics.scalar("path")? {
            if v != "/metrics" {
                return Err(invalid(
                    "metrics.path",
                    origin.clone(),
                    "the admin endpoint serves /metrics, /catalog and /healthz",
                ));
            }
        }
        let catalog = root.section("catalog")?;
        catalog.only(&["enabled", "address"])?;
        if let Some(v) = catalog.boolean("enabled")? {
            cfg.catalog_enabled = v;
        }
        if let Some(v) = catalog.string("address")? {
            cfg.catalog_address = v;
        }
        for (path, item) in root.items("topics")? {
            cfg.topics.push(read_topic(&path, item)?);
        }
        let mut names = std::collections::BTreeSet::new();
        for t in &cfg.topics {
            if !names.insert(t.dds_name.as_str()) {
                return Err(invalid(
                    "topics",
                    zerodds_config::Origin::Default,
                    format!("topic {} is mapped twice", t.dds_name),
                ));
            }
        }
        Ok(cfg)
    }

    /// The address the catalog goes to (§5.2).
    #[must_use]
    pub fn catalog_address(&self) -> String {
        if self.catalog_address.is_empty() {
            format!("zerodds.bridge.{}.catalog", self.container_id)
        } else {
            self.catalog_address.clone()
        }
    }

    /// The catalog message of §5.2: JSON with every topic, its type,
    /// address and QoS.
    #[must_use]
    pub fn catalog_json(&self) -> String {
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let topics: Vec<String> = self
            .topics
            .iter()
            .map(|t| {
                format!(
                    "{{\"dds_name\":\"{}\",\"dds_type\":\"{}\",\"amqp_address\":\"{}\",\"direction\":\"{}\",\"qos\":{{\"reliability\":\"{}\",\"durability\":\"{}\"}}}}",
                    esc(&t.dds_name),
                    esc(&t.dds_type),
                    esc(&t.amqp_address),
                    match t.direction {
                        Direction::In => "in",
                        Direction::Out => "out",
                        Direction::Bidir => "bidir",
                    },
                    if t.reliable { "reliable" } else { "best_effort" },
                    match t.durability {
                        zerodds_qos::DurabilityKind::Volatile => "volatile",
                        zerodds_qos::DurabilityKind::TransientLocal => "transient_local",
                        zerodds_qos::DurabilityKind::Transient => "transient",
                        zerodds_qos::DurabilityKind::Persistent => "persistent",
                    }
                )
            })
            .collect();
        format!("{{\"topics\":[{}]}}", topics.join(","))
    }

    /// The SASL credentials of the configuration.
    ///
    /// # Errors
    /// A mechanism that needs a username or password without one.
    pub fn sasl(&self) -> Result<SaslCredentials, String> {
        let need = |what: &str, v: &str| {
            if v.is_empty() {
                Err(format!(
                    "amqp.sasl.mechanism {} needs amqp.sasl.{what}",
                    self.sasl_mechanism
                ))
            } else {
                Ok(v.to_string())
            }
        };
        Ok(match self.sasl_mechanism.as_str() {
            "ANONYMOUS" => SaslCredentials::Anonymous,
            "EXTERNAL" => {
                if !self.tls_enabled || self.tls_cert_file.is_empty() {
                    return Err(
                        "EXTERNAL needs TLS with a client certificate (amqp.tls.cert_file)".into(),
                    );
                }
                SaslCredentials::External
            }
            "PLAIN" => SaslCredentials::Plain {
                username: need("username", &self.sasl_username)?,
                password: need("password", &self.sasl_password)?,
            },
            "SCRAM-SHA-256" => SaslCredentials::ScramSha256 {
                username: need("username", &self.sasl_username)?,
                password: need("password", &self.sasl_password)?,
            },
            "XOAUTH2" => SaslCredentials::XOAuth2 {
                username: need("username", &self.sasl_username)?,
                token: need("password", &self.sasl_password)?,
            },
            other => return Err(format!("unknown SASL mechanism {other}")),
        })
    }
}

fn read_topic(path: &str, item: &zerodds_config::Node) -> Result<TopicMapping, ConfigError> {
    let mut t = TopicMapping {
        dds_name: String::new(),
        dds_type: String::new(),
        keyed: false,
        type_file: String::new(),
        amqp_address: String::new(),
        direction: Direction::Bidir,
        settled: false,
        durable: 0,
        snd_settle_mode: 0,
        rcv_settle_mode: 0,
        credit: 256,
        reliable: true,
        durability: zerodds_qos::DurabilityKind::Volatile,
        history: History::KeepLast(1),
    };
    if let zerodds_config::Node::Scalar(text, origin) = item {
        // `--topic DDS=ADDR`.
        let (dds, addr) = text.split_once('=').ok_or_else(|| {
            invalid(
                path,
                origin.clone(),
                format!("got `{text}`, expected DDS=ADDRESS or a map"),
            )
        })?;
        t.dds_name = dds.trim().to_string();
        t.amqp_address = addr.trim().to_string();
    } else {
        let entry = Section::of(Some(item), path.to_string())?;
        entry.only(&[
            "dds_name",
            "dds_type",
            "keyed",
            "type_file",
            "amqp_address",
            "direction",
            "sender",
            "receiver",
            "qos",
        ])?;
        t.dds_name = entry.string("dds_name")?.unwrap_or_default();
        t.dds_type = entry.string("dds_type")?.unwrap_or_default();
        if let Some(v) = entry.boolean("keyed")? {
            t.keyed = v;
        }
        t.type_file = entry.string("type_file")?.unwrap_or_default();
        t.amqp_address = entry.string("amqp_address")?.unwrap_or_default();
        if let Some(v) = entry.choice("direction", &["in", "out", "bidir"])? {
            t.direction = match v.as_str() {
                "in" => Direction::In,
                "out" => Direction::Out,
                _ => Direction::Bidir,
            };
        }
        let sender = entry.section("sender")?;
        sender.only(&["settled", "durable", "snd_settle_mode", "rcv_settle_mode"])?;
        if let Some(v) = sender.boolean("settled")? {
            t.settled = v;
        }
        let small = |s: &Section<'_>,
                     key: &str,
                     max: u32,
                     what: &str|
         -> Result<Option<u32>, ConfigError> {
            match s.scalar(key)? {
                None => Ok(None),
                Some((v, origin)) => v
                    .parse::<u32>()
                    .ok()
                    .filter(|n| *n <= max)
                    .map(Some)
                    .ok_or_else(|| {
                        invalid(
                            format!("{path}.sender.{key}"),
                            origin.clone(),
                            what.to_string(),
                        )
                    }),
            }
        };
        if let Some(v) = small(
            &sender,
            "durable",
            2,
            "0 none, 1 configuration, 2 unsettled-state",
        )? {
            t.durable = v;
        }
        if let Some(v) = small(
            &sender,
            "snd_settle_mode",
            2,
            "0 unsettled, 1 settled, 2 mixed",
        )? {
            t.snd_settle_mode = v as u8;
        }
        if let Some(v) = small(&sender, "rcv_settle_mode", 1, "0 first, 1 second")? {
            t.rcv_settle_mode = v as u8;
        }
        let receiver = entry.section("receiver")?;
        receiver.only(&["credit"])?;
        if let Some((v, origin)) = receiver.scalar("credit")? {
            t.credit = v.parse::<u32>().ok().filter(|n| *n > 0).ok_or_else(|| {
                invalid(
                    format!("{path}.receiver.credit"),
                    origin.clone(),
                    "messages, at least 1",
                )
            })?;
        }
        let qos = entry.section("qos")?;
        qos.only(&["reliability", "durability", "history"])?;
        if let Some(v) = qos.choice("reliability", &["reliable", "best_effort"])? {
            t.reliable = v == "reliable";
        }
        if let Some(v) = qos.choice(
            "durability",
            &["volatile", "transient_local", "transient", "persistent"],
        )? {
            t.durability = match v.as_str() {
                "transient_local" => zerodds_qos::DurabilityKind::TransientLocal,
                "transient" => zerodds_qos::DurabilityKind::Transient,
                "persistent" => zerodds_qos::DurabilityKind::Persistent,
                _ => zerodds_qos::DurabilityKind::Volatile,
            };
        }
        let history = qos.section("history")?;
        history.only(&["kind", "depth"])?;
        let depth = history.parse::<usize>("depth", "a KEEP_LAST depth, at least 1")?;
        match history
            .choice("kind", &["keep_last", "keep_all"])?
            .as_deref()
        {
            Some("keep_all") => {
                if depth.is_some() {
                    return Err(invalid(
                        format!("{path}.qos.history.depth"),
                        first_origin(item),
                        "KEEP_ALL has no depth",
                    ));
                }
                t.history = History::KeepAll;
            }
            _ => {
                let d = depth.unwrap_or(1);
                if d == 0 {
                    return Err(invalid(
                        format!("{path}.qos.history.depth"),
                        first_origin(item),
                        "at least 1",
                    ));
                }
                t.history = History::KeepLast(d);
            }
        }
    }
    if t.dds_name.is_empty() || t.amqp_address.is_empty() {
        return Err(invalid(
            path,
            first_origin(item),
            "a topic entry needs dds_name and amqp_address",
        ));
    }
    if t.dds_type.is_empty() {
        t.dds_type = t.dds_name.clone();
    }
    // §6: BEST_EFFORT is AT_MOST_ONCE, settled on sending.
    if !t.reliable {
        t.settled = true;
    }
    if t.settled {
        t.snd_settle_mode = 1;
    }
    Ok(t)
}

/// Parses `amqp://` or `amqps://host[:port]` into `(host, port, tls)`;
/// ports 5672 and 5671 by default.
///
/// # Errors
/// What is wrong with the URL.
pub fn parse_broker_url(url: &str) -> Result<(String, u16, bool), String> {
    let (tls, rest) = if let Some(r) = url.strip_prefix("amqps://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("amqp://") {
        (false, r)
    } else {
        return Err(format!(
            "got `{url}`, expected amqp://host[:port] or amqps://"
        ));
    };
    let authority = rest.split('/').next().unwrap_or_default();
    if authority.contains('@') {
        return Err("user information in the URL is not read; set amqp.sasl.username and amqp.sasl.password".into());
    }
    let (host, port) = if let Some(v6) = authority.strip_prefix('[') {
        let (h, after) = v6
            .split_once(']')
            .ok_or_else(|| format!("got `{url}`, an IPv6 address without `]`"))?;
        (h, after.strip_prefix(':'))
    } else {
        match authority.split_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (authority, None),
        }
    };
    if host.is_empty() {
        return Err(format!("got `{url}`, which names no host"));
    }
    let port = match port {
        None => {
            if tls {
                5671
            } else {
                5672
            }
        }
        Some(p) => p
            .parse()
            .map_err(|_| format!("port `{p}` is not 0..=65535"))?,
    };
    Ok((host.to_string(), port, tls))
}

// ============================================================================
// Messages (spec §4.3, §4.4; DDS-AMQP 1.0 §8).
// ============================================================================

/// The content type of a Pass-Through body (DDS-AMQP 1.0 §8.1.1).
pub const CONTENT_TYPE_XCDR2: &str = "application/vnd.dds.xcdr2";

/// The content type this bridge gives an XCDR1 sample. DDS-AMQP 1.0 §8.1.1
/// defines Pass-Through for XCDR2; an XCDR1 sample travels with its XCDR1
/// encapsulation header under this type instead of being relabelled.
pub const CONTENT_TYPE_XCDR1: &str = "application/vnd.dds.xcdr1";

/// The encapsulation header of a body (RTPS 2.5 §10.5): representation
/// identifier and options.
fn encapsulation(representation: u8, big_endian: bool) -> [u8; 4] {
    let id: u16 = match (representation, big_endian) {
        (0, true) => 0x0000,
        (0, false) => 0x0001,
        (_, true) => 0x0006,
        (_, false) => 0x0007,
    };
    let b = id.to_be_bytes();
    [b[0], b[1], 0, 0]
}

/// What a DDS sample contributes to its message.
#[derive(Debug, Clone)]
pub struct OutSample<'a> {
    /// CDR payload without the encapsulation header.
    pub payload: &'a [u8],
    /// 0 = XCDR1, 1 = XCDR2.
    pub representation: u8,
    /// Byte order of `payload`.
    pub big_endian: bool,
    /// Key hash; `None` for an unkeyed topic.
    pub key_hash: Option<[u8; 16]>,
    /// Writer GUID.
    pub writer_guid: [u8; 16],
    /// RTPS sequence number.
    pub sequence: i64,
    /// Source timestamp in nanoseconds since the epoch.
    pub source_timestamp_ns: Option<u64>,
    /// `dds:operation`.
    pub operation: DdsOperation,
    /// The writer's LIFESPAN in milliseconds; `None` for infinite.
    pub lifespan_ms: Option<u64>,
}

/// Encodes a DDS sample as the message of §4.3: header, properties,
/// application properties, a Pass-Through data body.
///
/// # Errors
/// An encoding error of the AMQP codec.
pub fn encode_message(
    topic: &TopicMapping,
    domain: i32,
    coexistence: &CoexistenceConfig,
    s: &OutSample<'_>,
) -> Result<Vec<u8>, String> {
    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX));
    let ts = s.source_timestamp_ns.unwrap_or(now_ns);
    let hdr = SampleHeader {
        writer_guid: s.writer_guid,
        seqnum: u64::try_from(s.sequence.max(0)).unwrap_or(0),
        source_timestamp_ms: i64::try_from(ts / 1_000_000).unwrap_or(i64::MAX),
        source_nsec_remainder: u32::try_from(ts % 1_000_000).unwrap_or(0),
        keyhash: s.key_hash.map(|k| k.to_vec()),
        instance_handle: s.key_hash.unwrap_or([0; 16]),
        lifespan_remaining_ms: s.lifespan_ms.map(|l| i64::try_from(l).unwrap_or(i64::MAX)),
        operation: s.operation,
        type_id_hex: None,
        domain_id: u32::try_from(domain).unwrap_or(0),
        partitions: Vec::new(),
    };
    let props = produce_properties(&hdr);
    let content_type = if s.representation == 0 {
        CONTENT_TYPE_XCDR1
    } else {
        CONTENT_TYPE_XCDR2
    };
    let durable = !matches!(topic.durability, zerodds_qos::DurabilityKind::Volatile);
    // §6: LIFESPAN is the message's ttl.
    let ttl = s.lifespan_ms.map_or(AmqpExtValue::Null, |l| {
        AmqpExtValue::Uint(u32::try_from(l).unwrap_or(u32::MAX))
    });
    let header = MessageSection::Header(AmqpExtValue::List(vec![
        AmqpExtValue::Boolean(durable),
        AmqpExtValue::Ubyte(4),
        ttl,
        AmqpExtValue::Boolean(true),
    ]));
    let properties = MessageSection::Properties(AmqpExtValue::List(vec![
        AmqpExtValue::Binary(props.message_id),
        AmqpExtValue::Null, // user-id
        AmqpExtValue::Str(topic.amqp_address.clone()),
        AmqpExtValue::Str(topic.dds_name.clone()),
        AmqpExtValue::Null, // reply-to
        AmqpExtValue::Null, // correlation-id
        AmqpExtValue::Symbol(content_type.to_string()),
        AmqpExtValue::Null, // content-encoding
        props
            .absolute_expiry_time_ms
            .map_or(AmqpExtValue::Null, AmqpExtValue::Timestamp),
        AmqpExtValue::Timestamp(props.creation_time_ms),
        props.group_id.map_or(AmqpExtValue::Null, AmqpExtValue::Str),
    ]));
    let mut app = produce_application_properties(&hdr);
    stamp_outbound(coexistence, &mut app);
    let mut body = Vec::with_capacity(4 + s.payload.len());
    body.extend_from_slice(&encapsulation(s.representation, s.big_endian));
    body.extend_from_slice(s.payload);
    let mut out = Vec::with_capacity(body.len() + 256);
    for section in [
        header,
        properties,
        MessageSection::ApplicationProperties(app),
        MessageSection::Data(body),
    ] {
        out.extend_from_slice(&section.encode().map_err(|e| format!("{e:?}"))?);
    }
    Ok(out)
}

/// A message from the broker, as far as the bridge reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InMessage {
    /// `properties.content-type`.
    pub content_type: Option<String>,
    /// `application-properties`.
    pub application_properties: AmqpExtValue,
    /// The body: all data sections in order, or a binary amqp-value.
    pub body: Vec<u8>,
    /// `dds:operation`.
    pub operation: DdsOperation,
    /// `dds:instance-handle`: the key hash of the instance.
    pub instance_handle: Option<[u8; 16]>,
}

fn app_prop<'a>(props: &'a AmqpExtValue, key: &str) -> Option<&'a AmqpExtValue> {
    match props {
        AmqpExtValue::Map(entries) => entries.iter().find_map(|(k, v)| match k {
            AmqpExtValue::Str(s) | AmqpExtValue::Symbol(s) if s == key => Some(v),
            _ => None,
        }),
        _ => None,
    }
}

/// Decodes the sections of a message.
///
/// # Errors
/// A section the codec cannot read, a body that is neither data nor a
/// binary value, an unknown `dds:operation`.
pub fn decode_message(bytes: &[u8]) -> Result<InMessage, String> {
    let mut rest = bytes;
    let mut msg = InMessage {
        content_type: None,
        application_properties: AmqpExtValue::Map(Vec::new()),
        body: Vec::new(),
        operation: DdsOperation::Write,
        instance_handle: None,
    };
    let mut has_body = false;
    while !rest.is_empty() {
        let (section, n) =
            MessageSection::decode(rest).map_err(|e| format!("message section: {e:?}"))?;
        if n == 0 {
            break;
        }
        rest = &rest[n..];
        match section {
            MessageSection::Properties(AmqpExtValue::List(fields)) => {
                msg.content_type = match fields.get(6) {
                    Some(AmqpExtValue::Symbol(s) | AmqpExtValue::Str(s)) => Some(s.clone()),
                    _ => None,
                };
            }
            MessageSection::ApplicationProperties(v) => msg.application_properties = v,
            MessageSection::Data(b) => {
                msg.body.extend_from_slice(&b);
                has_body = true;
            }
            MessageSection::AmqpValue(AmqpExtValue::Binary(b)) => {
                msg.body = b;
                has_body = true;
            }
            MessageSection::AmqpValue(_) | MessageSection::AmqpSequence(_) => {
                return Err("an amqp-value or amqp-sequence body; the bridge reads Pass-Through data bodies".into());
            }
            _ => {}
        }
    }
    if !has_body {
        return Err("a message without a body".into());
    }
    if let Some(v) = app_prop(&msg.application_properties, app_keys::OPERATION) {
        let text = match v {
            AmqpExtValue::Str(s) | AmqpExtValue::Symbol(s) => s.as_str(),
            _ => return Err("dds:operation is not a string".into()),
        };
        msg.operation =
            DdsOperation::parse(text).map_err(|o| format!("unknown dds:operation {o}"))?;
    }
    if let Some(AmqpExtValue::Binary(b)) =
        app_prop(&msg.application_properties, app_keys::INSTANCE_HANDLE)
    {
        if b.len() == 16 {
            let mut k = [0u8; 16];
            k.copy_from_slice(b);
            if k != [0; 16] {
                msg.instance_handle = Some(k);
            }
        }
    }
    Ok(msg)
}

/// The representation and byte order an encapsulation header names, and
/// the payload behind it.
///
/// # Errors
/// A body shorter than the header, or a representation identifier the
/// bridge does not write (RTPS 2.5 §10.5).
pub fn split_encapsulation(body: &[u8]) -> Result<(u8, bool, &[u8]), String> {
    if body.len() < 4 {
        return Err("a body shorter than the encapsulation header".into());
    }
    let id = u16::from_be_bytes([body[0], body[1]]);
    let (rep, be) = match id {
        0x0000 => (0, true),
        0x0001 => (0, false),
        0x0002 => (0, true),  // PL_CDR_BE
        0x0003 => (0, false), // PL_CDR_LE
        0x0006 | 0x0008 | 0x000a => (1, true),
        0x0007 | 0x0009 | 0x000b => (1, false),
        other => {
            return Err(format!(
                "encapsulation {other:#06x} is not a CDR representation"
            ));
        }
    };
    Ok((rep, be, &body[4..]))
}

// ============================================================================
// ACL (spec §7.3).
// ============================================================================

fn allowed(cfg: &DaemonConfig, subject: Option<&str>, topic: &str, write: bool) -> bool {
    acl_allows(cfg.acl_default_deny, &cfg.acl_rules, subject, topic, write)
}

type AclRules = BTreeMap<String, (Vec<String>, Vec<String>)>;

fn acl_allows(
    default_deny: bool,
    rules: &AclRules,
    subject: Option<&str>,
    topic: &str,
    write: bool,
) -> bool {
    match rules.get(topic) {
        Some((read, wr)) => {
            let list = if write { wr } else { read };
            list.iter().any(|s| s == "*" || Some(s.as_str()) == subject)
        }
        None => !default_deny,
    }
}

/// What a SIGHUP replaces while the daemon runs (§9.2): the ACL, and the TLS
/// material of the next connect.
struct Live {
    acl: std::sync::RwLock<(bool, AclRules)>,
    tls: std::sync::Mutex<Option<Arc<rustls::ClientConfig>>>,
    /// The bridge's own SASL subject, which the ACL names.
    subject: Option<String>,
}

impl Live {
    fn allows(&self, topic: &str, write: bool) -> bool {
        self.acl
            .read()
            .map(|a| acl_allows(a.0, &a.1, self.subject.as_deref(), topic, write))
            .unwrap_or(false)
    }
}

/// What `run_with_reload` needs to reload: the flag the signal watcher sets,
/// and how to read the configuration again.
pub struct Reload {
    /// Set on SIGHUP; cleared when the reload is done.
    pub flag: Arc<AtomicBool>,
    /// Reads and validates the configuration again (the same layered sources
    /// the daemon started from).
    pub load: Box<dyn Fn() -> Result<DaemonConfig, String> + Send + Sync>,
}

/// One reload (§9.2): the ACL applies at once, a withdrawn right stops the
/// topic's samples and transfers; the TLS material applies to the next
/// connect. A right the new ACL grants, changed topics, broker or domain
/// need a restart (the DDS endpoints are made at start) and are logged. A
/// configuration or TLS material that does not load changes nothing.
fn apply_reload(live: &Live, started: &DaemonConfig, new: Result<DaemonConfig, String>) {
    let new = match new {
        Ok(c) => c,
        Err(e) => {
            log("reload_refused", &[("err", &e)]);
            return;
        }
    };
    let tls = match tls_config(&new) {
        Ok(t) => t,
        Err(e) => {
            log("reload_refused", &[("err", &e)]);
            return;
        }
    };
    let mut restart: Vec<String> = Vec::new();
    if new.topics != started.topics {
        restart.push("topics".into());
    }
    if new.broker_url != started.broker_url {
        restart.push("amqp.broker_url".into());
    }
    if new.domain != started.domain {
        restart.push("domain".into());
    }
    let subject = live.subject.as_deref();
    for t in &started.topics {
        for write in [false, true] {
            let before = allowed(started, subject, &t.dds_name, write);
            let now = acl_allows(
                new.acl_default_deny,
                &new.acl_rules,
                subject,
                &t.dds_name,
                write,
            );
            if now && !before {
                restart.push(format!(
                    "acl grant {} {}",
                    if write { "write" } else { "read" },
                    t.dds_name
                ));
            }
        }
    }
    if let Ok(mut a) = live.acl.write() {
        *a = (new.acl_default_deny, new.acl_rules.clone());
    }
    if let Ok(mut t) = live.tls.lock() {
        *t = tls;
    }
    log("reloaded", &[("applied", "acl, tls for the next connect")]);
    if !restart.is_empty() {
        log("reload_needs_restart", &[("changed", &restart.join(", "))]);
    }
}

// ============================================================================
// The pump.
// ============================================================================

/// Why the daemon stopped.
#[derive(Debug)]
pub struct DaemonError {
    /// Exit code (§2): 1 config, 2 connect, 3 DDS.
    pub code: u8,
    /// Message.
    pub msg: String,
}

impl core::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for DaemonError {}

/// Shuts a runtime down when dropped.
struct ShutdownOnDrop(Arc<DcpsRuntime>);

impl Drop for ShutdownOnDrop {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}

/// The type `t.type_file` describes, for the instance keys of a keyed
/// topic's writer.
fn key_type_support(
    t: &TopicMapping,
) -> Result<Arc<zerodds_dcps::serialized_type::SerializedTypeSupport>, String> {
    let bytes = std::fs::read(&t.type_file)
        .map_err(|e| format!("topics[{}].type_file '{}': {e}", t.dds_name, t.type_file))?;
    let object = zerodds_types::type_object::TypeObject::from_bytes_le(&bytes).map_err(|e| {
        format!(
            "topics[{}].type_file '{}': not a TypeObject: {e:?}",
            t.dds_name, t.type_file
        )
    })?;
    let zerodds_types::type_object::TypeObject::Complete(complete) = object else {
        return Err(format!(
            "topics[{}].type_file '{}': a MINIMAL TypeObject; the file must hold the COMPLETE one",
            t.dds_name, t.type_file
        ));
    };
    let type_name = if t.dds_type.is_empty() {
        t.dds_name.as_str()
    } else {
        t.dds_type.as_str()
    };
    let support = zerodds_dcps::serialized_type::SerializedTypeSupport::from_complete_type_objects(
        type_name,
        complete,
        Vec::new(),
    )
    .map_err(|e| format!("topics[{}].type_file '{}': {e}", t.dds_name, t.type_file))?;
    if !support.is_keyed() {
        return Err(format!(
            "topics[{}].type_file '{}': the type has no @key, the topic is keyed",
            t.dds_name, t.type_file
        ));
    }
    Ok(support)
}

fn err(code: u8, msg: impl Into<String>) -> DaemonError {
    DaemonError {
        code,
        msg: msg.into(),
    }
}

/// The `log_level` threshold: 0 trace, 1 debug, 2 info, 3 warn, 4 error.
static LOG_LEVEL: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(2);

fn level_rank(level: &str) -> u8 {
    match level {
        "trace" => 0,
        "debug" => 1,
        "warn" => 3,
        "error" => 4,
        _ => 2,
    }
}

/// The level of a log event (§8.1): what went wrong is `warn` or `error`,
/// the rest `info`.
fn event_level(event: &str) -> &'static str {
    match event {
        "encode_error" | "dds_write_failed" | "connection_lost" | "shutdown_with_unsettled" => {
            "error"
        }
        "connect_error"
        | "link_down"
        | "link_detached"
        | "attach_refused"
        | "rejected"
        | "rejected_by_broker"
        | "released_too_often"
        | "queue_full_dropped_oldest"
        | "dropped_hop_cap"
        | "acl_denied"
        | "settled_message_not_written"
        | "detach_of_unknown_link"
        | "end_of_unknown_session" => "warn",
        "dropped_own_message" => "debug",
        _ => "info",
    }
}

/// One JSON log line on stderr (§8.1), if its level reaches `log_level`.
fn log(event: &str, fields: &[(&str, &str)]) {
    let level = event_level(event);
    if level_rank(level) < LOG_LEVEL.load(Ordering::Relaxed) {
        return;
    }
    let mut line = format!("{{\"level\":\"{level}\",\"event\":\"{event}\"");
    for (k, v) in fields {
        line.push_str(&format!(
            ",\"{k}\":\"{}\"",
            v.replace('\\', "\\\\").replace('"', "\\\"")
        ));
    }
    line.push('}');
    #[allow(clippy::print_stderr)]
    {
        eprintln!("{line}");
    }
}

enum Event {
    /// A DDS sample of the topic with this index, boxed: a `UserSample` is
    /// several times the size of the other variants.
    Dds(usize, Box<UserSample>),
    /// A frame of connection `generation`.
    Amqp(u64, Inbound),
    /// Connection `generation` is gone.
    Lost(u64, String),
}

/// A message waiting for credit, or unsettled at the broker.
#[derive(Clone)]
struct Pending {
    message: Vec<u8>,
    attempts: u32,
}

/// Where a link's session stands. Every link has a session of its own, so
/// a broker that ends one (RabbitMQ does for a queue that does not exist)
/// leaves the others running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkState {
    /// No session; begun again at `retry_at`, or at once when `None`.
    Down,
    /// Begin sent, waiting for the broker's.
    Beginning,
    /// Attach sent, waiting for the broker's.
    Attaching,
    /// Attached.
    Up,
    /// End sent, waiting for the broker's.
    Ending,
}

/// A sending link: one per `out`/`bidir` topic, and the catalog's.
struct OutLink {
    /// The topic it carries; `None` for the catalog link.
    topic: Option<usize>,
    /// The channel of the link's session.
    channel: u16,
    state: LinkState,
    spec: LinkSpec,
    /// Send settled (AT_MOST_ONCE).
    settled: bool,
    remote_handle: Option<u32>,
    /// Credit the broker granted and deliveries have not used.
    credit: u32,
    delivery_count: u32,
    queue: VecDeque<Pending>,
    /// Unsettled deliveries by delivery id.
    unsettled: BTreeMap<u32, Pending>,
    cap: usize,
    /// The next delivery tag; unique per link (Part 2 §2.6.12).
    next_tag: u64,
    /// When a link the broker detached is attached again.
    retry_at: Option<Instant>,
    retry_delay: Duration,
}

/// A receiving link: one per `in`/`bidir` topic.
struct InLink {
    topic: usize,
    /// The channel of the link's session.
    channel: u16,
    state: LinkState,
    spec: LinkSpec,
    remote_handle: Option<u32>,
    writer: EntityId,
    delivery_count: u32,
    /// Credit still granted to the broker.
    outstanding: u32,
    retry_at: Option<Instant>,
    retry_delay: Duration,
}

/// How often a released or modified delivery is sent again.
const MAX_ATTEMPTS: u32 = 5;

/// How long a shutdown waits for the broker to settle what is in flight
/// (§9.2).
const DRAIN_BUDGET: Duration = Duration::from_secs(30);

fn link_spec(cfg: &DaemonConfig, t: &TopicMapping, handle: u32, sender: bool) -> LinkSpec {
    LinkSpec {
        name: format!(
            "{}/{}/{}",
            cfg.container_id,
            t.dds_name,
            if sender { "sender" } else { "receiver" }
        ),
        handle,
        sender,
        address: t.amqp_address.clone(),
        snd_settle_mode: t.snd_settle_mode,
        rcv_settle_mode: t.rcv_settle_mode,
        durable: t.durable,
        properties: vec![
            ("zerodds_topic".into(), t.dds_name.clone()),
            ("zerodds_type".into(), t.dds_type.clone()),
        ],
        desired_capabilities: vec!["dds.cdr2".into()],
    }
}

/// The catalog message (§5.2): JSON in a data section.
fn catalog_message(cfg: &DaemonConfig) -> Result<Vec<u8>, String> {
    let properties = MessageSection::Properties(AmqpExtValue::List(vec![
        AmqpExtValue::Null,
        AmqpExtValue::Null,
        AmqpExtValue::Str(cfg.catalog_address()),
        AmqpExtValue::Str("catalog".into()),
        AmqpExtValue::Null,
        AmqpExtValue::Null,
        AmqpExtValue::Symbol("application/json".into()),
    ]));
    let mut out = properties.encode().map_err(|e| format!("{e:?}"))?;
    out.extend_from_slice(
        &MessageSection::Data(cfg.catalog_json().into_bytes())
            .encode()
            .map_err(|e| format!("{e:?}"))?,
    );
    Ok(out)
}

fn bridge_uuid() -> String {
    let mut b = [0u8; 16];
    let _ = zerodds_bridge_security::tls_provider()
        .secure_random
        .fill(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0],
        b[1],
        b[2],
        b[3],
        b[4],
        b[5],
        b[6],
        b[7],
        b[8],
        b[9],
        b[10],
        b[11],
        b[12],
        b[13],
        b[14],
        b[15]
    )
}

fn tls_config(cfg: &DaemonConfig) -> Result<Option<Arc<rustls::ClientConfig>>, String> {
    if !cfg.tls_enabled {
        return Ok(None);
    }
    if cfg.tls_ca_file.is_empty() {
        return Err("amqp.tls.enabled needs amqp.tls.ca_file, the CA the broker certificate is checked against".into());
    }
    use std::path::PathBuf;
    let path = |s: &str| (!s.is_empty()).then(|| PathBuf::from(s));
    zerodds_bridge_security::connection::build_client_tls_connector(
        path(&cfg.tls_ca_file).as_deref(),
        path(&cfg.tls_cert_file).as_deref(),
        path(&cfg.tls_key_file).as_deref(),
    )
    .map(Some)
    .map_err(|e| format!("amqp.tls: {e}"))
}

/// Runs the daemon until `shutdown` is set, or, with `once`, until the
/// first connection ends.
///
/// # Errors
/// [`DaemonError`] with the exit code of §2.
pub fn run(
    cfg: &DaemonConfig,
    runtime_config: RuntimeConfig,
    shutdown: &Arc<AtomicBool>,
    metrics: &BridgeMetrics,
    once: bool,
    connect_timeout: Duration,
) -> Result<(), DaemonError> {
    run_with_reload(
        cfg,
        runtime_config,
        shutdown,
        metrics,
        once,
        connect_timeout,
        None,
    )
}

/// [`run`], reloading ACL and TLS material on `reload` (§9.2).
///
/// # Errors
/// As [`run`].
pub fn run_with_reload(
    cfg: &DaemonConfig,
    runtime_config: RuntimeConfig,
    shutdown: &Arc<AtomicBool>,
    metrics: &BridgeMetrics,
    once: bool,
    connect_timeout: Duration,
    reload: Option<Reload>,
) -> Result<(), DaemonError> {
    LOG_LEVEL.store(level_rank(&cfg.log_level), Ordering::Relaxed);
    // STOPPING=1 as soon as the stop is seen, wherever the daemon is then
    // (connecting, pumping or backing off); the drain follows.
    {
        let stop = Arc::clone(shutdown);
        std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(100));
            }
            zerodds_dcps::cli::notify_service_manager("STOPPING=1");
        });
    }
    let (host, port, url_tls) = parse_broker_url(&cfg.broker_url).map_err(|e| err(1, e))?;
    if url_tls != cfg.tls_enabled {
        return Err(err(
            1,
            format!(
                "amqp.broker_url {} and amqp.tls.enabled={} disagree; amqps:// is TLS",
                cfg.broker_url, cfg.tls_enabled
            ),
        ));
    }
    let sasl = cfg.sasl().map_err(|e| err(1, e))?;
    let mechanism = match &sasl {
        SaslCredentials::Anonymous => "ANONYMOUS",
        SaslCredentials::Plain { .. } => "PLAIN",
        SaslCredentials::ScramSha256 { .. } => "SCRAM-SHA-256",
        SaslCredentials::External => "EXTERNAL",
        SaslCredentials::XOAuth2 { .. } => "XOAUTH2",
    };
    let topic_names: Vec<String> = cfg.topics.iter().map(|t| t.dds_name.clone()).collect();
    let metrics = &metrics.with_topics(&topic_names, mechanism);
    if matches!(sasl, SaslCredentials::Plain { .. }) && !cfg.tls_enabled && !is_loopback(&host) {
        return Err(err(
            1,
            "SASL PLAIN sends the password in the clear; use amqps:// (spec §7.2)",
        ));
    }
    if matches!(sasl, SaslCredentials::Anonymous) && !is_loopback(&host) {
        return Err(err(
            1,
            "SASL ANONYMOUS is allowed only towards a loopback broker (spec §7.2)",
        ));
    }
    let tls = tls_config(cfg).map_err(|e| err(1, e))?;
    let subject = sasl.subject().map(str::to_string);
    let live = Arc::new(Live {
        acl: std::sync::RwLock::new((cfg.acl_default_deny, cfg.acl_rules.clone())),
        tls: std::sync::Mutex::new(tls.clone()),
        subject: subject.clone(),
    });
    if let Some(r) = reload {
        let (live, started, shutdown) = (Arc::clone(&live), cfg.clone(), Arc::clone(shutdown));
        std::thread::Builder::new()
            .name("amqp-reload".into())
            .spawn(move || {
                while !shutdown.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(100));
                    if r.flag.swap(false, Ordering::SeqCst) {
                        apply_reload(&live, &started, (r.load)());
                    }
                }
            })
            .map_err(|e| err(1, format!("reload thread: {e}")))?;
    }

    let prefix = zerodds_dcps::participant::bridge_guid_prefix(&cfg.container_id);
    let runtime = DcpsRuntime::start(cfg.domain, prefix, runtime_config)
        .map_err(|e| err(3, format!("DDS participant: {e:?}")))?;
    // The participant ends with this function, on every path out of it: a
    // runtime's own threads hold it, so dropping the last handle stops
    // nothing, and a caller that runs the endpoint again in the same
    // process (a test, an embedding) met the previous participant — same
    // GUID prefix, same writers — still on the bus.
    let _participant = ShutdownOnDrop(Arc::clone(&runtime));

    let (tx, rx) = mpsc::channel::<Event>();
    let mut writers: BTreeMap<usize, EntityId> = BTreeMap::new();
    for (i, t) in cfg.topics.iter().enumerate() {
        if t.direction.sends() {
            if !allowed(cfg, subject.as_deref(), &t.dds_name, false) {
                log("acl_denied", &[("topic", &t.dds_name), ("op", "read")]);
            } else {
                let (_eid, samples) = runtime
                    .register_user_reader_kind(
                        UserReaderConfig {
                            topic_name: t.dds_name.clone(),
                            type_name: t.dds_type.clone(),
                            reliable: t.reliable,
                            durability: t.durability,
                            ..Default::default()
                        },
                        t.keyed,
                    )
                    .map_err(|e| err(3, format!("DDS reader {}: {e:?}", t.dds_name)))?;
                let tx = tx.clone();
                std::thread::Builder::new()
                    .name(format!("amqp-dds-{i}"))
                    .spawn(move || {
                        while let Ok(s) = samples.recv() {
                            if tx.send(Event::Dds(i, Box::new(s))).is_err() {
                                break;
                            }
                        }
                    })
                    .map_err(|e| err(3, format!("reader thread: {e}")))?;
            }
        }
        if t.direction.receives() {
            if !allowed(cfg, subject.as_deref(), &t.dds_name, true) {
                log("acl_denied", &[("topic", &t.dds_name), ("op", "write")]);
            } else {
                let eid = runtime
                    .register_user_writer_kind(
                        UserWriterConfig {
                            topic_name: t.dds_name.clone(),
                            type_name: t.dds_type.clone(),
                            reliable: t.reliable,
                            durability: t.durability,
                            ..Default::default()
                        },
                        t.keyed,
                    )
                    .map_err(|e| err(3, format!("DDS writer {}: {e:?}", t.dds_name)))?;
                if let History::KeepLast(d) = t.history {
                    let _ = runtime.set_user_writer_history_depth(eid, d);
                }
                if t.keyed && !t.type_file.is_empty() {
                    let support = key_type_support(t).map_err(|e| err(1, e))?;
                    runtime
                        .set_user_writer_key_support(eid, support)
                        .map_err(|e| err(1, format!("topics[{}].type_file: {e:?}", t.dds_name)))?;
                }
                writers.insert(i, eid);
            }
        }
    }

    let coexistence = CoexistenceConfig::new(bridge_uuid());
    let initial_retry = Duration::from_millis(cfg.reconnect.initial_delay_ms);
    let mut outs: Vec<OutLink> = Vec::new();
    let mut ins: Vec<InLink> = Vec::new();
    let mut handle = 0u32;
    for (i, t) in cfg.topics.iter().enumerate() {
        if t.direction.sends() && allowed(cfg, subject.as_deref(), &t.dds_name, false) {
            outs.push(OutLink {
                topic: Some(i),
                channel: handle as u16,
                state: LinkState::Down,
                spec: link_spec(cfg, t, handle, true),
                settled: t.settled,
                remote_handle: None,
                credit: 0,
                delivery_count: 0,
                queue: VecDeque::new(),
                unsettled: BTreeMap::new(),
                cap: match t.history {
                    History::KeepLast(d) => d,
                    History::KeepAll => KEEP_ALL_QUEUE_CAP,
                },
                next_tag: 0,
                retry_at: None,
                retry_delay: initial_retry,
            });
            handle += 1;
        }
        if let Some(eid) = writers.get(&i) {
            ins.push(InLink {
                topic: i,
                channel: handle as u16,
                state: LinkState::Down,
                spec: link_spec(cfg, t, handle, false),
                remote_handle: None,
                writer: *eid,
                delivery_count: 0,
                outstanding: 0,
                retry_at: None,
                retry_delay: initial_retry,
            });
            handle += 1;
        }
    }
    if cfg.catalog_enabled {
        outs.push(OutLink {
            topic: None,
            channel: handle as u16,
            state: LinkState::Down,
            spec: LinkSpec {
                name: format!("{}/catalog/sender", cfg.container_id),
                handle,
                sender: true,
                address: cfg.catalog_address(),
                snd_settle_mode: 1,
                rcv_settle_mode: 0,
                durable: 0,
                properties: vec![],
                desired_capabilities: vec![],
            },
            settled: true,
            remote_handle: None,
            credit: 0,
            delivery_count: 0,
            queue: VecDeque::new(),
            unsettled: BTreeMap::new(),
            cap: 1,
            next_tag: 0,
            retry_at: None,
            retry_delay: initial_retry,
        });
    }
    let catalog = catalog_message(cfg).map_err(|e| err(1, format!("catalog: {e}")))?;
    let links = outs.len() + ins.len();
    if links > usize::from(cfg.channel_max) + 1 {
        return Err(err(
            1,
            format!(
                "{links} links need {links} sessions; amqp.channel_max {} allows {}",
                cfg.channel_max,
                usize::from(cfg.channel_max) + 1
            ),
        ));
    }

    let mut params = ConnectParams {
        addr: format!("{host}:{port}"),
        host: host.clone(),
        tls,
        sasl,
        container_id: cfg.container_id.clone(),
        hostname: Some(if cfg.hostname.is_empty() {
            host.clone()
        } else {
            cfg.hostname.clone()
        }),
        max_frame_size: cfg.max_frame_size,
        channel_max: cfg.channel_max,
        idle_time_out_ms: cfg.idle_time_out_ms,
        properties: vec![
            ("zerodds_version".into(), "1.0".into()),
            ("zerodds_role".into(), "bridge".into()),
        ],
        capabilities: vec!["AMQP_DDS_BRIDGE".into()],
        timeout: connect_timeout,
    };

    let mut delay = initial_retry;
    let mut generation = 0u64;
    let own_prefix = prefix.to_bytes();
    loop {
        if shutdown.load(Ordering::SeqCst) {
            return Ok(());
        }
        generation += 1;
        metrics.connections_total.inc();
        // The TLS material of the last reload (§9.2).
        if let Ok(t) = live.tls.lock() {
            params.tls = t.clone();
        }
        let (mut writer, mut reader, remote) = match broker::connect(&params) {
            Ok(c) => {
                if let Some(sm) = &metrics.spec {
                    sm.connections_established.inc();
                }
                // Ready once the broker connection stands (also after a
                // reconnect; a repeated READY=1 changes nothing).
                zerodds_dcps::cli::notify_service_manager("READY=1");
                c
            }
            Err(e) => {
                metrics.errors_total.inc();
                if let Some(sm) = &metrics.spec {
                    sm.connections_failed.inc();
                    if matches!(e.stage, broker::ConnectStage::Sasl) {
                        sm.sasl_failures.inc();
                    }
                }
                log(
                    "connect_error",
                    &[("broker", &params.addr), ("err", &e.to_string())],
                );
                if once {
                    // §2: 2 TCP, 4 TLS, 5 SASL, 6 open refused.
                    let code = match e.stage {
                        broker::ConnectStage::Tcp => 2,
                        broker::ConnectStage::Tls => 4,
                        broker::ConnectStage::Sasl => 5,
                        broker::ConnectStage::Open => 6,
                    };
                    return Err(err(code, format!("connect {}: {e}", params.addr)));
                }
                sleep_unless(shutdown, delay);
                delay = next_delay(delay, &cfg.reconnect);
                continue;
            }
        };
        delay = initial_retry;
        metrics.connections_active.set(1);
        log(
            "connected",
            &[
                ("broker", &params.addr),
                ("container", &remote.container_id),
            ],
        );
        {
            let tx = tx.clone();
            let gen_ = generation;
            std::thread::Builder::new()
                .name("amqp-read".into())
                .spawn(move || {
                    loop {
                        match reader.next_event() {
                            Ok(ev) => {
                                if tx.send(Event::Amqp(gen_, ev)).is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Event::Lost(gen_, e.to_string()));
                                break;
                            }
                        }
                    }
                })
                .map_err(|e| err(2, format!("reader thread: {e}")))?;
        }
        // The catalog goes out once per connection (§5.2).
        if let Some(c) = outs.iter_mut().find(|l| l.topic.is_none()) {
            c.queue.clear();
            c.queue.push_back(Pending {
                message: catalog.clone(),
                attempts: 0,
            });
        }
        if links > usize::from(remote.channel_max) + 1 {
            writer.close(Some(&AmqpError {
                condition: "amqp:resource-limit-exceeded".into(),
                description: Some("channel-max".into()),
            }));
            return Err(err(
                2,
                format!(
                    "{links} links need {links} sessions; the broker allows {}",
                    usize::from(remote.channel_max) + 1
                ),
            ));
        }
        let begun = begin_all(&mut writer, &mut outs, &mut ins);
        let result = begun.and_then(|()| {
            pump(
                cfg,
                &live,
                &runtime,
                &mut writer,
                &rx,
                generation,
                &remote,
                &mut outs,
                &mut ins,
                &coexistence,
                &own_prefix,
                metrics,
                shutdown,
            )
        });
        metrics.connections_active.set(0);
        if let (Some(sm), Err(_)) = (&metrics.spec, &result) {
            sm.connections_lost.inc();
            sm.links_sender.set(0);
            sm.links_receiver.set(0);
        }
        match result {
            Ok(()) => {
                // Shutdown: detach, end, close (§9.2).
                for l in &outs {
                    if l.state == LinkState::Up {
                        let _ = writer.detach(l.channel, l.spec.handle, None);
                    }
                }
                for l in &ins {
                    if l.state == LinkState::Up {
                        let _ = writer.detach(l.channel, l.spec.handle, None);
                    }
                }
                writer.close(None);
                log("closed", &[]);
                return Ok(());
            }
            Err(reason) => {
                metrics.errors_total.inc();
                log("connection_lost", &[("err", &reason)]);
                writer.close(Some(&AmqpError {
                    condition: "amqp:internal-error".into(),
                    description: Some(reason.clone()),
                }));
                // Unsettled deliveries go out again first (§9.3).
                for l in &mut outs {
                    let unsettled: Vec<Pending> =
                        std::mem::take(&mut l.unsettled).into_values().collect();
                    for p in unsettled.into_iter().rev() {
                        l.queue.push_front(p);
                    }
                    l.remote_handle = None;
                    l.credit = 0;
                    l.retry_at = None;
                    l.state = LinkState::Down;
                }
                for l in &mut ins {
                    l.remote_handle = None;
                    l.outstanding = 0;
                    l.retry_at = None;
                    l.state = LinkState::Down;
                }
                if once {
                    return Err(err(2, reason));
                }
                sleep_unless(shutdown, delay);
                delay = next_delay(delay, &cfg.reconnect);
            }
        }
    }
}

fn is_loopback(host: &str) -> bool {
    host == "localhost"
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|a| a.is_loopback())
}

fn next_delay(d: Duration, rc: &Reconnect) -> Duration {
    let next = (d.as_millis() as f64 * rc.factor) as u64;
    Duration::from_millis(next.min(rc.max_delay_ms).max(rc.initial_delay_ms))
}

fn sleep_unless(shutdown: &AtomicBool, d: Duration) {
    let end = Instant::now() + d;
    while !shutdown.load(Ordering::SeqCst) {
        let now = Instant::now();
        if now >= end {
            break;
        }
        std::thread::sleep((end - now).min(Duration::from_millis(100)));
    }
}

/// Begins the session of every link that has none.
fn begin_all(
    writer: &mut BrokerWriter,
    outs: &mut [OutLink],
    ins: &mut [InLink],
) -> Result<(), String> {
    for l in outs.iter_mut().filter(|l| l.state == LinkState::Down) {
        writer
            .begin(l.channel)
            .map_err(|e| format!("begin {}: {e}", l.spec.name))?;
        l.state = LinkState::Beginning;
        l.retry_at = None;
    }
    for l in ins.iter_mut().filter(|l| l.state == LinkState::Down) {
        writer
            .begin(l.channel)
            .map_err(|e| format!("begin {}: {e}", l.spec.name))?;
        l.state = LinkState::Beginning;
        l.retry_at = None;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn pump(
    cfg: &DaemonConfig,
    live: &Live,
    runtime: &Arc<DcpsRuntime>,
    writer: &mut BrokerWriter,
    rx: &mpsc::Receiver<Event>,
    generation: u64,
    remote: &broker::RemoteOpen,
    outs: &mut [OutLink],
    ins: &mut [InLink],
    coexistence: &CoexistenceConfig,
    own_prefix: &[u8; 12],
    metrics: &BridgeMetrics,
    shutdown: &AtomicBool,
) -> Result<(), String> {
    // Part 2 §2.4.5: a frame at least every half of the broker's idle
    // time-out; without one, wake up only to look at the shutdown flag and
    // the links waiting to be attached again.
    let beat = remote
        .idle_time_out_ms
        .map(|t| Duration::from_millis(u64::from(t / 2).max(1)));
    let wake = beat
        .unwrap_or(Duration::from_millis(250))
        .min(Duration::from_millis(250));
    let mut next_beat = beat.map(|b| Instant::now() + b);
    let mut drain_until: Option<Instant> = None;
    loop {
        if let Some(sm) = &metrics.spec {
            let up = |s: &LinkState| *s == LinkState::Up;
            sm.links_sender.set(
                i64::try_from(outs.iter().filter(|l| up(&l.state)).count()).unwrap_or(i64::MAX),
            );
            sm.links_receiver.set(
                i64::try_from(ins.iter().filter(|l| up(&l.state)).count()).unwrap_or(i64::MAX),
            );
        }
        if shutdown.load(Ordering::SeqCst) {
            // §9.2: stop taking DDS samples, wait for what is in flight.
            let until = *drain_until.get_or_insert_with(|| Instant::now() + DRAIN_BUDGET);
            let in_flight: usize = outs.iter().map(|l| l.unsettled.len()).sum();
            if in_flight == 0 || Instant::now() >= until {
                if in_flight > 0 {
                    metrics.errors_total.inc();
                    log(
                        "shutdown_with_unsettled",
                        &[("count", &in_flight.to_string())],
                    );
                }
                return Ok(());
            }
        }
        let now = Instant::now();
        if let (Some(at), Some(b)) = (next_beat, beat) {
            if now >= at {
                writer.heartbeat().map_err(|e| format!("heartbeat: {e}"))?;
                next_beat = Some(now + b);
            }
        }
        for l in outs.iter_mut() {
            if l.state == LinkState::Down && l.retry_at.is_some_and(|t| now >= t) {
                l.retry_at = None;
                writer
                    .begin(l.channel)
                    .map_err(|e| format!("begin {}: {e}", l.spec.name))?;
                l.state = LinkState::Beginning;
            }
        }
        for l in ins.iter_mut() {
            if l.state == LinkState::Down && l.retry_at.is_some_and(|t| now >= t) {
                l.retry_at = None;
                writer
                    .begin(l.channel)
                    .map_err(|e| format!("begin {}: {e}", l.spec.name))?;
                l.state = LinkState::Beginning;
            }
        }
        let ev = match rx.recv_timeout(wake) {
            Ok(ev) => ev,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("event channel closed".into());
            }
        };
        match ev {
            Event::Lost(g, reason) if g == generation => return Err(reason),
            Event::Lost(..) => {}
            Event::Amqp(g, _) if g != generation => {}
            Event::Amqp(_, inbound) => {
                on_inbound(
                    cfg,
                    live,
                    runtime,
                    writer,
                    inbound,
                    outs,
                    ins,
                    coexistence,
                    metrics,
                )?;
            }
            Event::Dds(..) if drain_until.is_some() => {}
            Event::Dds(topic, sample) => {
                let Some(l) = outs.iter_mut().find(|l| l.topic == Some(topic)) else {
                    continue;
                };
                let t = &cfg.topics[topic];
                // A right withdrawn by a reload: the sample stays in DDS.
                if !live.allows(&t.dds_name, false) {
                    metrics.errors_total.inc();
                    log("acl_denied", &[("topic", &t.dds_name), ("op", "read")]);
                    continue;
                }
                let out = match &*sample {
                    UserSample::Alive {
                        key_hash,
                        payload,
                        writer_guid,
                        representation,
                        big_endian,
                        source_timestamp,
                        source_sequence_number,
                        lifespan_nanos,
                        ..
                    } => {
                        if writer_guid[..12] == own_prefix[..] {
                            continue; // written by this bridge from AMQP
                        }
                        // A dispose coming back from AMQP names this
                        // instance by its key hash alone; the topic's
                        // AMQP → DDS writer records the key now.
                        if t.keyed {
                            if let Some(back) = ins.iter().find(|l| l.topic == topic) {
                                let _ = runtime.remember_user_writer_instance(
                                    back.writer,
                                    *key_hash,
                                    payload.as_ref(),
                                    *representation,
                                    *big_endian,
                                );
                            }
                        }
                        encode_message(
                            t,
                            cfg.domain,
                            coexistence,
                            &OutSample {
                                payload: payload.as_ref(),
                                representation: *representation,
                                big_endian: *big_endian,
                                key_hash: if t.keyed { *key_hash } else { None },
                                writer_guid: *writer_guid,
                                sequence: *source_sequence_number,
                                source_timestamp_ns: source_timestamp.map(|ts| {
                                    // RTPS Time_t: seconds and 2^-32
                                    // fractions, rounded to the nearest ns.
                                    let time = zerodds_dcps::time::he_timestamp_to_time(ts);
                                    u64::try_from(time.sec).unwrap_or(0) * 1_000_000_000
                                        + u64::from(time.nanosec)
                                }),
                                operation: DdsOperation::Write,
                                lifespan_ms: (*lifespan_nanos > 0)
                                    .then(|| (*lifespan_nanos / 1_000_000).max(1)),
                            },
                        )
                    }
                    UserSample::Lifecycle { key_hash, kind, .. } => {
                        let op = if matches!(
                            kind,
                            zerodds_rtps::history_cache::ChangeKind::NotAliveUnregistered
                        ) {
                            DdsOperation::Unregister
                        } else {
                            DdsOperation::Dispose
                        };
                        encode_message(
                            t,
                            cfg.domain,
                            coexistence,
                            &OutSample {
                                payload: &[],
                                representation: 1,
                                big_endian: false,
                                key_hash: Some(*key_hash),
                                writer_guid: [0; 16],
                                sequence: 0,
                                source_timestamp_ns: None,
                                operation: op,
                                lifespan_ms: None,
                            },
                        )
                    }
                    UserSample::CoherentSetEnd { .. } => continue,
                };
                match out {
                    Ok(message) => {
                        metrics.dds_samples_in_total.inc();
                        if let Some(tm) = l.topic.and_then(|i| topic_metrics(metrics, i)) {
                            tm.dds_received.inc();
                        }
                        if l.queue.len() >= l.cap {
                            l.queue.pop_front();
                            metrics.errors_total.inc();
                            log("queue_full_dropped_oldest", &[("topic", &t.dds_name)]);
                        }
                        l.queue.push_back(Pending {
                            message,
                            attempts: 0,
                        });
                        drain(writer, l, metrics)?;
                    }
                    Err(e) => {
                        metrics.errors_total.inc();
                        log("encode_error", &[("topic", &t.dds_name), ("err", &e)]);
                    }
                }
            }
        }
    }
}

/// The families of topic `index` (spec §8.2), when registered.
fn topic_metrics(
    metrics: &BridgeMetrics,
    index: usize,
) -> Option<&crate::daemon_runtime::TopicMetrics> {
    metrics.spec.as_ref().and_then(|s| s.topics.get(index))
}

/// A delivery settled with `outcome`, in either direction (spec §8.2).
fn count_outcome(metrics: &BridgeMetrics, outcome: &Outcome) {
    let Some(sm) = &metrics.spec else {
        return;
    };
    match outcome {
        Outcome::Accepted => sm.dispositions_accepted.inc(),
        Outcome::Rejected(e) => sm.rejected(e.as_ref().map(|e| e.condition.as_str())),
        Outcome::Released | Outcome::Modified { .. } => sm.dispositions_released.inc(),
        Outcome::None => {}
    }
}

/// Sends queued messages while the link has credit and the session room.
fn drain(
    writer: &mut BrokerWriter,
    l: &mut OutLink,
    metrics: &BridgeMetrics,
) -> Result<(), String> {
    while l.credit > 0 && l.state == LinkState::Up {
        let Some(p) = l.queue.pop_front() else {
            break;
        };
        l.next_tag += 1;
        let mut tag = [0u8; 16];
        tag[..8].copy_from_slice(&l.next_tag.to_be_bytes());
        match writer.transfer(l.channel, l.spec.handle, &tag, &p.message, l.settled) {
            Ok(delivery_id) => {
                l.credit -= 1;
                l.delivery_count = l.delivery_count.wrapping_add(1);
                if l.topic.is_some() {
                    metrics.dds_samples_out_total.inc();
                }
                if let Some(tm) = l.topic.and_then(|i| topic_metrics(metrics, i)) {
                    tm.transfers_out.inc();
                    tm.bytes_out.add(p.message.len() as u64);
                }
                metrics.frames_out_total.inc();
                metrics.bytes_out_total.add(p.message.len() as u64);
                if !l.settled {
                    l.unsettled.insert(delivery_id, p);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                l.queue.push_front(p);
                break;
            }
            Err(e) => {
                l.queue.push_front(p);
                return Err(format!("transfer: {e}"));
            }
        }
    }
    Ok(())
}

fn drain_all(
    writer: &mut BrokerWriter,
    outs: &mut [OutLink],
    metrics: &BridgeMetrics,
) -> Result<(), String> {
    for l in outs.iter_mut() {
        drain(writer, l, metrics)?;
    }
    Ok(())
}

/// A link whose session ended: its unsettled deliveries wait in the queue,
/// the session is begun again after the back-off.
fn out_link_down(cfg: &DaemonConfig, l: &mut OutLink, reason: &str) {
    let unsettled: Vec<Pending> = std::mem::take(&mut l.unsettled).into_values().collect();
    for p in unsettled.into_iter().rev() {
        l.queue.push_front(p);
    }
    l.remote_handle = None;
    l.credit = 0;
    l.state = LinkState::Down;
    l.retry_at = Some(Instant::now() + l.retry_delay);
    log(
        "link_down",
        &[
            ("link", &l.spec.name),
            ("err", reason),
            ("retry_ms", &l.retry_delay.as_millis().to_string()),
        ],
    );
    l.retry_delay = next_delay(l.retry_delay, &cfg.reconnect);
}

fn in_link_down(cfg: &DaemonConfig, l: &mut InLink, reason: &str) {
    l.remote_handle = None;
    l.outstanding = 0;
    l.state = LinkState::Down;
    l.retry_at = Some(Instant::now() + l.retry_delay);
    log(
        "link_down",
        &[
            ("link", &l.spec.name),
            ("err", reason),
            ("retry_ms", &l.retry_delay.as_millis().to_string()),
        ],
    );
    l.retry_delay = next_delay(l.retry_delay, &cfg.reconnect);
}

#[allow(clippy::too_many_arguments)]
fn on_inbound(
    cfg: &DaemonConfig,
    live: &Live,
    runtime: &Arc<DcpsRuntime>,
    writer: &mut BrokerWriter,
    inbound: Inbound,
    outs: &mut [OutLink],
    ins: &mut [InLink],
    coexistence: &CoexistenceConfig,
    metrics: &BridgeMetrics,
) -> Result<(), String> {
    metrics.frames_in_total.inc();
    let initial_retry = Duration::from_millis(cfg.reconnect.initial_delay_ms);
    match inbound {
        Inbound::Heartbeat => {}
        Inbound::Begin {
            channel,
            incoming_window,
            ..
        } => {
            writer.session_begun(channel, incoming_window);
            if let Some(l) = outs.iter_mut().find(|l| l.channel == channel) {
                writer
                    .attach(channel, &l.spec)
                    .map_err(|e| format!("attach {}: {e}", l.spec.name))?;
                l.state = LinkState::Attaching;
            } else if let Some(l) = ins.iter_mut().find(|l| l.channel == channel) {
                writer
                    .attach(channel, &l.spec)
                    .map_err(|e| format!("attach {}: {e}", l.spec.name))?;
                l.state = LinkState::Attaching;
            } else {
                return Err(format!(
                    "the broker began a session on channel {channel} nobody asked for"
                ));
            }
        }
        Inbound::Attach {
            channel,
            name,
            remote_handle,
            has_node,
            initial_delivery_count,
        } => {
            if let Some(l) = outs
                .iter_mut()
                .find(|l| l.channel == channel && l.spec.name == name)
            {
                // Without a node the detach with the reason follows under
                // this handle; no credit comes, nothing is sent.
                l.remote_handle = Some(remote_handle);
                if has_node {
                    l.state = LinkState::Up;
                    l.retry_delay = initial_retry;
                    log("attached", &[("link", &name)]);
                } else {
                    log(
                        "attach_refused",
                        &[("link", &name), ("address", &l.spec.address)],
                    );
                }
            } else if let Some(l) = ins
                .iter_mut()
                .find(|l| l.channel == channel && l.spec.name == name)
            {
                l.remote_handle = Some(remote_handle);
                if has_node {
                    let credit = cfg.topics[l.topic].credit;
                    l.state = LinkState::Up;
                    l.retry_delay = initial_retry;
                    l.delivery_count = initial_delivery_count.unwrap_or(0);
                    l.outstanding = credit;
                    writer
                        .flow(channel, l.spec.handle, l.delivery_count, credit)
                        .map_err(|e| format!("flow: {e}"))?;
                    if let Some(sm) = &metrics.spec {
                        sm.credit_topup.inc();
                    }
                    log("attached", &[("link", &name)]);
                } else {
                    log(
                        "attach_refused",
                        &[("link", &name), ("address", &l.spec.address)],
                    );
                }
            } else {
                return Err(format!("the broker attached an unknown link {name}"));
            }
        }
        Inbound::Flow {
            channel,
            next_incoming_id,
            incoming_window,
            remote_handle,
            delivery_count,
            link_credit,
            ..
        } => {
            writer.apply_session_flow(channel, next_incoming_id, incoming_window);
            if let (Some(h), Some(credit)) = (remote_handle, link_credit) {
                if let Some(l) = outs
                    .iter_mut()
                    .find(|l| l.channel == channel && l.remote_handle == Some(h))
                {
                    // Part 2 §2.6.7: credit = receiver's delivery-count +
                    // link-credit - sender's delivery-count.
                    let theirs = delivery_count.unwrap_or(l.delivery_count);
                    l.credit = theirs.wrapping_add(credit).wrapping_sub(l.delivery_count);
                }
            }
            drain_all(writer, outs, metrics)?;
        }
        Inbound::Transfer {
            channel,
            remote_handle,
            delivery_id,
            settled,
            message,
        } => {
            let Some(l) = ins
                .iter_mut()
                .find(|l| l.channel == channel && l.remote_handle == Some(remote_handle))
            else {
                return Err(format!(
                    "a transfer on unknown link handle {remote_handle} of session {channel}"
                ));
            };
            let t = &cfg.topics[l.topic];
            l.delivery_count = l.delivery_count.wrapping_add(1);
            l.outstanding = l.outstanding.saturating_sub(1);
            metrics.bytes_in_total.add(message.len() as u64);
            if let Some(tm) = topic_metrics(metrics, l.topic) {
                tm.transfers_in.inc();
                tm.bytes_in.add(message.len() as u64);
            }
            let outcome = if live.allows(&t.dds_name, true) {
                deliver(runtime, l.writer, t, &message, coexistence, metrics)
            } else {
                // A right withdrawn by a reload.
                log("acl_denied", &[("topic", &t.dds_name), ("op", "write")]);
                Outcome::Rejected(Some(AmqpError {
                    condition: "amqp:unauthorized-access".into(),
                    description: Some(format!("acl: no write on {}", t.dds_name)),
                }))
            };
            if let (Some(tm), Outcome::Accepted) = (topic_metrics(metrics, l.topic), &outcome) {
                tm.dds_published.inc();
            }
            if !settled {
                count_outcome(metrics, &outcome);
                writer
                    .disposition(channel, delivery_id, delivery_id, &outcome)
                    .map_err(|e| format!("disposition: {e}"))?;
            } else if !matches!(outcome, Outcome::Accepted) {
                log("settled_message_not_written", &[("topic", &t.dds_name)]);
            }
            if l.outstanding <= t.credit / 2 {
                l.outstanding = t.credit;
                writer
                    .flow(channel, l.spec.handle, l.delivery_count, t.credit)
                    .map_err(|e| format!("flow: {e}"))?;
                if let Some(sm) = &metrics.spec {
                    sm.credit_topup.inc();
                }
            }
        }
        Inbound::Disposition {
            channel,
            role_receiver,
            first,
            last,
            outcome,
            ..
        } => {
            if !role_receiver {
                return Ok(());
            }
            if let Some(l) = outs.iter_mut().find(|l| l.channel == channel) {
                let ids: Vec<u32> = l.unsettled.range(first..=last).map(|(id, _)| *id).collect();
                let name = l.spec.name.clone();
                for id in ids {
                    let Some(mut p) = l.unsettled.remove(&id) else {
                        continue;
                    };
                    count_outcome(metrics, &outcome);
                    match &outcome {
                        Outcome::Accepted | Outcome::None => {}
                        Outcome::Rejected(e) => {
                            metrics.errors_total.inc();
                            let reason = e
                                .as_ref()
                                .map_or_else(|| "no reason".to_string(), ToString::to_string);
                            log("rejected_by_broker", &[("link", &name), ("err", &reason)]);
                        }
                        Outcome::Released | Outcome::Modified { .. } => {
                            p.attempts += 1;
                            if p.attempts < MAX_ATTEMPTS {
                                l.queue.push_front(p);
                            } else {
                                metrics.errors_total.inc();
                                log("released_too_often", &[("link", &name)]);
                            }
                        }
                    }
                }
            }
            drain_all(writer, outs, metrics)?;
        }
        Inbound::Detach {
            channel,
            remote_handle,
            error,
            ..
        } => {
            // The broker gave up one link. Answer the detach (Part 2
            // §2.6.5) and end its session; the session is begun again once
            // the broker's end arrives, after the back-off.
            let reason = error.map_or_else(|| "no reason".to_string(), |e| e.to_string());
            let handle = if let Some(l) = outs.iter_mut().find(|l| l.channel == channel) {
                l.state = LinkState::Ending;
                l.spec.handle
            } else if let Some(l) = ins.iter_mut().find(|l| l.channel == channel) {
                l.state = LinkState::Ending;
                l.spec.handle
            } else {
                log(
                    "detach_of_unknown_link",
                    &[("handle", &remote_handle.to_string()), ("err", &reason)],
                );
                return Ok(());
            };
            log(
                "link_detached",
                &[("channel", &channel.to_string()), ("err", &reason)],
            );
            writer
                .detach(channel, handle, None)
                .map_err(|e| format!("detach: {e}"))?;
            writer.end(channel, None).map_err(|e| format!("end: {e}"))?;
        }
        Inbound::End { channel, error } => {
            let reason = error.map_or_else(|| "no reason".to_string(), |e| e.to_string());
            if let Some(l) = outs.iter_mut().find(|l| l.channel == channel) {
                if l.state != LinkState::Ending {
                    // The broker ended it: answer with ours.
                    writer.end(channel, None).map_err(|e| format!("end: {e}"))?;
                }
                out_link_down(cfg, l, &reason);
            } else if let Some(l) = ins.iter_mut().find(|l| l.channel == channel) {
                if l.state != LinkState::Ending {
                    writer.end(channel, None).map_err(|e| format!("end: {e}"))?;
                }
                in_link_down(cfg, l, &reason);
            } else {
                log(
                    "end_of_unknown_session",
                    &[("channel", &channel.to_string()), ("err", &reason)],
                );
            }
        }
        Inbound::Close(e) => {
            return Err(format!(
                "the broker closed the connection: {}",
                e.map_or_else(|| "no reason".into(), |e| e.to_string())
            ));
        }
    }
    Ok(())
}

/// Writes one message as a DDS sample; the outcome settles it (§4.4).
fn deliver(
    runtime: &Arc<DcpsRuntime>,
    eid: EntityId,
    t: &TopicMapping,
    message: &[u8],
    coexistence: &CoexistenceConfig,
    metrics: &BridgeMetrics,
) -> Outcome {
    let rejected = |condition: &str, description: String| {
        metrics.errors_total.inc();
        log("rejected", &[("topic", &t.dds_name), ("err", &description)]);
        Outcome::Rejected(Some(AmqpError {
            condition: condition.into(),
            description: Some(description),
        }))
    };
    let msg = match decode_message(message) {
        Ok(m) => m,
        Err(e) => return rejected("amqp:decode-error", e),
    };
    match inspect_inbound(coexistence, &msg.application_properties) {
        InboundDecision::Forward => {}
        // Sent by this bridge (or past the hop cap): consumed, not written,
        // and said so (DDS-AMQP 1.0 §8.4).
        InboundDecision::DropLoop => {
            log("dropped_own_message", &[("topic", &t.dds_name)]);
            return Outcome::Accepted;
        }
        InboundDecision::DropHopCap => {
            metrics.errors_total.inc();
            log("dropped_hop_cap", &[("topic", &t.dds_name)]);
            return Outcome::Accepted;
        }
    }
    match msg.content_type.as_deref() {
        None | Some(CONTENT_TYPE_XCDR2) | Some(CONTENT_TYPE_XCDR1) => {}
        Some(other) => {
            return rejected(
                "amqp:decode-error",
                format!("content-type {other}; the bridge writes {CONTENT_TYPE_XCDR2} bodies"),
            );
        }
    }
    let key = if t.keyed { msg.instance_handle } else { None };
    let result = match msg.operation {
        DdsOperation::Dispose | DdsOperation::Unregister => {
            let Some(key) = key else {
                return rejected(
                    "amqp:decode-error",
                    "dispose/unregister without dds:instance-handle".into(),
                );
            };
            let bits = if msg.operation == DdsOperation::Dispose {
                zerodds_rtps::inline_qos::status_info::DISPOSED
            } else {
                zerodds_rtps::inline_qos::status_info::UNREGISTERED
            };
            // The DDS-AMQP 1.0 mapping's dispose/unregister message
            // carries only `dds:instance-handle` (16 byte,
            // `msg.instance_handle` above), so no key holder is passed.
            // With the topic's `type_file` the writer takes the key it
            // recorded — from a sample this endpoint wrote to DDS or one it
            // carried to AMQP — or reads it from a key hash that spells it
            // out, and the marker carries the serializedKey. Without a
            // type the marker has the hash alone and the writer reports
            // that once.
            runtime.write_user_lifecycle(eid, key, None, bits, None)
        }
        DdsOperation::Register => return Outcome::Accepted,
        DdsOperation::Write => {
            let (rep, be, payload) = match split_encapsulation(&msg.body) {
                Ok(v) => v,
                Err(e) => return rejected("amqp:decode-error", e),
            };
            use zerodds_rtps::publication_data::data_representation as dr;
            let offer = if rep == 0 { dr::XCDR } else { dr::XCDR2 };
            let _ = runtime.set_user_writer_data_rep_override(eid, Some(vec![offer]));
            let _ = runtime.set_user_writer_byte_order_override(eid, be);
            match key {
                Some(k) => runtime.write_user_sample_keyed(eid, payload, k, None),
                None => runtime.write_user_sample_borrowed(eid, payload),
            }
        }
    };
    match result {
        Ok(()) => Outcome::Accepted,
        Err(e) => {
            metrics.errors_total.inc();
            log(
                "dds_write_failed",
                &[("topic", &t.dds_name), ("err", &format!("{e:?}"))],
            );
            Outcome::Released
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn load(yaml: &str) -> Result<DaemonConfig, String> {
        let mut c = zerodds_config::Config::default();
        c.push(
            zerodds_config::parse_str(yaml, "<string>").map_err(|e| e.to_string())?,
            "<string>".into(),
        )
        .map_err(|e| e.to_string())?;
        DaemonConfig::from_config(Some(&c)).map_err(|e| e.to_string())
    }

    fn topic() -> TopicMapping {
        load("topics:\n  - dds_name: T\n    amqp_address: /queues/t\n    keyed: true\n")
            .unwrap()
            .topics
            .remove(0)
    }

    /// The spec §3 example, with its paths, loads as written.
    #[test]
    fn the_spec_example_loads() {
        let yaml = "domain: 0\nlog_level: info\namqp:\n  broker_url: \"amqps://rabbitmq.example.com:5671\"\n  container_id: \"zerodds-bridge-prod-01\"\n  channel_max: 256\n  max_frame_size: 65536\n  idle_time_out_ms: 30000\n  hostname: \"rabbitmq.example.com\"\n  sasl:\n    mechanism: \"SCRAM-SHA-256\"\n    username: \"u\"\n    password: \"p\"\n  tls:\n    enabled: true\n    ca_file: \"/etc/zerodds/amqp-ca.pem\"\n    cert_file: \"/etc/zerodds/amqp-client.pem\"\n    key_file: \"/etc/zerodds/amqp-client.key\"\n    verify_hostname: true\n    alpn: [\"amqp\"]\n  reconnect:\n    initial_delay_ms: 500\n    max_delay_ms: 30000\n    factor: 2.0\ntopics:\n  - dds_name: \"Chat::Message\"\n    dds_type: \"Chat::Message\"\n    amqp_address: \"topic://chat/message\"\n    direction: \"bidir\"\n    sender:\n      settled: false\n      durable: 1\n      rcv_settle_mode: 0\n      snd_settle_mode: 0\n    receiver:\n      credit: 256\n    qos:\n      reliability: \"reliable\"\n      durability: \"volatile\"\n      history: { kind: \"keep_last\", depth: 10 }\n  - dds_name: \"Sensor::Reading\"\n    amqp_address: \"queue://sensors/reading\"\n    direction: \"out\"\n    sender:\n      settled: true\n    qos:\n      reliability: \"best_effort\"\nacl:\n  default_deny: false\nmetrics:\n  enabled: true\n  listen: \"127.0.0.1:9093\"\n  path: \"/metrics\"\n";
        let c = load(yaml).unwrap();
        assert_eq!(c.topics.len(), 2);
        assert_eq!(c.topics[0].history, History::KeepLast(10));
        assert_eq!(c.topics[0].durable, 1);
        assert!(!c.topics[0].settled);
        assert!(c.topics[1].settled, "BEST_EFFORT is AT_MOST_ONCE");
        assert_eq!(c.topics[1].dds_type, "Sensor::Reading");
        assert!((c.reconnect.factor - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn what_the_daemon_would_not_honour_is_refused() {
        for (yaml, needle) in [
            ("amqp:\n  tls:\n    alpn: [\"h2\"]\n", "amqp.tls.alpn"),
            ("metrics:\n  path: /stats\n", "metrics.path"),
            ("amqp:\n  max_frame_size: 100\n", "at least 512"),
            (
                "topics:\n  - dds_name: T\n    amqp_address: a\n    qos:\n      history: { kind: keep_all, depth: 3 }\n",
                "KEEP_ALL has no depth",
            ),
            (
                "topics:\n  - dds_name: T\n    amqp_address: a\n  - dds_name: T\n    amqp_address: b\n",
                "mapped twice",
            ),
            (
                "amqp:\n  broker_url: \"amqp://u:p@h\"\n",
                "user information",
            ),
            (
                "topics:\n  - dds_name: T\n    amqp_address: a\n    sender:\n      snd_settle_mode: 3\n",
                "snd_settle_mode",
            ),
        ] {
            let e = load(yaml).expect_err(yaml);
            assert!(e.contains(needle), "{yaml}: {e}");
        }
    }

    /// A sample goes out as the §4.3 message and comes back as the same
    /// body, key and operation.
    #[test]
    fn a_sample_round_trips_through_the_message_form() {
        let t = topic();
        let coex = CoexistenceConfig::new("11111111-2222-4333-8444-555555555555".into());
        let key = [9u8; 16];
        let m = encode_message(
            &t,
            3,
            &coex,
            &OutSample {
                payload: &[1, 2, 3, 4],
                representation: 1,
                big_endian: false,
                key_hash: Some(key),
                writer_guid: [7; 16],
                sequence: 42,
                source_timestamp_ns: Some(1_700_000_000_123_456_789),
                operation: DdsOperation::Write,
                lifespan_ms: None,
            },
        )
        .unwrap();
        let d = decode_message(&m).unwrap();
        assert_eq!(d.content_type.as_deref(), Some(CONTENT_TYPE_XCDR2));
        assert_eq!(d.body, vec![0x00, 0x07, 0, 0, 1, 2, 3, 4]);
        assert_eq!(d.instance_handle, Some(key));
        assert_eq!(d.operation, DdsOperation::Write);
        assert_eq!(
            split_encapsulation(&d.body).unwrap(),
            (1, false, &[1u8, 2, 3, 4][..])
        );
        // This bridge's own stamp: a receiver of the same address drops it.
        assert_eq!(
            inspect_inbound(&coex, &d.application_properties),
            InboundDecision::DropLoop
        );
        let other = CoexistenceConfig::new("99999999-2222-4333-8444-555555555555".into());
        assert_eq!(
            inspect_inbound(&other, &d.application_properties),
            InboundDecision::Forward
        );
    }

    #[test]
    fn an_xcdr1_sample_keeps_its_header_and_says_so() {
        let t = topic();
        let coex = CoexistenceConfig::new("x".into());
        let m = encode_message(
            &t,
            0,
            &coex,
            &OutSample {
                payload: &[5],
                representation: 0,
                big_endian: true,
                key_hash: None,
                writer_guid: [1; 16],
                sequence: 1,
                source_timestamp_ns: None,
                operation: DdsOperation::Write,
                lifespan_ms: None,
            },
        )
        .unwrap();
        let d = decode_message(&m).unwrap();
        assert_eq!(d.content_type.as_deref(), Some(CONTENT_TYPE_XCDR1));
        assert_eq!(d.body[..2], [0x00, 0x00], "CDR_BE");
    }

    #[test]
    fn broker_urls() {
        assert_eq!(
            parse_broker_url("amqp://h").unwrap(),
            ("h".into(), 5672, false)
        );
        assert_eq!(
            parse_broker_url("amqps://h:1").unwrap(),
            ("h".into(), 1, true)
        );
        assert_eq!(
            parse_broker_url("amqp://[::1]:9").unwrap(),
            ("::1".into(), 9, false)
        );
        assert!(parse_broker_url("http://h").is_err());
    }

    /// The shipped example, through the same reader.
    const EXAMPLE_YAML: &str =
        include_str!("../../../packaging/linux/configs/amqp-bridged.yaml.example");

    #[test]
    fn the_shipped_example_reads_as_it_says() {
        let c = load(EXAMPLE_YAML).unwrap();
        assert_eq!(c.broker_url, "amqp://localhost:5672");
        assert_eq!(c.channel_max, 128, "not the default 256");
        assert_eq!(c.sasl_mechanism, "PLAIN");
        assert_eq!(c.acl_rules["OrderEvent"].0, vec!["zerodds".to_string()]);
        assert_eq!(c.metrics_listen, "127.0.0.1:9094");
        let t = &c.topics[0];
        assert_eq!(t.dds_name, "OrderEvent");
        assert!(t.keyed);
        assert_eq!(t.direction, Direction::Out);
        assert_eq!(t.history, History::KeepLast(10));
        assert_eq!(t.durable, 1);
        assert_eq!(t.credit, 256);
    }

    #[test]
    fn values_the_shared_parser_reads_reach_the_daemon() {
        let c = load(
            "amqp:\n  broker_url: \"amqp://h:1\"   # the broker\n  sasl:\n    password: \"${ZERODDS_UNSET_PROBE_a41f:-p#ss}\"\n",
        )
        .unwrap();
        assert_eq!(c.broker_url, "amqp://h:1");
        assert_eq!(c.sasl_password, "p#ss");
        for (yaml, needle) in [
            ("amqp:\n  channel_max: lots\n", "amqp.channel_max"),
            ("amqp:\n  tls:\n    enabled: yes\n", "amqp.tls.enabled"),
            (
                "amqp:\n  sasl:\n    mechanizm: PLAIN\n",
                "amqp.sasl.mechanizm",
            ),
            (
                "topics:\n  - dds_name: T\n    amqp_adress: q\n",
                "topics[0].amqp_adress",
            ),
            ("topics:\n  - just-a-name\n", "topics[0]"),
            (
                "amqp:\n  tls:\n    verify_hostname: false\n",
                "always checked",
            ),
        ] {
            let e = load(yaml).expect_err(yaml);
            assert!(e.contains(needle), "{yaml}: {e}");
        }
    }

    #[test]
    fn every_event_has_a_level_and_the_threshold_orders_them() {
        assert_eq!(event_level("connected"), "info");
        assert_eq!(event_level("link_down"), "warn");
        assert_eq!(event_level("connection_lost"), "error");
        assert!(level_rank("warn") > level_rank("info"));
        assert!(level_rank("error") > level_rank("warn"));
        assert_eq!(level_rank("nonsense"), level_rank("info"));
    }
}
