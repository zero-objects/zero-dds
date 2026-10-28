// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Config for `zerodds-opcua-bridged`. Spec `zerodds-opcua-bridge-1.0` §3.
//!
//! Read by the shared layered loader (`zerodds-config`): base file,
//! `opcua-bridged.d/` drop-ins, includes, `${VAR}`, and the precedence
//! defaults < environment < files < `--set`. [`DaemonConfig::from_config`]
//! reads this daemon's keys and refuses an unknown key inside a known
//! section, and a value of the wrong shape or type, with its file and line.

use std::path::PathBuf;
use std::string::{String, ToString};
use std::vec::Vec;

pub use zerodds_config::ConfigError;
use zerodds_config::read::{Section, first_origin};

/// The base name of the daemon's file: `opcua-bridged.yaml`, `opcua-bridged.d/`.
pub const CONFIG_STEM: &str = "opcua-bridged";

/// Every key this daemon reads; list-valued keys stop at the list.
pub const KEYS: &[&str] = &[
    "domain",
    "log_level",
    "opcua.idl",
    "opcua.gateway_config",
    "opcua.server.enabled",
    "opcua.server.bind",
    "opcua.server.security_policies",
    "opcua.server.certificate_der",
    "opcua.server.private_key_pem",
    "opcua.server.trusted_client_certificate_der",
    "opcua.server.trusted_client_public_key_pem",
    "opcua.server.historical.topics",
    "opcua.client.enabled",
    "opcua.client.servers",
    "opcua.pubsub.enabled",
    "opcua.pubsub.connections",
    "acl.default_deny",
    "acl.rules",
    "acl.allow_anonymous",
    "acl.users",
    "metrics.enabled",
    "metrics.listen",
    "metrics.otlp_endpoint",
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

/// `opcua.server.*` — this daemon's own `opc.tcp` server (Gateway 1.0 §9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    /// `opcua.server.enabled`.
    pub enabled: bool,
    /// `opcua.server.bind`.
    pub bind: String,
    /// `opcua.server.security_policies` — `none` plus, with the `crypto`
    /// feature, `basic256sha256`/`aes128_sha256_rsaoaep`/`aes256_sha256_rsapss`.
    pub security_policies: Vec<String>,
    /// `opcua.server.certificate_der`.
    pub certificate_der: Option<PathBuf>,
    /// `opcua.server.private_key_pem`.
    pub private_key_pem: Option<PathBuf>,
    /// `opcua.server.trusted_client_certificate_der`.
    pub trusted_client_certificate_der: Option<PathBuf>,
    /// `opcua.server.trusted_client_public_key_pem`.
    pub trusted_client_public_key_pem: Option<PathBuf>,
    /// `opcua.server.historical.*` — Spec §9.3.4.4 Historical Access.
    pub historical: HistoricalConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind: "0.0.0.0:4840".to_string(),
            security_policies: vec!["none".to_string()],
            certificate_der: None,
            private_key_pem: None,
            trusted_client_certificate_der: None,
            trusted_client_public_key_pem: None,
            historical: HistoricalConfig::default(),
        }
    }
}

/// One `opcua.server.historical.topics[]` entry: a `dds_to_ua_connection`
/// topic named here has every leaf of its sample tree marked historizing
/// (Spec §9.3.4.4), retaining up to `depth` samples per leaf
/// ([`zerodds_opcua_gateway::historical::adapter::InMemoryHistoryCache::push_bounded`]'s
/// retention bound).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalTopicConfig {
    /// `dds_topic` — must match a `dds_to_ua_connection`'s own topic name;
    /// a name matching no such connection is refused at load time (never
    /// silently unused).
    pub dds_topic: String,
    /// `depth` — the per-leaf retention bound, at least 1.
    pub depth: usize,
}

/// `opcua.server.historical.*`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HistoricalConfig {
    /// `opcua.server.historical.topics`.
    pub topics: Vec<HistoricalTopicConfig>,
}

/// One `acl.users[]` entry: a UserName credential (Part 4 §7.36.4)
/// this daemon's OPC-UA server's `UserStore` accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AclUserConfig {
    /// `policy_id` — the UserTokenPolicy id the client's
    /// `UserIdentityToken` names (Part 4 §7.41). Default `"user"`.
    pub policy_id: String,
    /// `username`.
    pub username: String,
    /// `password`.
    pub password: String,
}

/// One `opcua.client.servers[].nodes[]` entry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodeMapping {
    /// `node_id` — the canonical string form (`ns=N;i=NNN` or `ns=N;s=NAME`).
    pub node_id: String,
    /// `dds_topic`.
    pub dds_topic: String,
    /// `dds_type`.
    pub dds_type: String,
    /// `field_path` — where in the topic's type this node's value lands
    /// (empty = the whole, scalar sample).
    pub field_path: String,
    /// `keyed`.
    pub keyed: bool,
}

/// One `opcua.client.servers[]` entry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ForeignServerConfig {
    /// `endpoint_url`.
    pub endpoint_url: String,
    /// `nodes`.
    pub nodes: Vec<NodeMapping>,
}

/// `opcua.client.*` — Gateway 1.0 §8's client role.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClientConfig {
    /// `opcua.client.enabled`.
    pub enabled: bool,
    /// `opcua.client.servers`.
    pub servers: Vec<ForeignServerConfig>,
}

/// One `opcua.pubsub.connections[].publishers[]` entry (DDS → UADP).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublisherConfig {
    /// `dds_topic`.
    pub dds_topic: String,
    /// `dds_type`.
    pub dds_type: String,
    /// `keyed`.
    pub keyed: bool,
    /// `writer_group_id`.
    pub writer_group_id: u16,
    /// `dataset_writer_id`.
    pub dataset_writer_id: u16,
    /// `dataset_name`.
    pub dataset_name: String,
    /// `publish_interval_ms`.
    pub publish_interval_ms: u32,
}

/// One `opcua.pubsub.connections[].subscribers[]` entry (UADP → DDS).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscriberConfig {
    /// `dds_topic`.
    pub dds_topic: String,
    /// `dds_type`.
    pub dds_type: String,
    /// `keyed`.
    pub keyed: bool,
    /// `dataset_name`.
    pub dataset_name: String,
    /// `fields` — the remote Publisher's own field order.
    pub fields: Vec<String>,
}

/// One `opcua.pubsub.connections[]` entry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PubSubConnectionConfig {
    /// `name`.
    pub name: String,
    /// `local_addr` — the UDP bind address.
    pub local_addr: String,
    /// `multicast_addr` — the UADP multicast group + port.
    pub multicast_addr: String,
    /// `publisher_id`.
    pub publisher_id: u16,
    /// `publishers`.
    pub publishers: Vec<PublisherConfig>,
    /// `subscribers`.
    pub subscribers: Vec<SubscriberConfig>,
    /// `security` — Part 14 §8 UADP message security for this
    /// connection's NetworkMessages (feature `pubsub-security`), or
    /// `None` for cleartext UADP (this daemon's unchanged default).
    pub security: Option<PubSubSecurityConfig>,
}

/// A UADP message-security SecurityPolicy (Part 14 §8.2.1) as named in
/// `opcua.pubsub.connections[].security.policy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PubSubSecurityPolicy {
    /// `aes128-ctr`.
    Aes128Ctr,
    /// `aes256-ctr`.
    Aes256Ctr,
    /// `aes256-gcm`.
    Aes256Gcm,
}

/// `opcua.pubsub.connections[].security` — a single static current key
/// (Part 14 §8.2.2's `SecurityTokenId`), not a live `GetSecurityKeys`/SKS
/// rotation RPC: an operator rotates it by reloading config with a new
/// key/`token_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubSubSecurityConfig {
    /// `policy`.
    pub policy: PubSubSecurityPolicy,
    /// `security_group_id`.
    pub security_group_id: String,
    /// `token_id` — the `SecurityTokenId` this key set is identified by.
    pub token_id: u32,
    /// `signing_key`, hex-encoded (empty for `aes256-gcm`, whose
    /// integrity comes from the GCM tag, not a separate HMAC).
    pub signing_key: Vec<u8>,
    /// `encrypting_key`, hex-encoded.
    pub encrypting_key: Vec<u8>,
    /// `key_nonce`, hex-encoded (4 bytes — the high bytes of the AES-CTR/
    /// GCM IV).
    pub key_nonce: Vec<u8>,
}

/// `opcua.pubsub.*` — Part 14 UADP.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PubSubConfig {
    /// `opcua.pubsub.enabled`.
    pub enabled: bool,
    /// `opcua.pubsub.connections`.
    pub connections: Vec<PubSubConnectionConfig>,
}

/// Daemon config.
#[derive(Debug, Clone, PartialEq)]
pub struct DaemonConfig {
    /// `domain:`.
    pub domain: i32,
    /// `log_level:`.
    pub log_level: String,
    /// `opcua.idl:` — `.idl` files compiled to resolve `dds_type` names.
    pub idl: Vec<PathBuf>,
    /// `opcua.gateway_config:` — Spec §10 XML path.
    pub gateway_config: Option<PathBuf>,
    /// `opcua.server.*`.
    pub server: ServerConfig,
    /// `opcua.client.*`.
    pub client: ClientConfig,
    /// `opcua.pubsub.*`.
    pub pubsub: PubSubConfig,
    /// `acl.default_deny`.
    pub acl_default_deny: bool,
    /// `acl.rules` — `(subject, allow_read topics, allow_write topics)`.
    pub acl_rules: Vec<(String, Vec<String>, Vec<String>)>,
    /// `acl.allow_anonymous` — whether the OPC-UA server's own
    /// `UserStore` accepts the Anonymous `UserIdentityToken`. Default
    /// `true` (unchanged behavior when `acl.users` names no credential).
    pub acl_allow_anonymous: bool,
    /// `acl.users` — UserName credentials this daemon's server accepts at
    /// ActivateSession; each user's own name is the subject
    /// `acl.rules[].subject` and the per-session Write ACL match against
    /// (`crate::daemon::security::client_subject`).
    pub acl_users: Vec<AclUserConfig>,
    /// `metrics.enabled`.
    pub metrics_enabled: bool,
    /// `metrics.listen`.
    pub metrics_addr: String,
    /// `metrics.otlp_endpoint` — an OTel collector's OTLP/HTTP base URL
    /// (`host:port` or `http://host:port`); `None` exports no spans.
    pub metrics_otlp_endpoint: Option<String>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self::default_for_dev()
    }
}

impl DaemonConfig {
    /// Default for dev: the server enabled on the standard port, no
    /// client/PubSub connections, no ACL.
    #[must_use]
    pub fn default_for_dev() -> Self {
        Self {
            domain: 0,
            log_level: "info".to_string(),
            idl: Vec::new(),
            gateway_config: None,
            server: ServerConfig::default(),
            client: ClientConfig::default(),
            pubsub: PubSubConfig::default(),
            acl_default_deny: false,
            acl_rules: Vec::new(),
            acl_allow_anonymous: true,
            acl_users: Vec::new(),
            metrics_enabled: false,
            metrics_addr: String::new(),
            metrics_otlp_endpoint: None,
        }
    }

    /// Loads one file (and its sibling `<stem>.d/`), for tests and tools.
    /// The daemon itself loads through `zerodds_dcps::cli::Configured`.
    ///
    /// # Errors
    /// Unreadable file, text outside the subset, or a refused value.
    pub fn load_from_file(path: &std::path::Path) -> Result<Self, ConfigError> {
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

        let opcua = root.section("opcua")?;
        opcua.only(&["idl", "gateway_config", "server", "client", "pubsub"])?;
        for path in opcua.strings("idl")?.unwrap_or_default() {
            out.idl.push(PathBuf::from(path));
        }
        out.gateway_config = opcua.string("gateway_config")?.map(PathBuf::from);

        let server = opcua.section("server")?;
        server.only(&[
            "enabled",
            "bind",
            "security_policies",
            "certificate_der",
            "private_key_pem",
            "trusted_client_certificate_der",
            "trusted_client_public_key_pem",
            "historical",
        ])?;
        if let Some(v) = server.boolean("enabled")? {
            out.server.enabled = v;
        }
        if let Some(v) = server.string("bind")? {
            out.server.bind = v;
        }
        if let Some(policies) = server.strings("security_policies")? {
            for policy in &policies {
                if !matches!(
                    policy.as_str(),
                    "none" | "basic256sha256" | "aes128_sha256_rsaoaep" | "aes256_sha256_rsapss"
                ) {
                    return Err(invalid(
                        server.path_of("security_policies"),
                        server
                            .node("security_policies")
                            .map_or(zerodds_config::Origin::Default, first_origin),
                        std::format!("unknown SecurityPolicy `{policy}`"),
                    ));
                }
            }
            out.server.security_policies = policies;
        }
        out.server.certificate_der = server.string("certificate_der")?.map(PathBuf::from);
        out.server.private_key_pem = server.string("private_key_pem")?.map(PathBuf::from);
        out.server.trusted_client_certificate_der = server
            .string("trusted_client_certificate_der")?
            .map(PathBuf::from);
        out.server.trusted_client_public_key_pem = server
            .string("trusted_client_public_key_pem")?
            .map(PathBuf::from);
        let historical = server.section("historical")?;
        historical.only(&["topics"])?;
        for (path, item) in historical.items("topics")? {
            let entry = Section::of(Some(item), path.clone())?;
            entry.only(&["dds_topic", "depth"])?;
            let dds_topic = entry.string("dds_topic")?.ok_or_else(|| {
                invalid(
                    entry.path_of("dds_topic"),
                    first_origin(item),
                    "a historical topic entry needs a dds_topic".to_string(),
                )
            })?;
            let depth = entry
                .parse::<usize>("depth", "a positive integer")?
                .ok_or_else(|| {
                    invalid(
                        entry.path_of("depth"),
                        first_origin(item),
                        "a historical topic entry needs a depth".to_string(),
                    )
                })?;
            if depth == 0 {
                return Err(invalid(
                    entry.path_of("depth"),
                    first_origin(item),
                    "depth must be at least 1".to_string(),
                ));
            }
            out.server
                .historical
                .topics
                .push(HistoricalTopicConfig { dds_topic, depth });
        }

        let secured_policy_requested = out.server.security_policies.iter().any(|p| p != "none");
        if secured_policy_requested
            && (out.server.certificate_der.is_none() || out.server.private_key_pem.is_none())
        {
            return Err(invalid(
                "opcua.server".to_string(),
                server
                    .node("security_policies")
                    .map_or(zerodds_config::Origin::Default, first_origin),
                "a secured security_policy needs certificate_der and private_key_pem".to_string(),
            ));
        }

        let client = opcua.section("client")?;
        client.only(&["enabled", "servers"])?;
        if let Some(v) = client.boolean("enabled")? {
            out.client.enabled = v;
        }
        for (path, item) in client.items("servers")? {
            out.client.servers.push(read_foreign_server(&path, item)?);
        }

        let pubsub = opcua.section("pubsub")?;
        pubsub.only(&["enabled", "connections"])?;
        if let Some(v) = pubsub.boolean("enabled")? {
            out.pubsub.enabled = v;
        }
        for (path, item) in pubsub.items("connections")? {
            out.pubsub
                .connections
                .push(read_pubsub_connection(&path, item)?);
        }

        let acl = root.section("acl")?;
        acl.only(&["default_deny", "rules", "allow_anonymous", "users"])?;
        if let Some(v) = acl.boolean("default_deny")? {
            out.acl_default_deny = v;
        }
        for (path, rule) in acl.items("rules")? {
            let entry = Section::of(Some(rule), path)?;
            entry.only(&["subject", "allow_read", "allow_write"])?;
            let subject = entry.string("subject")?.ok_or_else(|| {
                invalid(
                    entry.path_of("subject"),
                    first_origin(rule),
                    "an acl rule needs a subject".to_string(),
                )
            })?;
            let allow_read = entry.strings("allow_read")?.unwrap_or_default();
            let allow_write = entry.strings("allow_write")?.unwrap_or_default();
            out.acl_rules.push((subject, allow_read, allow_write));
        }
        if let Some(v) = acl.boolean("allow_anonymous")? {
            out.acl_allow_anonymous = v;
        }
        for (path, user) in acl.items("users")? {
            let entry = Section::of(Some(user), path)?;
            entry.only(&["policy_id", "username", "password"])?;
            let username = entry.string("username")?.ok_or_else(|| {
                invalid(
                    entry.path_of("username"),
                    first_origin(user),
                    "an acl user entry needs a username".to_string(),
                )
            })?;
            let password = entry.string("password")?.ok_or_else(|| {
                invalid(
                    entry.path_of("password"),
                    first_origin(user),
                    "an acl user entry needs a password".to_string(),
                )
            })?;
            let policy_id = entry
                .string("policy_id")?
                .unwrap_or_else(|| "user".to_string());
            out.acl_users.push(AclUserConfig {
                policy_id,
                username,
                password,
            });
        }

        let metrics = root.section("metrics")?;
        metrics.only(&["enabled", "listen", "otlp_endpoint"])?;
        if let Some(v) = metrics.boolean("enabled")? {
            out.metrics_enabled = v;
        }
        if let Some(v) = metrics.string("listen")? {
            out.metrics_addr = v;
        }
        out.metrics_otlp_endpoint = metrics.string("otlp_endpoint")?;

        Ok(out)
    }
}

fn read_foreign_server(
    path: &str,
    item: &zerodds_config::Node,
) -> Result<ForeignServerConfig, ConfigError> {
    let entry = Section::of(Some(item), path.to_string())?;
    entry.only(&["endpoint_url", "nodes"])?;
    let endpoint_url = entry.string("endpoint_url")?.ok_or_else(|| {
        invalid(
            entry.path_of("endpoint_url"),
            first_origin(item),
            "a server entry needs an endpoint_url".to_string(),
        )
    })?;
    let mut nodes = Vec::new();
    for (node_path, node_item) in entry.items("nodes")? {
        let node_entry = Section::of(Some(node_item), node_path.clone())?;
        node_entry.only(&["node_id", "dds_topic", "dds_type", "field_path", "keyed"])?;
        let node_id = node_entry.string("node_id")?.ok_or_else(|| {
            invalid(
                node_entry.path_of("node_id"),
                first_origin(node_item),
                "a node entry needs a node_id".to_string(),
            )
        })?;
        let dds_topic = node_entry.string("dds_topic")?.ok_or_else(|| {
            invalid(
                node_entry.path_of("dds_topic"),
                first_origin(node_item),
                "a node entry needs a dds_topic".to_string(),
            )
        })?;
        let dds_type = node_entry
            .string("dds_type")?
            .unwrap_or_else(|| dds_topic.clone());
        let field_path = node_entry.string("field_path")?.unwrap_or_default();
        let keyed = node_entry.boolean("keyed")?.unwrap_or(false);
        nodes.push(NodeMapping {
            node_id,
            dds_topic,
            dds_type,
            field_path,
            keyed,
        });
    }
    Ok(ForeignServerConfig {
        endpoint_url,
        nodes,
    })
}

fn read_pubsub_connection(
    path: &str,
    item: &zerodds_config::Node,
) -> Result<PubSubConnectionConfig, ConfigError> {
    let entry = Section::of(Some(item), path.to_string())?;
    entry.only(&[
        "name",
        "local_addr",
        "multicast_addr",
        "publisher_id",
        "publishers",
        "subscribers",
        "security",
    ])?;
    let name = entry.string("name")?.unwrap_or_else(|| path.to_string());
    let local_addr = entry
        .string("local_addr")?
        .unwrap_or_else(|| "0.0.0.0:0".to_string());
    let multicast_addr = entry.string("multicast_addr")?.ok_or_else(|| {
        invalid(
            entry.path_of("multicast_addr"),
            first_origin(item),
            "a pubsub connection needs a multicast_addr".to_string(),
        )
    })?;
    let publisher_id = entry
        .parse::<u16>("publisher_id", "an integer 0..=65535")?
        .unwrap_or(1);

    let mut publishers = Vec::new();
    for (p_path, p_item) in entry.items("publishers")? {
        let p = Section::of(Some(p_item), p_path.clone())?;
        p.only(&[
            "dds_topic",
            "dds_type",
            "keyed",
            "writer_group_id",
            "dataset_writer_id",
            "dataset_name",
            "publish_interval_ms",
        ])?;
        let dds_topic = p.string("dds_topic")?.ok_or_else(|| {
            invalid(
                p.path_of("dds_topic"),
                first_origin(p_item),
                "a publisher entry needs a dds_topic".to_string(),
            )
        })?;
        let dds_type = p.string("dds_type")?.unwrap_or_else(|| dds_topic.clone());
        let dataset_name = p
            .string("dataset_name")?
            .unwrap_or_else(|| dds_topic.clone());
        publishers.push(PublisherConfig {
            dds_topic,
            dds_type,
            keyed: p.boolean("keyed")?.unwrap_or(false),
            writer_group_id: p
                .parse::<u16>("writer_group_id", "an integer 0..=65535")?
                .unwrap_or(1),
            dataset_writer_id: p
                .parse::<u16>("dataset_writer_id", "an integer 0..=65535")?
                .unwrap_or(1),
            dataset_name,
            publish_interval_ms: p
                .parse::<u32>("publish_interval_ms", "milliseconds, an integer")?
                .unwrap_or(1000),
        });
    }

    let mut subscribers = Vec::new();
    for (s_path, s_item) in entry.items("subscribers")? {
        let s = Section::of(Some(s_item), s_path.clone())?;
        s.only(&["dds_topic", "dds_type", "keyed", "dataset_name", "fields"])?;
        let dds_topic = s.string("dds_topic")?.ok_or_else(|| {
            invalid(
                s.path_of("dds_topic"),
                first_origin(s_item),
                "a subscriber entry needs a dds_topic".to_string(),
            )
        })?;
        let dds_type = s.string("dds_type")?.unwrap_or_else(|| dds_topic.clone());
        let dataset_name = s
            .string("dataset_name")?
            .unwrap_or_else(|| dds_topic.clone());
        let fields = s.strings("fields")?.ok_or_else(|| {
            invalid(
                s.path_of("fields"),
                first_origin(s_item),
                "a subscriber entry needs its remote Publisher's field order".to_string(),
            )
        })?;
        subscribers.push(SubscriberConfig {
            dds_topic,
            dds_type,
            keyed: s.boolean("keyed")?.unwrap_or(false),
            dataset_name,
            fields,
        });
    }

    let security = if entry.node("security").is_some() {
        Some(read_pubsub_security(&entry, item)?)
    } else {
        None
    };

    Ok(PubSubConnectionConfig {
        name,
        local_addr,
        multicast_addr,
        publisher_id,
        publishers,
        subscribers,
        security,
    })
}

fn read_pubsub_security(
    entry: &Section<'_>,
    item: &zerodds_config::Node,
) -> Result<PubSubSecurityConfig, ConfigError> {
    let sec = entry.section("security")?;
    sec.only(&[
        "policy",
        "security_group_id",
        "token_id",
        "signing_key",
        "encrypting_key",
        "key_nonce",
    ])?;
    let policy_str = sec.string("policy")?.ok_or_else(|| {
        invalid(
            sec.path_of("policy"),
            first_origin(item),
            "a pubsub security entry needs a policy".to_string(),
        )
    })?;
    let policy = match policy_str.as_str() {
        "aes128-ctr" => PubSubSecurityPolicy::Aes128Ctr,
        "aes256-ctr" => PubSubSecurityPolicy::Aes256Ctr,
        "aes256-gcm" => PubSubSecurityPolicy::Aes256Gcm,
        other => {
            return Err(invalid(
                sec.path_of("policy"),
                first_origin(item),
                std::format!(
                    "unknown security policy `{other}` (expected aes128-ctr, aes256-ctr or aes256-gcm)"
                ),
            ));
        }
    };
    let security_group_id = sec.string("security_group_id")?.ok_or_else(|| {
        invalid(
            sec.path_of("security_group_id"),
            first_origin(item),
            "a pubsub security entry needs a security_group_id".to_string(),
        )
    })?;
    let token_id = sec
        .parse::<u32>("token_id", "an integer 0..=4294967295")?
        .unwrap_or(1);
    let signing_key = hex_decode(&sec, "signing_key", item)?;
    let encrypting_key = hex_decode(&sec, "encrypting_key", item)?;
    let key_nonce = hex_decode(&sec, "key_nonce", item)?;

    let (signing_len, encrypting_len, nonce_len) = match policy {
        PubSubSecurityPolicy::Aes128Ctr => (32usize, 16usize, 4usize),
        PubSubSecurityPolicy::Aes256Ctr => (32, 32, 4),
        PubSubSecurityPolicy::Aes256Gcm => (0, 32, 4),
    };
    if signing_key.len() != signing_len
        || encrypting_key.len() != encrypting_len
        || key_nonce.len() != nonce_len
    {
        return Err(invalid(
            sec.path_of("signing_key"),
            first_origin(item),
            std::format!(
                "{policy_str} needs signing_key/encrypting_key/key_nonce of {signing_len}/{encrypting_len}/{nonce_len} bytes (hex-encoded)"
            ),
        ));
    }

    Ok(PubSubSecurityConfig {
        policy,
        security_group_id,
        token_id,
        signing_key,
        encrypting_key,
        key_nonce,
    })
}

/// Hex-decodes `key` under `sec`, refusing an odd length or a non-hex
/// character with the setting's own path — never silently truncating or
/// masking a copy-paste error in an operator-supplied key.
fn hex_decode(
    sec: &Section<'_>,
    key: &str,
    item: &zerodds_config::Node,
) -> Result<Vec<u8>, ConfigError> {
    let Some(text) = sec.string(key)? else {
        return Ok(Vec::new());
    };
    if text.len() % 2 != 0 {
        return Err(invalid(
            sec.path_of(key),
            first_origin(item),
            "hex-encoded key must have an even number of characters".to_string(),
        ));
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    for i in (0..text.len()).step_by(2) {
        let byte = u8::from_str_radix(&text[i..i + 2], 16).map_err(|_| {
            invalid(
                sec.path_of(key),
                first_origin(item),
                std::format!("`{text}` is not valid hex"),
            )
        })?;
        out.push(byte);
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_config() {
        let yaml = "\
domain: 7
opcua:
  server:
    bind: 0.0.0.0:4840
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert_eq!(cfg.domain, 7);
        assert_eq!(cfg.server.bind, "0.0.0.0:4840");
        assert!(cfg.server.enabled);
    }

    #[test]
    fn rejects_unknown_top_level_section_key() {
        let yaml = "opcua:\n  serverr:\n    bind: x\n";
        let err = DaemonConfig::load_from_str(yaml).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid { .. }));
    }

    #[test]
    fn client_server_entry_needs_endpoint_url() {
        let yaml = "\
opcua:
  client:
    enabled: true
    servers:
      - nodes: []
";
        let err = DaemonConfig::load_from_str(yaml).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid { .. }));
    }

    #[test]
    fn parses_a_full_client_server_entry() {
        let yaml = "\
opcua:
  client:
    enabled: true
    servers:
      - endpoint_url: \"opc.tcp://plc.local:4840\"
        nodes:
          - node_id: \"ns=2;s=Cmd\"
            dds_topic: Commands
            dds_type: robotics::Command
            field_path: value
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert!(cfg.client.enabled);
        assert_eq!(cfg.client.servers.len(), 1);
        assert_eq!(
            cfg.client.servers[0].endpoint_url,
            "opc.tcp://plc.local:4840"
        );
        assert_eq!(cfg.client.servers[0].nodes[0].dds_topic, "Commands");
    }

    #[test]
    fn parses_a_full_pubsub_connection() {
        let yaml = "\
opcua:
  pubsub:
    enabled: true
    connections:
      - name: primary
        multicast_addr: \"224.0.0.22:4840\"
        publishers:
          - dds_topic: Telemetry
            writer_group_id: 1
            dataset_writer_id: 1
        subscribers:
          - dds_topic: Commands
            fields: [\"id\", \"value\"]
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert!(cfg.pubsub.enabled);
        let conn = &cfg.pubsub.connections[0];
        assert_eq!(conn.multicast_addr, "224.0.0.22:4840");
        assert_eq!(conn.publishers[0].dds_topic, "Telemetry");
        assert_eq!(
            conn.subscribers[0].fields,
            vec!["id".to_string(), "value".to_string()]
        );
    }

    #[test]
    fn pubsub_security_parses_and_hex_decodes_the_keys() {
        let yaml = "\
opcua:
  pubsub:
    connections:
      - multicast_addr: \"224.0.0.22:4840\"
        security:
          policy: aes256-gcm
          security_group_id: g1
          token_id: 1
          encrypting_key: \"abababababababababababababababababababababababababababababababab\"
          key_nonce: \"deadbeef\"
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        let sec = cfg.pubsub.connections[0]
            .security
            .as_ref()
            .expect("security config");
        assert_eq!(sec.policy, PubSubSecurityPolicy::Aes256Gcm);
        assert_eq!(sec.security_group_id, "g1");
        assert_eq!(sec.token_id, 1);
        assert!(sec.signing_key.is_empty());
        assert_eq!(sec.encrypting_key.len(), 32);
        assert_eq!(sec.key_nonce, vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn pubsub_security_with_wrong_key_length_is_refused() {
        let yaml = "\
opcua:
  pubsub:
    connections:
      - multicast_addr: \"224.0.0.22:4840\"
        security:
          policy: aes256-gcm
          security_group_id: g1
          encrypting_key: \"deadbeef\"
          key_nonce: \"deadbeef\"
";
        let err = DaemonConfig::load_from_str(yaml).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid { .. }));
    }

    #[test]
    fn pubsub_security_odd_length_hex_is_refused() {
        let yaml = "\
opcua:
  pubsub:
    connections:
      - multicast_addr: \"224.0.0.22:4840\"
        security:
          policy: aes256-gcm
          security_group_id: g1
          encrypting_key: \"abc\"
          key_nonce: \"deadbeef\"
";
        let err = DaemonConfig::load_from_str(yaml).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid { .. }));
    }

    #[test]
    fn pubsub_security_unknown_policy_is_refused() {
        let yaml = "\
opcua:
  pubsub:
    connections:
      - multicast_addr: \"224.0.0.22:4840\"
        security:
          policy: not-a-policy
          security_group_id: g1
";
        let err = DaemonConfig::load_from_str(yaml).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid { .. }));
    }

    #[test]
    fn no_security_key_leaves_it_none() {
        let yaml = "\
opcua:
  pubsub:
    connections:
      - multicast_addr: \"224.0.0.22:4840\"
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert!(cfg.pubsub.connections[0].security.is_none());
    }

    #[test]
    fn otlp_endpoint_parses() {
        let yaml = "metrics:\n  otlp_endpoint: \"http://127.0.0.1:4318\"\n";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert_eq!(
            cfg.metrics_otlp_endpoint,
            Some("http://127.0.0.1:4318".to_string())
        );
    }

    #[test]
    fn no_otlp_endpoint_leaves_it_none() {
        let cfg = DaemonConfig::load_from_str("domain: 0\n").unwrap();
        assert!(cfg.metrics_otlp_endpoint.is_none());
    }

    #[test]
    fn secured_policy_without_certificate_is_refused() {
        let yaml = "\
opcua:
  server:
    security_policies: [\"basic256sha256\"]
";
        let err = DaemonConfig::load_from_str(yaml).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid { .. }));
    }

    #[test]
    fn unknown_security_policy_is_refused() {
        let yaml = "opcua:\n  server:\n    security_policies: [\"not-a-policy\"]\n";
        let err = DaemonConfig::load_from_str(yaml).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid { .. }));
    }

    #[test]
    fn historical_topics_parse() {
        let yaml = "\
opcua:
  server:
    historical:
      topics:
        - dds_topic: Temperature
          depth: 100
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert_eq!(cfg.server.historical.topics.len(), 1);
        assert_eq!(cfg.server.historical.topics[0].dds_topic, "Temperature");
        assert_eq!(cfg.server.historical.topics[0].depth, 100);
    }

    #[test]
    fn historical_topic_without_depth_is_refused() {
        let yaml = "\
opcua:
  server:
    historical:
      topics:
        - dds_topic: Temperature
";
        let err = DaemonConfig::load_from_str(yaml).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid { .. }));
    }

    #[test]
    fn historical_topic_zero_depth_is_refused() {
        let yaml = "\
opcua:
  server:
    historical:
      topics:
        - dds_topic: Temperature
          depth: 0
";
        let err = DaemonConfig::load_from_str(yaml).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid { .. }));
    }

    #[test]
    fn acl_users_parse_with_default_policy_id() {
        let yaml = "\
acl:
  allow_anonymous: false
  users:
    - username: alice
      password: s3cret
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert!(!cfg.acl_allow_anonymous);
        assert_eq!(cfg.acl_users.len(), 1);
        assert_eq!(cfg.acl_users[0].username, "alice");
        assert_eq!(cfg.acl_users[0].password, "s3cret");
        assert_eq!(cfg.acl_users[0].policy_id, "user");
    }

    #[test]
    fn acl_user_without_password_is_refused() {
        let yaml = "acl:\n  users:\n    - username: alice\n";
        let err = DaemonConfig::load_from_str(yaml).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid { .. }));
    }

    #[test]
    fn acl_rules_parse() {
        let yaml = "\
acl:
  default_deny: true
  rules:
    - subject: \"opc-client-1\"
      allow_read: [\"Telemetry\"]
      allow_write: [\"Commands\"]
";
        let cfg = DaemonConfig::load_from_str(yaml).unwrap();
        assert!(cfg.acl_default_deny);
        assert_eq!(cfg.acl_rules[0].0, "opc-client-1");
        assert_eq!(cfg.acl_rules[0].1, vec!["Telemetry".to_string()]);
        assert_eq!(cfg.acl_rules[0].2, vec!["Commands".to_string()]);
    }
}
