// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The configuration (spec §3): the keys, the mappings, the targets.

#[allow(unused_imports)]
use super::prelude::*;

/// Direction of a mapping (§3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// GIOP request → DDS request topic, DDS reply topic → GIOP reply.
    CorbaToDds,
    /// DDS notify topic → oneway GIOP request.
    DdsToCorba,
    /// Both.
    Bidir,
}

/// A DDS topic of a mapping.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TopicRef {
    /// Topic name.
    pub dds_name: String,
    /// Type name; defaults to the topic name.
    pub dds_type: String,
}

/// One `mappings:` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    /// CORBA repository id of the interface.
    pub repo_id: String,
    /// The operation.
    pub operation: String,
    /// Direction.
    pub direction: Direction,
    /// `request_topic` (corba_to_dds).
    pub request_topic: TopicRef,
    /// `reply_topic` (corba_to_dds).
    pub reply_topic: TopicRef,
    /// `notify_topic` (dds_to_corba).
    pub notify_topic: TopicRef,
    /// Reply wait (corba_to_dds).
    pub timeout_ms: u64,
    /// `qos.reliability`: `true` = RELIABLE.
    pub reliable: bool,
    /// `qos.durability`.
    pub durability: zerodds_qos::DurabilityKind,
    /// `qos.history`: `Some(depth)` for KEEP_LAST, `None` for KEEP_ALL.
    pub history_depth: Option<u32>,
    /// `qos.lifespan_ms`: the lifespan of the request and notify samples,
    /// and of a reply in the replay store (§6).
    pub lifespan_ms: Option<u64>,
    /// `qos.deadline_ms`: offered by the request writer, requested by the
    /// reply and notify readers (§6).
    pub deadline_ms: Option<u64>,
    /// `qos.liveliness_lease_ms`: the lease of the DDS endpoints.
    pub liveliness_lease_ms: Option<u64>,
    /// `qos.partition`: the partitions of the DDS endpoints.
    pub partition: Vec<String>,
    /// `targets` (dds_to_corba): `corbaloc:` URLs or stringified IORs.
    pub targets: Vec<String>,
    /// `idl_file`: the IDL of the interface; the types of the topics are
    /// generated from it (§5.2).
    pub idl_file: Option<String>,
    /// `correlation_id`: the name of the first member of the generated
    /// request and reply types (§4.3 step 5).
    pub correlation_id: String,
    /// The exceptions the operation raises, by `exception_kind` (§5.2);
    /// from `idl_file`.
    pub raises: Vec<super::typegen::Raised>,
}

impl Mapping {
    pub(crate) fn receives_giop(&self) -> bool {
        matches!(self.direction, Direction::CorbaToDds | Direction::Bidir)
    }
    pub(crate) fn sends_giop(&self) -> bool {
        matches!(self.direction, Direction::DdsToCorba | Direction::Bidir)
    }
}

/// An ACL rule (§7.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AclRule {
    /// The subject: the client certificate's subject DN (RFC 4514, e.g.
    /// `CN=trader,O=Example`), its fingerprint `mtls:<SHA-256 hex>`, or `*`.
    pub subject: String,
    /// Repository id patterns; a trailing `*` matches a prefix.
    pub allow_invoke: Vec<String>,
}

/// The daemon's settings.
#[derive(Debug, Clone, PartialEq)]
pub struct DaemonConfig {
    /// DDS domain.
    pub domain: i32,
    /// `log_level`.
    pub log_level: String,
    /// `corba.orb_id`.
    pub orb_id: String,
    /// `corba.iiop.bind`.
    pub iiop_bind: String,
    /// `corba.iiop.advertise_host`: the host the IORs name when the bind
    /// address is a wildcard.
    pub iiop_advertise_host: String,
    /// `corba.iiop.giop_version`: the version of the replies and IORs.
    pub giop_version: Version,
    /// `corba.iiop.fragment_size`: replies above it go out in fragments.
    pub fragment_size: usize,
    /// `corba.iiop.max_message_size`.
    pub max_message_size: usize,
    /// `corba.ssliop.enabled`.
    pub ssliop_enabled: bool,
    /// `corba.ssliop.bind`.
    pub ssliop_bind: String,
    /// `corba.ssliop.cert_file`.
    pub ssliop_cert_file: String,
    /// `corba.ssliop.key_file`.
    pub ssliop_key_file: String,
    /// `corba.ssliop.ca_file`: the CA client certificates are checked against.
    pub ssliop_ca_file: String,
    /// `corba.ssliop.require_client_cert`.
    pub ssliop_require_client_cert: bool,
    /// `corba.ssliop.required`: the objects are reached over SSLIOP only;
    /// the IOR's IIOP port is 0 and the plain IIOP listener forwards
    /// (`OBJECT_FORWARD`, `LOCATION_FORWARD`, §4.7).
    pub ssliop_required: bool,
    /// `corba.ior.ior_file`: the stringified IORs are written here.
    pub ior_file: String,
    /// `corba.idl_dir`: the IDL generated from the `idl_file` of the
    /// mappings is written here (§5.2).
    pub idl_dir: String,
    /// `corba.ior.naming_service`: a `corbaname:`/`corbaloc:` URL the
    /// daemon resolves at start to rebind every mapping's servant into,
    /// and unbinds at a clean stop.
    pub naming_service: String,
    /// `corba.ior.publish_to_naming`.
    pub publish_to_naming: bool,
    /// `corba.ior.naming_retry_ms`: total budget the daemon retries
    /// connecting to `naming_service` for, with exponential backoff,
    /// before aborting the start. Default 30000 (30s) — the same
    /// window this daemon already gives its own shutdown drain
    /// (`DRAIN_LIMIT`) and its systemd unit's `TimeoutStopSec`, so an
    /// operator who has already sized one against their NameService's
    /// typical boot time does not have to size a second, unrelated one.
    pub naming_retry_ms: u64,
    /// `corba.ior.naming_ssliop.ca_file`: trust root the NameService's
    /// own server certificate is checked against, when `naming_service`
    /// names an `ssliop:`/`tls:` endpoint.
    pub naming_ssliop_ca_file: String,
    /// `corba.ior.naming_ssliop.cert_file`: a client certificate this
    /// daemon presents to the NameService (needed only if it requires
    /// one — SSLIOP `EstablishTrustInClient`). Empty means none.
    pub naming_ssliop_cert_file: String,
    /// `corba.ior.naming_ssliop.key_file`.
    pub naming_ssliop_key_file: String,
    /// `corba.ior.initial_references`: name -> `corbaname:`/`corbaloc:`
    /// URL, resolved when `naming_service` (or a name it is federated
    /// through) uses the `rir:` protocol — the standard
    /// `resolve_initial_references` indirection (CORBA 3.4 Part 2
    /// §13.6.10.1; the same role `-ORBInitRef NAME=...` plays for
    /// omniORB/TAO and `ORBInitRef.NAME=...` for JacORB). This daemon is
    /// not a full ORB with its own initial-reference table, so `rir:`
    /// only resolves for a name configured here.
    pub naming_initial_references: BTreeMap<String, String>,
    /// `corba.naming.serve`: host a `zerodds-corba-cosnaming` NamingContext
    /// (object key `NameService`) on this daemon's own IIOP/SSLIOP
    /// listener, sharing its transport and the servant code with
    /// `zerodds-naming-svc`.
    pub naming_serve: bool,
    /// `corba.naming.persist_file`: same option as `zerodds-naming-svc`'s
    /// `naming.persist_file` -- the graph is loaded from here at start
    /// (if the file exists) and saved back after every mutating
    /// operation and on a clean stop. Empty means no persistence (an
    /// ephemeral, in-memory graph). Only meaningful with
    /// `corba.naming.serve`.
    pub naming_persist_file: String,
    /// `corba.csiv2.enabled`.
    pub csiv2_enabled: bool,
    /// `corba.csiv2.target_supports`: accepted `IdentityToken` forms
    /// for CSIv2 identity assertion, as an `IDENTITY_TYPE_*` mask.
    pub csiv2_target_supports: u32,
    /// `corba.csiv2.gssup.password_file`.
    pub csiv2_password_file: String,
    /// `corba.csiv2.gssup.target_name`: the AS-context target name this
    /// daemon's `TAG_CSI_SEC_MECH_LIST` IOR component advertises.
    pub csiv2_target_name: String,
    /// `acl.default_deny`.
    pub acl_default_deny: bool,
    /// `acl.rules`.
    pub acl_rules: Vec<AclRule>,
    /// `metrics.enabled`.
    pub metrics_enabled: bool,
    /// `metrics.listen`.
    pub metrics_listen: String,
    /// Mappings.
    pub mappings: Vec<Mapping>,
    /// §1 L6: the participants beside the one of `domain`.
    pub tenants: Vec<TenantConfig>,
}

/// A `tenants:` entry (§1 L6): a DomainParticipant of its own on `domain`
/// with its mappings; an SSLIOP session whose client certificate names one
/// of `subjects` (its DN or `mtls:<SHA-256>`) is bound to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantConfig {
    /// Its name, in the IOR lines, the log, the spans and the metrics.
    pub name: String,
    /// Its DDS domain.
    pub domain: i32,
    /// The certificate subjects bound to it.
    pub subjects: Vec<String>,
    /// Its mappings.
    pub mappings: Vec<Mapping>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            domain: 0,
            log_level: "info".into(),
            orb_id: "zerodds-corba-bridge".into(),
            iiop_bind: "0.0.0.0:6833".into(),
            iiop_advertise_host: "127.0.0.1".into(),
            giop_version: Version::V1_2,
            fragment_size: 65_536,
            max_message_size: 32 * 1024 * 1024,
            ssliop_enabled: false,
            ssliop_bind: "0.0.0.0:6834".into(),
            ssliop_cert_file: String::new(),
            ssliop_key_file: String::new(),
            ssliop_ca_file: String::new(),
            ssliop_require_client_cert: false,
            ssliop_required: false,
            ior_file: String::new(),
            idl_dir: "/var/lib/zerodds/corba-bridge".into(),
            naming_service: String::new(),
            publish_to_naming: false,
            naming_retry_ms: 30_000,
            naming_ssliop_ca_file: String::new(),
            naming_ssliop_cert_file: String::new(),
            naming_ssliop_key_file: String::new(),
            naming_initial_references: BTreeMap::new(),
            naming_serve: false,
            naming_persist_file: String::new(),
            csiv2_enabled: false,
            csiv2_target_supports: 0,
            csiv2_password_file: String::new(),
            csiv2_target_name: String::new(),
            acl_default_deny: false,
            acl_rules: Vec::new(),
            metrics_enabled: false,
            metrics_listen: String::new(),
            mappings: Vec::new(),
            tenants: Vec::new(),
        }
    }
}

/// Every key the daemon reads (§3); lists stop at the list.
pub const KEYS: &[&str] = &[
    "domain",
    "log_level",
    "corba.orb_id",
    "corba.idl_dir",
    "corba.iiop.bind",
    "corba.iiop.advertise_host",
    "corba.iiop.giop_version",
    "corba.iiop.fragment_size",
    "corba.iiop.max_message_size",
    "corba.ssliop.enabled",
    "corba.ssliop.bind",
    "corba.ssliop.cert_file",
    "corba.ssliop.key_file",
    "corba.ssliop.ca_file",
    "corba.ssliop.require_client_cert",
    "corba.ssliop.required",
    "corba.ior.naming_service",
    "corba.ior.publish_to_naming",
    "corba.ior.ior_file",
    "corba.ior.naming_retry_ms",
    "corba.ior.naming_ssliop.ca_file",
    "corba.ior.naming_ssliop.cert_file",
    "corba.ior.naming_ssliop.key_file",
    "corba.ior.initial_references",
    "corba.naming.serve",
    "corba.naming.persist_file",
    "corba.csiv2.enabled",
    "corba.csiv2.target_supports",
    "corba.csiv2.gssup.password_file",
    "corba.csiv2.gssup.target_name",
    "acl.default_deny",
    "acl.rules",
    "metrics.enabled",
    "metrics.listen",
    "metrics.path",
    "mappings",
    "tenants",
];

/// Environment variables the daemon reads as settings (the env layer).
pub const ENV: &[(&str, &str)] = &[("ZERODDS_CORBA_ADVERTISE_HOST", "corba.iiop.advertise_host")];

/// Whether `path` is a key [`DaemonConfig::from_config`] reads.
#[must_use]
pub fn is_key(path: &str) -> bool {
    // `corba.ior.initial_references` is a map with operator-chosen keys
    // (a reference name, e.g. `NameService`), so `unknown_keys`'s walk
    // renders each entry as its own dotted leaf
    // (`corba.ior.initial_references.NameService`) rather than the bare
    // section path `KEYS` names — accepted by prefix rather than listed
    // one name at a time, the same way `mappings`' sequence entries are
    // already exempt (`unknown_keys` collapses a `[N]` path on its own;
    // a map has no bracket to collapse).
    KEYS.contains(&path) || path.starts_with("corba.ior.initial_references.")
}

pub(crate) fn invalid(
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

/// §5.1: `IDL:zerodds/MarketData/Quote:1.0` + `request_quote` →
/// `MarketData::Quote::request_quote`.
pub(crate) fn slug(repo_id: &str, operation: &str) -> String {
    let body = repo_id
        .strip_prefix("IDL:")
        .unwrap_or(repo_id)
        .rsplit_once(':')
        .map_or(repo_id, |(b, _)| b);
    let parts: Vec<&str> = body.split('/').collect();
    // The first segment is the prefix pragma (`zerodds`), not a module.
    let modules = if parts.len() > 1 {
        &parts[1..]
    } else {
        &parts[..]
    };
    format!("{}::{operation}", modules.join("::"))
}

pub(crate) fn read_topic_ref(
    section: &Section<'_>,
    fallback: String,
) -> Result<TopicRef, ConfigError> {
    section.only(&["dds_name", "dds_type"])?;
    let dds_name = section.string("dds_name")?.unwrap_or(fallback);
    let dds_type = section
        .string("dds_type")?
        .unwrap_or_else(|| dds_name.clone());
    Ok(TopicRef { dds_name, dds_type })
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
        let corba = root.section("corba")?;
        corba.only(&[
            "orb_id", "idl_dir", "iiop", "ssliop", "ior", "naming", "csiv2",
        ])?;
        if let Some(v) = corba.string("orb_id")? {
            cfg.orb_id = v;
        }
        if let Some(v) = corba.string("idl_dir")? {
            cfg.idl_dir = v;
        }
        let iiop = corba.section("iiop")?;
        iiop.only(&[
            "bind",
            "advertise_host",
            "giop_version",
            "fragment_size",
            "max_message_size",
        ])?;
        if let Some(v) = iiop.string("bind")? {
            cfg.iiop_bind = v;
        }
        if let Some(v) = iiop.string("advertise_host")? {
            cfg.iiop_advertise_host = v;
        }
        if let Some(v) = iiop.choice("giop_version", &["1.0", "1.1", "1.2"])? {
            cfg.giop_version = match v.as_str() {
                "1.0" => Version::V1_0,
                "1.1" => Version::V1_1,
                _ => Version::V1_2,
            };
        }
        if let Some((v, origin)) = iiop.scalar("fragment_size")? {
            cfg.fragment_size = v
                .parse::<usize>()
                .ok()
                .filter(|n| *n >= 64)
                .ok_or_else(|| {
                    invalid(
                        "corba.iiop.fragment_size",
                        origin.clone(),
                        "bytes, at least 64",
                    )
                })?;
        }
        if let Some((v, origin)) = iiop.scalar("max_message_size")? {
            cfg.max_message_size =
                v.parse::<usize>()
                    .ok()
                    .filter(|n| *n >= 1024)
                    .ok_or_else(|| {
                        invalid(
                            "corba.iiop.max_message_size",
                            origin.clone(),
                            "bytes, at least 1024",
                        )
                    })?;
        }
        let ssl = corba.section("ssliop")?;
        ssl.only(&[
            "enabled",
            "bind",
            "cert_file",
            "key_file",
            "ca_file",
            "require_client_cert",
            "required",
        ])?;
        if let Some(v) = ssl.boolean("enabled")? {
            cfg.ssliop_enabled = v;
        }
        if let Some(v) = ssl.string("bind")? {
            cfg.ssliop_bind = v;
        }
        if let Some(v) = ssl.string("cert_file")? {
            cfg.ssliop_cert_file = v;
        }
        if let Some(v) = ssl.string("key_file")? {
            cfg.ssliop_key_file = v;
        }
        if let Some(v) = ssl.string("ca_file")? {
            cfg.ssliop_ca_file = v;
        }
        if let Some(v) = ssl.boolean("require_client_cert")? {
            cfg.ssliop_require_client_cert = v;
        }
        if let Some(v) = ssl.boolean("required")? {
            cfg.ssliop_required = v;
        }
        let ior = corba.section("ior")?;
        ior.only(&[
            "naming_service",
            "publish_to_naming",
            "ior_file",
            "naming_retry_ms",
            "naming_ssliop",
            "initial_references",
        ])?;
        if let Some(v) = ior.string("naming_service")? {
            cfg.naming_service = v;
        }
        if let Some(v) = ior.string("ior_file")? {
            cfg.ior_file = v;
        }
        if let Some(v) = ior.boolean("publish_to_naming")? {
            cfg.publish_to_naming = v;
        }
        if let Some(v) = ior.parse::<u64>("naming_retry_ms", "milliseconds")? {
            cfg.naming_retry_ms = v;
        }
        let naming_ssliop = ior.section("naming_ssliop")?;
        naming_ssliop.only(&["ca_file", "cert_file", "key_file"])?;
        if let Some(v) = naming_ssliop.string("ca_file")? {
            cfg.naming_ssliop_ca_file = v;
        }
        if let Some(v) = naming_ssliop.string("cert_file")? {
            cfg.naming_ssliop_cert_file = v;
        }
        if let Some(v) = naming_ssliop.string("key_file")? {
            cfg.naming_ssliop_key_file = v;
        }
        if cfg.naming_ssliop_cert_file.is_empty() != cfg.naming_ssliop_key_file.is_empty() {
            return Err(invalid(
                "corba.ior.naming_ssliop",
                naming_ssliop
                    .node("cert_file")
                    .or_else(|| naming_ssliop.node("key_file"))
                    .map_or(zerodds_config::Origin::Default, first_origin),
                "cert_file and key_file must be set together",
            ));
        }
        if let Some(node) = ior.node("initial_references") {
            let zerodds_config::Node::Map(map) = node else {
                return Err(invalid(
                    "corba.ior.initial_references",
                    first_origin(node),
                    "expected a map of name to a corbaname:/corbaloc: URL",
                ));
            };
            for (name, value) in map {
                let Some(url) = value.as_str() else {
                    return Err(invalid(
                        format!("corba.ior.initial_references.{name}"),
                        first_origin(value),
                        "expected a corbaname:/corbaloc: URL string",
                    ));
                };
                cfg.naming_initial_references
                    .insert(name.clone(), url.to_string());
            }
        }
        if cfg.publish_to_naming && cfg.naming_service.is_empty() {
            return Err(invalid(
                "corba.ior.publish_to_naming",
                ior.node("publish_to_naming")
                    .map_or(zerodds_config::Origin::Default, first_origin),
                "publish_to_naming needs corba.ior.naming_service",
            ));
        }
        let naming = corba.section("naming")?;
        naming.only(&["serve", "persist_file"])?;
        if let Some(v) = naming.boolean("serve")? {
            cfg.naming_serve = v;
        }
        if let Some(v) = naming.string("persist_file")? {
            cfg.naming_persist_file = v;
        }
        let csiv2 = corba.section("csiv2")?;
        csiv2.only(&["enabled", "target_supports", "gssup"])?;
        if let Some(v) = csiv2.boolean("enabled")? {
            cfg.csiv2_enabled = v;
        }
        for (path, item) in csiv2.items("target_supports")? {
            let s = item.as_str().unwrap_or_default();
            let bit = match s {
                "anonymous" => zerodds_corba_csiv2::IDENTITY_TYPE_ANONYMOUS,
                "principal_name" => zerodds_corba_csiv2::IDENTITY_TYPE_PRINCIPAL_NAME,
                "x509_cert_chain" => zerodds_corba_csiv2::IDENTITY_TYPE_X509_CERT_CHAIN,
                "distinguished_name" => zerodds_corba_csiv2::IDENTITY_TYPE_DISTINGUISHED_NAME,
                _ => {
                    return Err(invalid(
                        path.clone(),
                        first_origin(item),
                        "expected one of: anonymous, principal_name, x509_cert_chain, distinguished_name",
                    ));
                }
            };
            cfg.csiv2_target_supports |= bit;
        }
        let gssup = csiv2.section("gssup")?;
        gssup.only(&["password_file", "target_name"])?;
        if let Some(v) = gssup.string("password_file")? {
            cfg.csiv2_password_file = v;
        }
        if let Some(v) = gssup.string("target_name")? {
            cfg.csiv2_target_name = v;
        }
        if cfg.csiv2_enabled && cfg.csiv2_password_file.is_empty() {
            return Err(invalid(
                "corba.csiv2.enabled",
                csiv2
                    .node("enabled")
                    .map_or(zerodds_config::Origin::Default, first_origin),
                "csiv2.enabled needs corba.csiv2.gssup.password_file",
            ));
        }
        let acl = root.section("acl")?;
        acl.only(&["default_deny", "rules"])?;
        if let Some(v) = acl.boolean("default_deny")? {
            cfg.acl_default_deny = v;
        }
        for (path, item) in acl.items("rules")? {
            let e = Section::of(Some(item), path.clone())?;
            e.only(&["subject", "allow_invoke"])?;
            let subject = e.string("subject")?.ok_or_else(|| {
                invalid(
                    format!("{path}.subject"),
                    first_origin(item),
                    "an ACL rule names its subject",
                )
            })?;
            let allow_invoke = e
                .items("allow_invoke")?
                .into_iter()
                .filter_map(|(_, n)| n.as_str().map(str::to_string))
                .collect();
            cfg.acl_rules.push(AclRule {
                subject,
                allow_invoke,
            });
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
        for (path, item) in root.items("mappings")? {
            cfg.mappings.push(read_mapping(&path, item)?);
        }
        for (path, item) in root.items("tenants")? {
            let e = Section::of(Some(item), path.clone())?;
            e.only(&["name", "domain", "subjects", "mappings"])?;
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
            let subjects: Vec<String> = e
                .items("subjects")?
                .into_iter()
                .filter_map(|(_, n)| n.as_str().map(str::to_string))
                .collect();
            if subjects.is_empty() {
                return Err(invalid(
                    format!("{path}.subjects"),
                    first_origin(item),
                    "a tenant needs the certificate subjects bound to it",
                ));
            }
            let mut mappings = Vec::new();
            for (mpath, m) in e.items("mappings")? {
                mappings.push(read_mapping(&mpath, m)?);
            }
            cfg.tenants.push(TenantConfig {
                name,
                domain,
                subjects,
                mappings,
            });
        }
        let mut names = std::collections::BTreeSet::new();
        let mut bound = std::collections::BTreeMap::new();
        for t in &cfg.tenants {
            if !names.insert(t.name.clone()) {
                return Err(invalid(
                    "tenants",
                    zerodds_config::Origin::Default,
                    format!("the tenant {} is named twice", t.name),
                ));
            }
            for s in &t.subjects {
                if let Some(other) = bound.insert(s.clone(), t.name.clone()) {
                    return Err(invalid(
                        "tenants",
                        zerodds_config::Origin::Default,
                        format!("the subject {s} is bound to {other} and {}", t.name),
                    ));
                }
            }
        }
        check_mappings(&cfg.mappings)?;
        for t in &cfg.tenants {
            check_mappings(&t.mappings)?;
        }
        // §5.2: the types of the mappings that name their IDL.
        let idl = |e: String| invalid("mappings", zerodds_config::Origin::Default, e);
        super::typegen::apply(&mut cfg.mappings).map_err(idl)?;
        for t in &mut cfg.tenants {
            super::typegen::apply(&mut t.mappings).map_err(idl)?;
        }
        super::typegen::files(&cfg).map_err(idl)?;
        if cfg.ssliop_enabled && (cfg.ssliop_cert_file.is_empty() || cfg.ssliop_key_file.is_empty())
        {
            return Err(invalid(
                "corba.ssliop",
                zerodds_config::Origin::Default,
                "SSLIOP needs cert_file and key_file",
            ));
        }
        if cfg.ssliop_require_client_cert && cfg.ssliop_ca_file.is_empty() {
            return Err(invalid(
                "corba.ssliop.require_client_cert",
                zerodds_config::Origin::Default,
                "client certificates are checked against corba.ssliop.ca_file, which is empty",
            ));
        }
        if cfg.ssliop_required && !cfg.ssliop_enabled {
            return Err(invalid(
                "corba.ssliop.required",
                zerodds_config::Origin::Default,
                "SSLIOP only needs corba.ssliop.enabled",
            ));
        }
        Ok(cfg)
    }

    /// The configuration of tenant `t`'s participant: this one on its
    /// domain with its mappings.
    #[must_use]
    pub fn of_tenant(&self, t: &TenantConfig) -> Self {
        Self {
            domain: t.domain,
            mappings: t.mappings.clone(),
            tenants: Vec::new(),
            ..self.clone()
        }
    }
}

/// A repo_id and operation is mapped once; and since the key is of repo_id
/// and topics (§4.5), two mappings with both equal would be one object.
fn check_mappings(mappings: &[Mapping]) -> Result<(), ConfigError> {
    let mut seen = std::collections::BTreeSet::new();
    for m in mappings {
        if !seen.insert((m.repo_id.clone(), m.operation.clone())) {
            return Err(invalid(
                "mappings",
                zerodds_config::Origin::Default,
                format!("{} {} is mapped twice", m.repo_id, m.operation),
            ));
        }
    }
    let mut keys = std::collections::BTreeMap::new();
    for m in mappings.iter().filter(|m| m.receives_giop()) {
        if let Some(other) = keys.insert(super::object_key(m), m.operation.clone()) {
            return Err(invalid(
                "mappings",
                zerodds_config::Origin::Default,
                format!(
                    "{} {} and {other} have the same repo_id and topics, so the same object key",
                    m.repo_id, m.operation
                ),
            ));
        }
    }
    Ok(())
}

pub(crate) fn read_mapping(
    path: &str,
    item: &zerodds_config::Node,
) -> Result<Mapping, ConfigError> {
    let mut m = Mapping {
        repo_id: String::new(),
        operation: "invoke".into(),
        direction: Direction::CorbaToDds,
        request_topic: TopicRef::default(),
        reply_topic: TopicRef::default(),
        notify_topic: TopicRef::default(),
        timeout_ms: 5_000,
        reliable: true,
        durability: zerodds_qos::DurabilityKind::Volatile,
        history_depth: Some(1),
        lifespan_ms: None,
        deadline_ms: None,
        liveliness_lease_ms: None,
        partition: Vec::new(),
        targets: Vec::new(),
        idl_file: None,
        correlation_id: "request_id".into(),
        raises: Vec::new(),
    };
    if let zerodds_config::Node::Scalar(text, origin) = item {
        // `--topic REPO_ID=TOPIC`: TOPIC and TOPIC::Reply.
        let (repo, topic) = text.rsplit_once('=').ok_or_else(|| {
            invalid(
                path,
                origin.clone(),
                format!("got `{text}`, expected REPO_ID=TOPIC or a map"),
            )
        })?;
        m.repo_id = repo.trim().to_string();
        let topic = topic.trim().to_string();
        m.request_topic = TopicRef {
            dds_name: topic.clone(),
            dds_type: topic.clone(),
        };
        m.reply_topic = TopicRef {
            dds_name: format!("{topic}::Reply"),
            dds_type: format!("{topic}::Reply"),
        };
    } else {
        let e = Section::of(Some(item), path.to_string())?;
        e.only(&[
            "repo_id",
            "operation",
            "direction",
            "request_topic",
            "reply_topic",
            "notify_topic",
            "correlation_id",
            "idl_file",
            "timeout_ms",
            "qos",
            "targets",
            "target_object_keys",
        ])?;
        m.repo_id = e.string("repo_id")?.unwrap_or_default();
        if let Some(v) = e.string("operation")? {
            m.operation = v;
        }
        if let Some(v) = e.choice("direction", &["corba_to_dds", "dds_to_corba", "bidir"])? {
            m.direction = match v.as_str() {
                "dds_to_corba" => Direction::DdsToCorba,
                "bidir" => Direction::Bidir,
                _ => Direction::CorbaToDds,
            };
        }
        let base = slug(&m.repo_id, &m.operation);
        m.request_topic = read_topic_ref(&e.section("request_topic")?, format!("{base}::Request"))?;
        m.reply_topic = read_topic_ref(&e.section("reply_topic")?, format!("{base}::Reply"))?;
        m.notify_topic = read_topic_ref(&e.section("notify_topic")?, format!("{base}::Notify"))?;
        m.idl_file = e.string("idl_file")?.filter(|f| !f.is_empty());
        if m.idl_file.is_some() {
            // The types are the generated ones.
            for topic in ["request_topic", "reply_topic", "notify_topic"] {
                if let Some((_, origin)) = e.section(topic)?.scalar("dds_type")? {
                    return Err(invalid(
                        format!("{path}.{topic}.dds_type"),
                        origin.clone(),
                        "with idl_file the type is the one generated from the IDL",
                    ));
                }
            }
        }
        if let Some((v, origin)) = e.scalar("correlation_id")? {
            if v != "request_id" && m.idl_file.is_none() {
                return Err(invalid(
                    format!("{path}.correlation_id"),
                    origin.clone(),
                    "the correlation field is the first member, request_id; another name needs the type, which idl_file gives the bridge",
                ));
            }
            m.correlation_id = v.to_string();
        }
        if let Some(v) = e.parse::<u64>("timeout_ms", "milliseconds")? {
            m.timeout_ms = v;
        }
        let qos = e.section("qos")?;
        qos.only(&[
            "reliability",
            "durability",
            "history",
            "lifespan_ms",
            "deadline_ms",
            "liveliness_lease_ms",
            "partition",
        ])?;
        if let Some(v) = qos.choice("reliability", &["reliable", "best_effort"])? {
            m.reliable = v == "reliable";
        }
        if let Some(v) = qos.choice("durability", &["volatile", "transient_local"])? {
            m.durability = if v == "transient_local" {
                zerodds_qos::DurabilityKind::TransientLocal
            } else {
                zerodds_qos::DurabilityKind::Volatile
            };
        }
        let history = qos.section("history")?;
        history.only(&["kind", "depth"])?;
        let keep_all = history
            .choice("kind", &["keep_last", "keep_all"])?
            .as_deref()
            == Some("keep_all");
        if let Some((v, origin)) = history.scalar("depth")? {
            let depth = v.parse::<u32>().ok().filter(|d| *d >= 1).ok_or_else(|| {
                invalid(
                    format!("{path}.qos.history.depth"),
                    origin.clone(),
                    "a depth of at least 1",
                )
            })?;
            m.history_depth = Some(depth);
        }
        if keep_all {
            m.history_depth = None;
        }
        let positive = |key: &str| -> Result<Option<u64>, ConfigError> {
            match qos.scalar(key)? {
                None => Ok(None),
                Some((v, origin)) => v
                    .parse::<u64>()
                    .ok()
                    .filter(|n| *n > 0)
                    .map(Some)
                    .ok_or_else(|| {
                        invalid(
                            format!("{path}.qos.{key}"),
                            origin.clone(),
                            "milliseconds, more than 0",
                        )
                    }),
            }
        };
        m.lifespan_ms = positive("lifespan_ms")?;
        m.deadline_ms = positive("deadline_ms")?;
        m.liveliness_lease_ms = positive("liveliness_lease_ms")?;
        m.partition = qos
            .items("partition")?
            .into_iter()
            .filter_map(|(_, n)| n.as_str().map(str::to_string))
            .collect();
        if m.durability == zerodds_qos::DurabilityKind::TransientLocal
            && m.history_depth.is_none()
            && m.lifespan_ms.is_none()
        {
            return Err(invalid(
                format!("{path}.qos.history.kind"),
                first_origin(item),
                "transient_local with keep_all keeps every reply; it needs qos.lifespan_ms to bound the replay store",
            ));
        }
        for (tpath, t) in e.items("targets")? {
            let text = t.as_str().unwrap_or_default().to_string();
            resolve_target(&text).map_err(|msg| invalid(tpath, first_origin(t), msg))?;
            m.targets.push(text);
        }
        if let Some(node) = e.node("target_object_keys") {
            return Err(invalid(
                format!("{path}.target_object_keys"),
                first_origin(node),
                "an object key names no host; list the targets as corbaloc: URLs or IORs in `targets`",
            ));
        }
    }
    if m.repo_id.is_empty() {
        return Err(invalid(
            format!("{path}.repo_id"),
            first_origin(item),
            "a mapping needs a repo_id",
        ));
    }
    if m.sends_giop() && m.targets.is_empty() {
        return Err(invalid(
            format!("{path}.targets"),
            first_origin(item),
            "dds_to_corba needs at least one target",
        ));
    }
    if m.receives_giop() && !m.reliable {
        return Err(invalid(
            format!("{path}.qos.reliability"),
            first_origin(item),
            "request-reply needs RELIABLE (spec §6); best_effort is for dds_to_corba",
        ));
    }
    Ok(m)
}

/// A notify target: host, port, object key, GIOP version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// Host.
    pub host: String,
    /// Port.
    pub port: u16,
    /// Object key.
    pub object_key: Vec<u8>,
}

/// Resolves a `corbaloc:iiop:` URL or a stringified IOR to its first IIOP
/// endpoint. The schemes `corbaloc:` and `IOR:` are matched without regard
/// to case (CORBA 3.3 Part 2 §7.6.9, §7.6.10).
///
/// # Errors
/// What the text lacks.
pub fn resolve_target(text: &str) -> Result<Target, String> {
    let has_scheme = |scheme: &str| {
        text.get(..scheme.len())
            .is_some_and(|s| s.eq_ignore_ascii_case(scheme))
    };
    if has_scheme("corbaloc:") {
        let loc = parse_corbaloc(text).map_err(|e| format!("{text}: {e:?}"))?;
        let ep = loc
            .iiop_addresses()
            .next()
            .ok_or_else(|| format!("{text}: no iiop endpoint"))?;
        return Ok(Target {
            host: ep.host.clone(),
            port: ep.port,
            object_key: loc.object_key,
        });
    }
    if has_scheme("IOR:") {
        let ior = from_stringified(text).map_err(|e| format!("IOR: {e:?}"))?;
        for p in &ior.profiles {
            if let Some(Ok(body)) = p.as_iiop() {
                return Ok(Target {
                    host: body.host,
                    port: body.port,
                    object_key: body.object_key,
                });
            }
        }
        return Err("the IOR has no IIOP profile".into());
    }
    Err(format!("`{text}` is neither a corbaloc: URL nor an IOR"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn load(yaml: &str) -> Result<DaemonConfig, String> {
        let mut c = zerodds_config::Config::default();
        c.push(
            zerodds_config::parse_str(yaml, "<string>").map_err(|e| e.to_string())?,
            "<string>".into(),
        )
        .map_err(|e| e.to_string())?;
        DaemonConfig::from_config(Some(&c)).map_err(|e| e.to_string())
    }

    /// §5.2 and §4.3 step 5: `idl_file` gives a mapping the generated
    /// types and the exceptions of its operation, and lets
    /// `correlation_id` name the first member; without it only
    /// `request_id` is accepted, with it no `dds_type`.
    #[test]
    fn corbabridge_5_2_idl_file_gives_the_mapping_its_types() {
        let idl = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/omniorb/Div.idl");
        let mapping = format!(
            "mappings:\n  - repo_id: \"IDL:zerodds_test/Divider:1.0\"\n    operation: divide\n    idl_file: \"{idl}\"\n"
        );
        let c = load(&format!("{mapping}    correlation_id: call_id\n")).unwrap();
        let m = &c.mappings[0];
        assert_eq!(
            m.request_topic.dds_type,
            "zerodds_test::Divider::divide::Request"
        );
        assert_eq!(
            m.reply_topic.dds_type,
            "zerodds_test::Divider::divide::Reply"
        );
        assert_eq!(m.request_topic.dds_name, "Divider::divide::Request");
        assert_eq!(m.correlation_id, "call_id");
        assert_eq!(
            m.raises
                .iter()
                .map(|r| r.repo_id.as_str())
                .collect::<Vec<_>>(),
            [
                "IDL:zerodds_test/Divider/DivideByZero:1.0",
                "IDL:zerodds_test/Divider/Unsupported:1.0"
            ]
        );
        assert_eq!(c.idl_dir, "/var/lib/zerodds/corba-bridge");
        let files = crate::bridge_daemon::typegen::files(&c).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].0,
            std::path::Path::new("/var/lib/zerodds/corba-bridge/Div.idl")
        );
        assert!(
            files[0].1.contains("unsigned long long call_id;"),
            "{}",
            files[0].1
        );

        let e = load(&format!(
            "{mapping}    request_topic:\n      dds_type: Mine\n"
        ))
        .unwrap_err();
        assert!(
            e.contains("with idl_file the type is the one generated"),
            "{e}"
        );
        let e = load(&mapping.replace("divide", "multiply")).unwrap_err();
        assert!(e.contains("no operation multiply"), "{e}");
        let e = load(&mapping.replace("Div.idl", "Missing.idl")).unwrap_err();
        assert!(e.contains("Missing.idl"), "{e}");
        let e = load("mappings:\n  - repo_id: X\n    correlation_id: call_id\n").unwrap_err();
        assert!(e.contains("idl_file gives the bridge"), "{e}");
        // The generated file may not replace its source.
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/omniorb");
        let e = load(&format!("corba:\n  idl_dir: \"{dir}\"\n{mapping}")).unwrap_err();
        assert!(e.contains("would replace it"), "{e}");
    }

    /// The shipped example, through the same reader.
    /// The example of §3 of the spec, through the same reader.
    #[test]
    fn corbabridge_3_the_example_of_the_spec_loads() {
        let spec = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/specs/zerodds-corba-bridge-1.0.md"
        ))
        .unwrap();
        let section = &spec[spec.find("## §3 ").unwrap()..];
        let start = section.find("```yaml\n").unwrap() + "```yaml\n".len();
        let yaml = &section[start..start + section[start..].find("```").unwrap()];
        let c = load(yaml).unwrap();
        assert_eq!(c.mappings.len(), 2);
        assert_eq!(c.mappings[0].timeout_ms, 5000);
        assert_eq!(c.mappings[1].direction, Direction::DdsToCorba);
        assert_eq!(c.tenants.len(), 1);
        assert_eq!(c.tenants[0].subjects, ["CN=plant-a,O=Example"]);
        assert!(c.ssliop_enabled && !c.ssliop_required);
        assert_eq!(c.csiv2_target_name, "zerodds");
        assert!(c.acl_default_deny);
    }

    /// The QoS keys of §6 and the tenants of §1 L6.
    #[test]
    fn corbabridge_6_the_qos_keys_and_the_tenants_are_read() {
        let c = load(
            "mappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: op\n    qos:\n      durability: transient_local\n      history:\n        kind: keep_last\n        depth: 4\n      lifespan_ms: 500\n      deadline_ms: 200\n      liveliness_lease_ms: 900\n      partition: [a, b]\n\
             tenants:\n  - name: t\n    domain: 9\n    subjects: [\"CN=x\"]\n    mappings:\n      - repo_id: \"IDL:x/Y:1.0\"\n        operation: op\n",
        )
        .unwrap();
        let m = &c.mappings[0];
        assert_eq!(m.history_depth, Some(4));
        assert_eq!(
            (m.lifespan_ms, m.deadline_ms, m.liveliness_lease_ms),
            (Some(500), Some(200), Some(900))
        );
        assert_eq!(m.partition, ["a", "b"]);
        let t = c.of_tenant(&c.tenants[0]);
        assert_eq!((t.domain, t.mappings.len(), t.tenants.len()), (9, 1, 0));
        for (yaml, needle) in [
            (
                "mappings:\n  - repo_id: X\n    qos:\n      durability: transient_local\n      history:\n        kind: keep_all\n",
                "lifespan_ms",
            ),
            (
                "mappings:\n  - repo_id: X\n    qos:\n      deadline_ms: 0\n",
                "more than 0",
            ),
            ("tenants:\n  - name: t\n    domain: 1\n", "subjects"),
            (
                "tenants:\n  - name: t\n    domain: 1\n    subjects: [a]\n  - name: u\n    domain: 2\n    subjects: [a]\n",
                "bound to t and u",
            ),
            (
                "tenants:\n  - name: t\n    domain: 1\n    subjects: [a]\n  - name: t\n    domain: 2\n    subjects: [b]\n",
                "named twice",
            ),
            (
                "corba:\n  ssliop:\n    required: true\n",
                "SSLIOP only needs",
            ),
        ] {
            let e = load(yaml).expect_err(yaml);
            assert!(e.contains(needle), "{yaml}: {e}");
        }
    }

    #[test]
    fn corbabridge_3_the_shipped_example_reads_as_it_says() {
        let c = load(include_str!(
            "../../../../packaging/linux/configs/corba-bridged.yaml.example"
        ))
        .unwrap();
        assert_eq!(c.iiop_bind, "0.0.0.0:6833");
        assert_eq!(c.fragment_size, 65536);
        assert_eq!(c.ior_file, "/var/lib/zerodds/corba-bridge.ior");
        assert_eq!(c.mappings.len(), 2);
        assert_eq!(
            c.mappings[0].reply_topic.dds_type,
            "fin::Portfolio::UpdateReply"
        );
        assert_eq!(c.mappings[0].timeout_ms, 5000);
        assert_eq!(c.mappings[1].direction, Direction::DdsToCorba);
        assert_eq!(
            resolve_target(&c.mappings[1].targets[0]).unwrap().port,
            2809
        );
        assert_eq!(c.acl_rules[0].allow_invoke, vec!["IDL:fin/*".to_string()]);
    }

    #[test]
    fn corbabridge_5_1_the_slug_of_an_operation() {
        assert_eq!(
            slug("IDL:zerodds/MarketData/Quote:1.0", "request_quote"),
            "MarketData::Quote::request_quote"
        );
        let c = load("mappings:\n  - repo_id: \"IDL:zerodds/MarketData/Quote:1.0\"\n    operation: request_quote\n").unwrap();
        assert_eq!(
            c.mappings[0].request_topic.dds_name,
            "MarketData::Quote::request_quote::Request"
        );
        assert_eq!(
            c.mappings[0].reply_topic.dds_name,
            "MarketData::Quote::request_quote::Reply"
        );
    }

    #[test]
    fn corbabridge_3_what_the_daemon_does_not_build_is_refused() {
        for (yaml, needle) in [
            (
                "corba:\n  csiv2:\n    target_supports: [integrity]\n",
                "expected one of",
            ),
            (
                "corba:\n  csiv2:\n    enabled: true\n",
                "corba.csiv2.gssup.password_file",
            ),
            (
                "corba:\n  ior:\n    publish_to_naming: true\n",
                "corba.ior.naming_service",
            ),
            (
                "mappings:\n  - repo_id: X\n    correlation_id: order_id\n",
                "request_id",
            ),
            (
                "mappings:\n  - repo_id: X\n    direction: dds_to_corba\n    target_object_keys: [a]\n",
                "corbaloc",
            ),
            (
                "mappings:\n  - repo_id: X\n    direction: dds_to_corba\n",
                "at least one target",
            ),
            (
                "mappings:\n  - repo_id: X\n    qos:\n      reliability: best_effort\n",
                "RELIABLE",
            ),
            (
                "mappings:\n  - repo_id: X\n    direction: dds_to_corba\n    targets: [\"http://x\"]\n",
                "neither",
            ),
            (
                "mappings:\n  - repo_id: X\n  - repo_id: X\n",
                "mapped twice",
            ),
        ] {
            let e = load(yaml).expect_err(yaml);
            assert!(e.contains(needle), "{yaml}: {e}");
        }
    }

    /// What used to be refused now builds: a `naming_service` URL, a
    /// full CSIv2 section, and `corba.naming.serve`.
    #[test]
    fn naming_service_and_csiv2_now_parse() {
        let c = load(
            "corba:\n\
             \x20 ior:\n\
             \x20   naming_service: \"corbaname::h:2809#NS\"\n\
             \x20   publish_to_naming: true\n\
             \x20   naming_retry_ms: 5000\n\
             \x20 naming:\n\
             \x20   serve: true\n\
             \x20   persist_file: \"/var/lib/zerodds/corba-bridge/naming.txt\"\n\
             \x20 csiv2:\n\
             \x20   enabled: true\n\
             \x20   target_supports: [principal_name, x509_cert_chain]\n\
             \x20   gssup:\n\
             \x20     password_file: \"/etc/zerodds/gssup.passwd\"\n\
             \x20     target_name: \"REALM.LAB\"\n",
        )
        .unwrap();
        assert_eq!(c.naming_service, "corbaname::h:2809#NS");
        assert!(c.publish_to_naming);
        assert_eq!(c.naming_retry_ms, 5000);
        assert!(c.naming_serve);
        assert_eq!(
            c.naming_persist_file,
            "/var/lib/zerodds/corba-bridge/naming.txt"
        );
        assert!(c.csiv2_enabled);
        assert_eq!(
            c.csiv2_target_supports,
            zerodds_corba_csiv2::IDENTITY_TYPE_PRINCIPAL_NAME
                | zerodds_corba_csiv2::IDENTITY_TYPE_X509_CERT_CHAIN
        );
        assert_eq!(c.csiv2_password_file, "/etc/zerodds/gssup.passwd");
        assert_eq!(c.csiv2_target_name, "REALM.LAB");
    }

    #[test]
    fn naming_ssliop_and_initial_references_parse() {
        let c = load(
            "corba:\n\
             \x20 ior:\n\
             \x20   naming_ssliop:\n\
             \x20     ca_file: \"/etc/zerodds/certs/naming-ca.pem\"\n\
             \x20     cert_file: \"/etc/zerodds/certs/bridge-client.crt\"\n\
             \x20     key_file: \"/etc/zerodds/certs/bridge-client.key\"\n\
             \x20   initial_references:\n\
             \x20     NameService: \"corbaloc::ns.example:2809/NameService\"\n",
        )
        .unwrap();
        assert_eq!(c.naming_ssliop_ca_file, "/etc/zerodds/certs/naming-ca.pem");
        assert_eq!(
            c.naming_ssliop_cert_file,
            "/etc/zerodds/certs/bridge-client.crt"
        );
        assert_eq!(
            c.naming_ssliop_key_file,
            "/etc/zerodds/certs/bridge-client.key"
        );
        assert_eq!(
            c.naming_initial_references.get("NameService").unwrap(),
            "corbaloc::ns.example:2809/NameService"
        );
    }

    #[test]
    fn naming_ssliop_cert_without_key_is_refused() {
        let err = load("corba:\n  ior:\n    naming_ssliop:\n      cert_file: c.pem\n").unwrap_err();
        assert!(err.contains("cert_file and key_file"));
    }

    #[test]
    fn initial_references_of_the_wrong_shape_is_refused() {
        let err = load("corba:\n  ior:\n    initial_references: [a, b]\n").unwrap_err();
        assert!(err.contains("expected a map"));
        let err =
            load("corba:\n  ior:\n    initial_references:\n      NameService:\n        - a\n")
                .unwrap_err();
        assert!(err.contains("expected a corbaname"));
    }

    /// `corba.ior.initial_references.<name>` accepts an arbitrary
    /// operator-chosen reference name — `is_key` recognizes the whole
    /// map by prefix rather than needing every name pre-listed.
    #[test]
    fn initial_reference_keys_are_not_reported_as_unknown() {
        assert!(is_key("corba.ior.initial_references.NameService"));
        assert!(is_key("corba.ior.initial_references.TradingService"));
        assert!(!is_key("corba.ior.initial_reference_typo.NameService"));
    }
}
