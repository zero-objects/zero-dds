// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The configuration of the daemon (spec §3): the keys, their defaults,
//! and the reader over the layered sources of `zerodds-config`.

use std::collections::BTreeMap;
use std::string::{String, ToString};
use std::vec::Vec;

use zerodds_config::ConfigError;
use zerodds_config::read::{Section, first_origin};

use crate::broker::SaslCredentials;

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
    /// For a keyed topic bridged DDS → AMQP the reader computes with it the
    /// KeyHash of a sample whose writer leaves `PID_KEY_HASH` out (Cyclone)
    /// and reads the key of a dispose from its `serializedKey`.
    pub type_file: String,
    /// The AMQP node address.
    pub amqp_address: String,
    /// Direction.
    pub direction: Direction,
    /// `sender.settled`: settle on sending (AT_MOST_ONCE).
    pub settled: bool,
    /// `sender.durable`: terminus durability 0/1/2 of both links of the
    /// topic; `None` when not set (see [`TopicMapping::terminus_durable`]).
    pub durable: Option<u32>,
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
    /// `qos.deadline_ms`: requested by the DDS reader, offered by the DDS
    /// writer (§6); `None` for infinite.
    pub deadline_ms: Option<u32>,
    /// `qos.liveliness_lease_ms`: the lease of the DDS endpoints (§6,
    /// AUTOMATIC); `None` for infinite.
    pub liveliness_lease_ms: Option<u32>,
    /// `qos.partition`: the partitions of the DDS endpoints (§6); empty for
    /// the default partition.
    pub partition: Vec<String>,
}

impl TopicMapping {
    /// The terminus `durable` of the topic's sender link (`sender`, its
    /// target) or receiver link (its source): `sender.durable` when set.
    /// Without it (§6) the target of the sender link asks for a durable
    /// node (1, configuration) when the durability is not VOLATILE, so the
    /// durable messages of the daemon go to a durable node; the source of
    /// the receiver link, and every terminus of a VOLATILE topic, stays 0.
    #[must_use]
    pub fn terminus_durable(&self, sender: bool) -> u32 {
        self.durable.unwrap_or(
            if sender && self.durability != zerodds_qos::DurabilityKind::Volatile {
                1
            } else {
                0
            },
        )
    }
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
    /// `amqp.cbs.enabled`: authorize the links with `put-token` on `$cbs`
    /// (§7.2, Azure Service Bus).
    pub cbs_enabled: bool,
    /// `amqp.cbs.key_name`: the name of the shared access key.
    pub cbs_key_name: String,
    /// `amqp.cbs.secret`: the shared access key.
    pub cbs_secret: String,
    /// `amqp.cbs.ttl_s`: the lifetime of a token; renewed at four
    /// fifths of it.
    pub cbs_ttl_s: u32,
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
    /// §1 L6: the participants beside the one of `domain`, each with a
    /// broker connection of its own.
    pub tenants: Vec<TenantConfig>,
    /// §9.2: how long a stop waits for the broker to settle the deliveries
    /// in flight; 30 s. Not a key of the configuration: the specification
    /// fixes the bound, an embedding (a test) may shorten it.
    pub drain_budget: std::time::Duration,
}

/// A `tenants:` entry (§1 L6): a DomainParticipant of its own on `domain`
/// with its topics, and a connection of its own to the broker of
/// `amqp.broker_url`, bound to the tenant by its container id, its
/// `open.hostname` (a virtual host) and its SASL identity or TLS client
/// certificate. What the entry does not set is the daemon's.
#[derive(Debug, Clone, PartialEq)]
pub struct TenantConfig {
    /// Its name, in the log lines and the `tenant` label of the metrics.
    pub name: String,
    /// Its DDS domain.
    pub domain: i32,
    /// `amqp.container_id` of its connection; it also seeds the GUID
    /// prefix of its participant.
    pub container_id: String,
    /// `amqp.hostname`.
    pub hostname: Option<String>,
    /// `amqp.sasl.mechanism`.
    pub sasl_mechanism: Option<String>,
    /// `amqp.sasl.username`.
    pub sasl_username: Option<String>,
    /// `amqp.sasl.password`.
    pub sasl_password: Option<String>,
    /// `amqp.tls.cert_file`.
    pub tls_cert_file: Option<String>,
    /// `amqp.tls.key_file`.
    pub tls_key_file: Option<String>,
    /// `catalog.address`.
    pub catalog_address: Option<String>,
    /// Its topics.
    pub topics: Vec<TopicMapping>,
}

/// `zerodds-bridge-<host>` (§2): the host name (`gethostname`, see
/// [`zerodds_dcps::participant::host_name`]) with its dots as hyphens and at
/// most 32 characters; `zerodds-bridge-pid<pid>` only on a host without a
/// name. The container id also seeds the participant's GUID prefix, so two
/// daemons on one host need two container ids.
fn default_container_id() -> String {
    container_id_for(zerodds_dcps::participant::host_name().as_deref())
}

fn container_id_for(host: Option<&str>) -> String {
    let host = host
        .map(|h| h.replace('.', "-").chars().take(32).collect::<String>())
        .filter(|h| !h.is_empty())
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
            cbs_enabled: false,
            cbs_key_name: String::new(),
            cbs_secret: String::new(),
            cbs_ttl_s: 3600,
            acl_default_deny: false,
            acl_rules: BTreeMap::new(),
            metrics_enabled: false,
            metrics_listen: String::new(),
            catalog_enabled: true,
            catalog_address: String::new(),
            topics: Vec::new(),
            tenants: Vec::new(),
            drain_budget: DRAIN_BUDGET,
        }
    }
}

/// The bound of the drain at a stop (§9.2).
pub const DRAIN_BUDGET: std::time::Duration = std::time::Duration::from_secs(30);

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
    "amqp.cbs.enabled",
    "amqp.cbs.key_name",
    "amqp.cbs.secret",
    "amqp.cbs.ttl_s",
    "acl.default_deny",
    "acl.rules",
    "metrics.enabled",
    "metrics.listen",
    "metrics.path",
    "catalog.enabled",
    "catalog.address",
    "topics",
    "tenants",
];

/// Whether `path` is a key [`DaemonConfig::from_config`] reads.
#[must_use]
pub fn is_key(path: &str) -> bool {
    KEYS.contains(&path)
}

/// The daemon's own environment names (§2).
pub const ENV: &[(&str, &str)] = &[
    ("AMQP_PASSWORD", "amqp.sasl.password"),
    ("AMQP_CBS_SECRET", "amqp.cbs.secret"),
];

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
            "cbs",
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
            &[
                "PLAIN",
                "SCRAM-SHA-256",
                "SCRAM-SHA-512",
                "EXTERNAL",
                "ANONYMOUS",
                "XOAUTH2",
            ],
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
        if let Some((v, origin)) = tls.scalar("verify_hostname")?
            && v != "true"
        {
            return Err(invalid(
                "amqp.tls.verify_hostname",
                origin.clone(),
                "the broker certificate is always checked against the host; \
                     connect by the name the certificate carries",
            ));
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
        let cbs = amqp.section("cbs")?;
        cbs.only(&["enabled", "key_name", "secret", "ttl_s"])?;
        if let Some(v) = cbs.boolean("enabled")? {
            cfg.cbs_enabled = v;
        }
        if let Some(v) = cbs.string("key_name")? {
            cfg.cbs_key_name = v;
        }
        if let Some(v) = cbs.string("secret")? {
            cfg.cbs_secret = v;
        }
        if let Some((v, origin)) = cbs.scalar("ttl_s")? {
            cfg.cbs_ttl_s = v
                .parse::<u32>()
                .ok()
                .filter(|s| (10..=86_400).contains(s))
                .ok_or_else(|| invalid("amqp.cbs.ttl_s", origin.clone(), "seconds, 10 to 86400"))?;
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
        if let Some((v, origin)) = metrics.scalar("path")?
            && v != "/metrics"
        {
            return Err(invalid(
                "metrics.path",
                origin.clone(),
                "the admin endpoint serves /metrics, /catalog and /healthz",
            ));
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
        check_topics("topics", &cfg.topics)?;
        // One tenant per name; one participant and one connection per
        // container id, the daemon's own included: the container id seeds
        // the GUID prefix (§2). The second entry is refused with its path
        // and the place it came from.
        let mut names = std::collections::BTreeSet::new();
        let mut ids = std::collections::BTreeSet::from([cfg.container_id.clone()]);
        for (path, item) in root.items("tenants")? {
            let e = Section::of(Some(item), path.clone())?;
            e.only(&["name", "domain", "amqp", "catalog", "topics"])?;
            let name = e.string("name")?.filter(|n| !n.is_empty()).ok_or_else(|| {
                invalid(
                    format!("{path}.name"),
                    first_origin(item),
                    "a tenant needs a name",
                )
            })?;
            let domain = e
                .parse::<i32>("domain", "a DDS domain id")?
                .ok_or_else(|| {
                    invalid(
                        format!("{path}.domain"),
                        first_origin(item),
                        "a tenant needs its domain",
                    )
                })?;
            let ta = e.section("amqp")?;
            ta.only(&["container_id", "hostname", "sasl", "tls"])?;
            let container_id = ta
                .string("container_id")?
                .filter(|c| !c.is_empty())
                .ok_or_else(|| {
                    invalid(
                        format!("{path}.amqp.container_id"),
                        first_origin(item),
                        "a tenant needs a container id of its own",
                    )
                })?;
            let origin_of = |s: &Section<'_>, key: &str| {
                s.node(key).map_or_else(|| first_origin(item), first_origin)
            };
            if !names.insert(name.clone()) {
                return Err(invalid(
                    format!("{path}.name"),
                    origin_of(&e, "name"),
                    format!("the tenant {name} is named twice"),
                ));
            }
            if !ids.insert(container_id.clone()) {
                return Err(invalid(
                    format!("{path}.amqp.container_id"),
                    origin_of(&ta, "container_id"),
                    format!(
                        "the tenant {name} has the container id {container_id}, which another participant of the daemon has"
                    ),
                ));
            }
            let tsasl = ta.section("sasl")?;
            tsasl.only(&["mechanism", "username", "password"])?;
            let ttls = ta.section("tls")?;
            ttls.only(&["cert_file", "key_file"])?;
            let tcat = e.section("catalog")?;
            tcat.only(&["address"])?;
            let mut topics = Vec::new();
            for (tpath, titem) in e.items("topics")? {
                topics.push(read_topic(&tpath, titem)?);
            }
            if topics.is_empty() {
                return Err(invalid(
                    format!("{path}.topics"),
                    first_origin(item),
                    "a tenant needs its topics",
                ));
            }
            check_topics(&format!("{path}.topics"), &topics)?;
            cfg.tenants.push(TenantConfig {
                name,
                domain,
                container_id,
                hostname: ta.string("hostname")?,
                sasl_mechanism: tsasl.choice(
                    "mechanism",
                    &[
                        "PLAIN",
                        "SCRAM-SHA-256",
                        "SCRAM-SHA-512",
                        "EXTERNAL",
                        "ANONYMOUS",
                        "XOAUTH2",
                    ],
                )?,
                sasl_username: tsasl.string("username")?,
                sasl_password: tsasl.string("password")?,
                tls_cert_file: ttls.string("cert_file")?,
                tls_key_file: ttls.string("key_file")?,
                catalog_address: tcat.string("address")?,
                topics,
            });
        }
        Ok(cfg)
    }

    /// The configuration of tenant `t`'s participant and connection: this
    /// one with the tenant's domain, connection identity and topics.
    #[must_use]
    pub fn of_tenant(&self, t: &TenantConfig) -> Self {
        let or = |v: &Option<String>, d: &String| v.clone().unwrap_or_else(|| d.clone());
        Self {
            domain: t.domain,
            container_id: t.container_id.clone(),
            hostname: or(&t.hostname, &self.hostname),
            sasl_mechanism: or(&t.sasl_mechanism, &self.sasl_mechanism),
            sasl_username: or(&t.sasl_username, &self.sasl_username),
            sasl_password: or(&t.sasl_password, &self.sasl_password),
            tls_cert_file: or(&t.tls_cert_file, &self.tls_cert_file),
            tls_key_file: or(&t.tls_key_file, &self.tls_key_file),
            // The daemon's catalog address names its own container; a
            // tenant without one gets the default of its container id.
            catalog_address: t.catalog_address.clone().unwrap_or_default(),
            topics: t.topics.clone(),
            tenants: Vec::new(),
            ..self.clone()
        }
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
            "SCRAM-SHA-512" => SaslCredentials::ScramSha512 {
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

/// A DDS topic is mapped once per participant.
fn check_topics(path: &str, topics: &[TopicMapping]) -> Result<(), ConfigError> {
    let mut names = std::collections::BTreeSet::new();
    for t in topics {
        if !names.insert(t.dds_name.as_str()) {
            return Err(invalid(
                path,
                zerodds_config::Origin::Default,
                format!("topic {} is mapped twice", t.dds_name),
            ));
        }
    }
    Ok(())
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
        durable: None,
        snd_settle_mode: 0,
        rcv_settle_mode: 0,
        credit: 256,
        reliable: true,
        durability: zerodds_qos::DurabilityKind::Volatile,
        history: History::KeepLast(1),
        deadline_ms: None,
        liveliness_lease_ms: None,
        partition: Vec::new(),
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
        // The reader and the writer of a keyed topic read the type (§3); on
        // an unkeyed topic the key would be read and have no effect.
        if !t.type_file.is_empty() && !t.keyed {
            return Err(invalid(
                entry.path_of("type_file"),
                entry
                    .node("type_file")
                    .map_or_else(|| first_origin(item), first_origin),
                "only a keyed topic reads a type_file",
            ));
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
            t.durable = Some(v);
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
        qos.only(&[
            "reliability",
            "durability",
            "history",
            "deadline_ms",
            "liveliness_lease_ms",
            "partition",
        ])?;
        // A period in milliseconds: more than 0, and within what a DDS
        // Duration_t of whole milliseconds holds here (i32).
        let period = |key: &str| -> Result<Option<u32>, ConfigError> {
            match qos.scalar(key)? {
                None => Ok(None),
                Some((v, origin)) => v
                    .parse::<u32>()
                    .ok()
                    .filter(|n| *n > 0 && i32::try_from(*n).is_ok())
                    .map(Some)
                    .ok_or_else(|| {
                        invalid(
                            format!("{path}.qos.{key}"),
                            origin.clone(),
                            "milliseconds, from 1 to 2147483647",
                        )
                    }),
            }
        };
        t.deadline_ms = period("deadline_ms")?;
        t.liveliness_lease_ms = period("liveliness_lease_ms")?;
        for (item_path, node) in qos.items("partition")? {
            match node {
                zerodds_config::Node::Scalar(name, _) => t.partition.push(name.clone()),
                other => {
                    return Err(invalid(item_path, first_origin(other), "a partition name"));
                }
            }
        }
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::bridge_daemon::test_support::load;

    #[test]
    fn amqpbridge_3_what_the_daemon_would_not_honour_is_refused() {
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

    /// §3: `type_file` is read by the reader and the writer of a keyed
    /// topic, in every direction; on an unkeyed topic it is refused with the
    /// entry's path and the file and line it came from, not ignored.
    #[test]
    fn amqpbridge_3_a_type_file_the_topic_would_not_read_is_refused() {
        for yaml in [
            "topics:\n  - dds_name: T\n    amqp_address: a\n    type_file: /t.typeobject\n",
            "topics:\n  - dds_name: T\n    amqp_address: a\n    keyed: false\n    type_file: /t.typeobject\n",
            "topics:\n  - dds_name: T\n    amqp_address: a\n    keyed: false\n    direction: in\n    type_file: /t.typeobject\n",
        ] {
            let e = load(yaml).expect_err(yaml);
            assert!(e.contains("topics[0].type_file"), "{yaml}: {e}");
            // The type_file line, the last one of the document.
            let line = yaml.lines().count();
            assert!(e.contains(&format!("<string>:{line}:")), "{yaml}: {e}");
            assert!(e.contains("only a keyed topic reads"), "{yaml}: {e}");
        }
        for direction in ["in", "out", "bidir"] {
            let c = load(&format!(
                "topics:\n  - dds_name: T\n    amqp_address: a\n    keyed: true\n    direction: {direction}\n    type_file: /t.typeobject\n"
            ))
            .unwrap();
            assert_eq!(c.topics[0].type_file, "/t.typeobject");
        }
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
        include_str!("../../../../packaging/linux/configs/amqp-bridged.yaml.example");

    #[test]
    fn amqpbridge_3_the_shipped_example_reads_as_it_says() {
        let c = load(EXAMPLE_YAML).unwrap();
        assert_eq!(c.broker_url, "amqp://localhost:5672");
        assert_eq!(c.channel_max, 128, "not the default 256");
        assert_eq!(c.sasl_mechanism, "PLAIN");
        assert_eq!(c.acl_rules["OrderEvent"].0, vec!["zerodds".to_string()]);
        assert_eq!(c.metrics_listen, "127.0.0.1:9094");
        assert!(c.catalog_enabled);
        assert_eq!(c.catalog_address(), "/queues/zerodds.catalog");
        let t = &c.topics[0];
        assert_eq!(t.dds_name, "OrderEvent");
        assert!(t.keyed);
        assert_eq!(t.direction, Direction::Out);
        assert_eq!(t.history, History::KeepLast(10));
        assert_eq!(t.durable, Some(1));
        assert_eq!(t.credit, 256);
    }

    #[test]
    fn amqpbridge_3_values_the_shared_parser_reads_reach_the_daemon() {
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

    /// §2: the default container id is `zerodds-bridge-<host>`, the host
    /// name `gethostname(3)` gives, not the shell variable `HOSTNAME`, which
    /// neither systemd nor a shell exports.
    #[test]
    fn amqpbridge_2_the_default_container_id_names_the_host() {
        assert_eq!(
            container_id_for(Some("build.example.org")),
            "zerodds-bridge-build-example-org"
        );
        assert_eq!(
            container_id_for(Some(&"h".repeat(40))),
            format!("zerodds-bridge-{}", "h".repeat(32))
        );
        assert_eq!(
            container_id_for(None),
            format!("zerodds-bridge-pid{}", std::process::id())
        );
        // `hostname(1)` prints what gethostname(2) returns.
        #[cfg(unix)]
        {
            let out = std::process::Command::new("hostname").output().unwrap();
            let host = String::from_utf8(out.stdout).unwrap();
            assert_eq!(
                DaemonConfig::default().container_id,
                container_id_for(Some(host.trim()))
            );
            assert!(!DaemonConfig::default().container_id.contains("-pid"));
        }
    }

    /// The YAML block of §3, read from the specification file itself:
    /// `${AMQP_USER}` and `${AMQP_PASSWORD}` are expanded the way a file is
    /// (here from their defaults), and every section reaches the daemon.
    #[test]
    fn amqpbridge_3_the_example_of_the_spec_loads_from_the_spec_file() {
        let spec = include_str!("../../../../docs/specs/zerodds-amqp-bridge-daemon-1.0.md");
        let section = &spec[spec.find("## §3").unwrap()..];
        let start = section.find("```yaml\n").unwrap() + "```yaml\n".len();
        let block = &section[start..start + section[start..].find("```").unwrap()];
        // The parser expands `${VAR}` itself; the two names get defaults
        // that stand in for the environment.
        let yaml = block
            .replace(
                "${AMQP_USER}",
                "${ZERODDS_SPEC_PROBE_UNSET_USER_7c1:-bridge}",
            )
            .replace(
                "${AMQP_PASSWORD}",
                "${ZERODDS_SPEC_PROBE_UNSET_PW_7c1:-secret}",
            )
            .replace(
                "${PLANT_B_USER}",
                "${ZERODDS_SPEC_PROBE_UNSET_BUSER_7c1:-plant-b}",
            )
            .replace(
                "${PLANT_B_PASSWORD}",
                "${ZERODDS_SPEC_PROBE_UNSET_BPW_7c1:-b-secret}",
            );
        let c = load(&yaml).unwrap();
        assert_eq!(c.broker_url, "amqps://rabbitmq.example.com:5671");
        assert_eq!(c.container_id, "zerodds-bridge-prod-01");
        assert_eq!(c.hostname, "rabbitmq.example.com");
        assert_eq!(c.sasl_mechanism, "SCRAM-SHA-256");
        assert_eq!(
            (c.sasl_username.as_str(), c.sasl_password.as_str()),
            ("bridge", "secret")
        );
        assert!(c.tls_enabled);
        assert_eq!(c.tls_key_file, "/etc/zerodds/amqp-client.key");
        assert!((c.reconnect.factor - 2.0).abs() < f64::EPSILON);
        assert_eq!(c.topics.len(), 3);
        let chat = &c.topics[0];
        assert!(chat.keyed);
        assert_eq!(chat.type_file, "/etc/zerodds/chat-message.typeobject");
        assert_eq!(chat.amqp_address, "/exchanges/amq.topic/chat.message");
        assert_eq!(chat.direction, Direction::Bidir);
        assert_eq!(
            (chat.durable, chat.snd_settle_mode, chat.rcv_settle_mode),
            (Some(1), 0, 0)
        );
        assert_eq!(chat.history, History::KeepLast(10));
        assert_eq!(
            (chat.deadline_ms, chat.liveliness_lease_ms),
            (Some(2000), Some(10_000))
        );
        assert_eq!(chat.partition, ["site-a"]);
        let sensor = &c.topics[1];
        assert!(sensor.settled, "BEST_EFFORT is AT_MOST_ONCE");
        assert_eq!(sensor.snd_settle_mode, 1);
        let order = &c.topics[2];
        assert_eq!(order.direction, Direction::In);
        assert_eq!(order.credit, 1024);
        assert_eq!(
            order.durability,
            zerodds_qos::DurabilityKind::TransientLocal
        );
        assert_eq!(
            c.acl_rules["Order::Event"],
            (vec!["*".to_string()], vec!["orders-service".to_string()])
        );
        assert!(!c.acl_default_deny);
        assert_eq!(c.catalog_address(), "/queues/zerodds.catalog");
        assert!(c.metrics_enabled);
        assert_eq!(c.metrics_listen, "127.0.0.1:9094");
        assert_eq!(c.tenants.len(), 1);
        let b = c.of_tenant(&c.tenants[0]);
        assert_eq!(
            (b.domain, b.container_id.as_str(), b.hostname.as_str()),
            (7, "zerodds-bridge-plant-b", "vhost:plant-b")
        );
        assert_eq!(
            (b.sasl_mechanism.as_str(), b.sasl_username.as_str()),
            ("SCRAM-SHA-256", "plant-b")
        );
        assert_eq!(b.catalog_address(), "/queues/plant-b.catalog");
        assert_eq!(b.topics[0].amqp_address, "/queues/plant-b.orders");
    }

    /// §5.1: `amqp_address` is mandatory, in the map form and in the
    /// `--topic DDS=ADDR` form; the refusal names the entry.
    #[test]
    fn amqpbridge_5_1_a_topic_without_amqp_address_is_refused() {
        for yaml in [
            "topics:\n  - dds_name: T\n",
            "topics:\n  - dds_name: T\n    amqp_address: \"\"\n",
            "topics:\n  - \"T=\"\n",
            "topics:\n  - dds_name: A\n    amqp_address: a\n  - dds_name: B\n    direction: in\n",
        ] {
            let e = load(yaml).expect_err(yaml);
            assert!(e.contains("amqp_address"), "{yaml}: {e}");
            assert!(e.contains("topics["), "{yaml}: {e}");
        }
        let e =
            load("topics:\n  - dds_name: A\n    amqp_address: a\n  - dds_name: B\n").unwrap_err();
        assert!(e.contains("topics[1]"), "{e}");
        let e = load("topics:\n  - T\n").unwrap_err();
        assert!(e.contains("DDS=ADDRESS"), "{e}");
    }

    /// §2/§5.1: `--topic DDS=ADDR` maps one topic with the defaults of a
    /// map entry: both directions, the type named like the topic.
    #[test]
    fn amqpbridge_5_1_the_topic_flag_form_is_dds_equals_address() {
        let c = load("topics:\n  - \"Chat::Message=/queues/chat\"\n").unwrap();
        let t = &c.topics[0];
        assert_eq!(t.dds_name, "Chat::Message");
        assert_eq!(t.dds_type, "Chat::Message");
        assert_eq!(t.amqp_address, "/queues/chat");
        assert_eq!(t.direction, Direction::Bidir);
        assert!(t.reliable && !t.settled);
        assert_eq!(t.credit, 256);
    }

    /// §6: `qos.deadline_ms`, `qos.liveliness_lease_ms` and `qos.partition`
    /// are read; a period of 0 or beyond i32 milliseconds and a partition
    /// that is not a name are refused with the entry's path. A durability
    /// other than VOLATILE asks for a durable node (terminus durable 1) on
    /// the target of the sender link, not on the source of the receiver
    /// link, unless `sender.durable` is set.
    #[test]
    fn amqpbridge_6_the_qos_keys_are_read() {
        let c = load(
            "topics:\n  - dds_name: A\n    amqp_address: a\n    qos:\n      deadline_ms: 250\n      liveliness_lease_ms: 4000\n      partition: [\"site-a\", \"x*\"]\n      durability: transient_local\n  - dds_name: B\n    amqp_address: b\n    sender:\n      durable: 0\n    qos:\n      durability: persistent\n  - dds_name: C\n    amqp_address: c\n",
        )
        .unwrap();
        let (a, b, cc) = (&c.topics[0], &c.topics[1], &c.topics[2]);
        assert_eq!(
            (a.deadline_ms, a.liveliness_lease_ms),
            (Some(250), Some(4000))
        );
        assert_eq!(a.partition, ["site-a", "x*"]);
        assert_eq!(
            (a.terminus_durable(true), a.terminus_durable(false)),
            (1, 0),
            "transient_local: a durable node for the messages the daemon sends"
        );
        assert_eq!(
            (b.terminus_durable(true), b.terminus_durable(false)),
            (0, 0),
            "sender.durable set"
        );
        assert_eq!(
            (
                cc.deadline_ms,
                cc.liveliness_lease_ms,
                cc.terminus_durable(true),
                cc.terminus_durable(false)
            ),
            (None, None, 0, 0)
        );
        assert!(cc.partition.is_empty());
        for (yaml, needle) in [
            ("deadline_ms: 0", "topics[0].qos.deadline_ms"),
            (
                "liveliness_lease_ms: 2147483648",
                "topics[0].qos.liveliness_lease_ms",
            ),
            ("deadline_ms: soon", "topics[0].qos.deadline_ms"),
            ("partition:\n        - n: a", "topics[0].qos.partition[0]"),
            ("partition: a", "topics[0].qos.partition"),
        ] {
            let text =
                format!("topics:\n  - dds_name: T\n    amqp_address: a\n    qos:\n      {yaml}\n");
            let e = load(&text).expect_err(&text);
            assert!(e.contains(needle), "{yaml}: {e}");
        }
    }

    /// §1 L6: a tenant has its name, domain, container id, connection
    /// identity and topics; what it does not set is the daemon's.
    #[test]
    fn amqpbridge_1_l6_the_tenants_are_read() {
        let c = load(
            "amqp:\n  container_id: own\n  hostname: vhost:own\n  sasl:\n    mechanism: PLAIN\n    username: u\n    password: p\ncatalog:\n  address: /queues/own.catalog\ntopics:\n  - dds_name: A\n    amqp_address: /queues/a\ntenants:\n  - name: plant-b\n    domain: 9\n    amqp:\n      container_id: bridge-b\n      hostname: vhost:b\n      sasl:\n        username: bob\n        password: secret\n      tls:\n        cert_file: /b.pem\n        key_file: /b.key\n    topics:\n      - dds_name: A\n        amqp_address: /queues/b\n        qos:\n          partition: [p]\n",
        )
        .unwrap();
        assert_eq!(c.tenants.len(), 1);
        let t = c.of_tenant(&c.tenants[0]);
        assert_eq!(
            (t.domain, t.container_id.as_str(), t.hostname.as_str()),
            (9, "bridge-b", "vhost:b")
        );
        assert_eq!(
            (
                t.sasl_mechanism.as_str(),
                t.sasl_username.as_str(),
                t.sasl_password.as_str()
            ),
            ("PLAIN", "bob", "secret"),
            "the mechanism is the daemon's"
        );
        assert_eq!(
            (t.tls_cert_file.as_str(), t.tls_key_file.as_str()),
            ("/b.pem", "/b.key")
        );
        assert_eq!(t.topics.len(), 1);
        assert_eq!(t.topics[0].amqp_address, "/queues/b");
        assert_eq!(t.topics[0].partition, ["p"]);
        assert!(t.tenants.is_empty());
        assert_eq!(
            t.catalog_address(),
            "zerodds.bridge.bridge-b.catalog",
            "not the daemon's own catalog address"
        );
        assert_eq!(c.catalog_address(), "/queues/own.catalog");
    }

    /// §1 L6: a tenant without a name, domain, container id or topics, two
    /// tenants of one name and two participants with one container id are
    /// refused, with the path of the entry and its file and line.
    #[test]
    fn amqpbridge_1_l6_a_tenant_that_cannot_be_told_apart_is_refused() {
        let tenant = |name: &str, cid: &str| {
            format!(
                "  - name: {name}\n    domain: 3\n    amqp:\n      container_id: {cid}\n    topics:\n      - dds_name: T\n        amqp_address: a\n"
            )
        };
        let own = "amqp:\n  container_id: own\ntenants:\n";
        for (yaml, needle) in [
            (
                format!(
                    "{own}  - domain: 3\n    amqp:\n      container_id: x\n    topics:\n      - dds_name: T\n        amqp_address: a\n"
                ),
                "tenants[0].name",
            ),
            (
                format!(
                    "{own}  - name: t\n    amqp:\n      container_id: x\n    topics:\n      - dds_name: T\n        amqp_address: a\n"
                ),
                "tenants[0].domain",
            ),
            (
                format!(
                    "{own}  - name: t\n    domain: 3\n    topics:\n      - dds_name: T\n        amqp_address: a\n"
                ),
                "tenants[0].amqp.container_id",
            ),
            (
                format!("{own}  - name: t\n    domain: 3\n    amqp:\n      container_id: x\n"),
                "tenants[0].topics",
            ),
            // The second entry, by its path and the line it is on.
            (
                format!("{own}{}{}", tenant("t", "x"), tenant("t", "y")),
                "<string>:11: invalid tenants[1].name: the tenant t is named twice",
            ),
            (
                format!("{own}{}{}", tenant("t", "x"), tenant("u", "x")),
                "<string>:14: invalid tenants[1].amqp.container_id: the tenant u has the container id x",
            ),
            (
                format!("{own}{}", tenant("t", "own")),
                "<string>:7: invalid tenants[0].amqp.container_id: the tenant t has the container id own",
            ),
            (
                format!(
                    "{own}  - name: t\n    domain: 3\n    amqp:\n      container_id: x\n      channel_max: 3\n    topics:\n      - dds_name: T\n        amqp_address: a\n"
                ),
                "channel_max",
            ),
        ] {
            let e = load(&yaml).expect_err(&yaml);
            assert!(e.contains(needle), "{yaml}: {e}");
        }
    }
}
