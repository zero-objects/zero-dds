// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The bridge daemon `zerodds-corba-bridged`
//! (`docs/specs/zerodds-corba-bridge-1.0.md`): an IIOP server (and SSLIOP)
//! on one side, a DDS participant on the other.
//!
//! * `corba_to_dds`: a GIOP request to a mapping's object and operation
//!   becomes a sample on the request topic; the sample on the reply topic
//!   with the same `request_id` becomes the GIOP reply (§4.3). No reply
//!   within `timeout_ms` is `CORBA::TIMEOUT`.
//! * `dds_to_corba`: a sample on the notify topic becomes a oneway GIOP
//!   request to every target (§4.4).
//!
//! The DDS samples carry the GIOP bytes unchanged, in the byte order the
//! GIOP message used (XCDR1, whose alignment is the one GIOP 1.2 gives a
//! body, 8 from the body start):
//!
//! ```idl
//! @final struct <Operation>Request {
//!     unsigned long long request_id;   // the bridge's, unique per daemon
//!     // the operation's in and inout arguments, as GIOP encodes them
//! };
//! @final struct <Operation>Reply {
//!     unsigned long long request_id;   // of the request it answers
//!     long reply_status;               // 0 NO_EXCEPTION, 1 USER_EXCEPTION,
//!                                      // 2 SYSTEM_EXCEPTION (GIOP ReplyStatusType)
//!     unsigned long reserved;          // 0; keeps the rest 8-aligned
//!     // the return value and out/inout arguments, or the exception
//! };
//! ```
//!
//! Because the alignment only matches for GIOP 1.2, a mapped operation
//! reached over GIOP 1.0 or 1.1 is answered `CORBA::NO_IMPLEMENT`.

use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::string::{String, ToString};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};
use std::vec::Vec;

use sha2::{Digest, Sha256};
use zerodds_config::ConfigError;
use zerodds_config::read::{Section, first_origin};
use zerodds_corba_giop::object_ops::{self, ObjectOpReply};
use zerodds_corba_giop::{
    LocateReply, LocateRequest, LocateStatusType, Message, ReplyStatusType, Request, ResponseFlags,
    ServiceContextList, TargetAddress, Version, decode_message_ctx, encode_message,
};
use zerodds_corba_iiop::{Connection, IiopProfileBody, IiopVersion};
use zerodds_corba_ior::{
    Ior, ProfileId, TaggedProfile, from_stringified, parse_corbaloc, to_stringified,
};
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_rtps::wire_types::EntityId;
use zerodds_wire::Endianness;

use crate::daemon_runtime::BridgeMetrics;

// ============================================================================
// Configuration (spec §3).
// ============================================================================

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
    /// `targets` (dds_to_corba): `corbaloc:` URLs or stringified IORs.
    pub targets: Vec<String>,
}

impl Mapping {
    pub(crate) fn receives_giop(&self) -> bool {
        matches!(self.direction, Direction::CorbaToDds | Direction::Bidir)
    }
    fn sends_giop(&self) -> bool {
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
    /// `corba.ior.ior_file`: the stringified IORs are written here.
    pub ior_file: String,
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
            ior_file: String::new(),
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
        }
    }
}

/// Every key the daemon reads (§3); lists stop at the list.
pub const KEYS: &[&str] = &[
    "domain",
    "log_level",
    "corba.orb_id",
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

fn read_topic_ref(section: &Section<'_>, fallback: String) -> Result<TopicRef, ConfigError> {
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
        corba.only(&["orb_id", "iiop", "ssliop", "ior", "naming", "csiv2"])?;
        if let Some(v) = corba.string("orb_id")? {
            cfg.orb_id = v;
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
        if let Some((v, origin)) = metrics.scalar("path")? {
            if v != "/metrics" {
                return Err(invalid(
                    "metrics.path",
                    origin.clone(),
                    "the admin endpoint serves /metrics, /catalog and /healthz",
                ));
            }
        }
        for (path, item) in root.items("mappings")? {
            cfg.mappings.push(read_mapping(&path, item)?);
        }
        let mut seen = std::collections::BTreeSet::new();
        for m in &cfg.mappings {
            if !seen.insert((m.repo_id.clone(), m.operation.clone())) {
                return Err(invalid(
                    "mappings",
                    zerodds_config::Origin::Default,
                    format!("{} {} is mapped twice", m.repo_id, m.operation),
                ));
            }
        }
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
        Ok(cfg)
    }
}

fn read_mapping(path: &str, item: &zerodds_config::Node) -> Result<Mapping, ConfigError> {
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
        targets: Vec::new(),
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
        if let Some((v, origin)) = e.scalar("correlation_id")? {
            if v != "request_id" {
                return Err(invalid(
                    format!("{path}.correlation_id"),
                    origin.clone(),
                    "the correlation field is the first member, request_id; another field needs the type, which the bridge does not have",
                ));
            }
        }
        if let Some(v) = e.parse::<u64>("timeout_ms", "milliseconds")? {
            m.timeout_ms = v;
        }
        let qos = e.section("qos")?;
        qos.only(&["reliability", "durability"])?;
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
/// endpoint.
///
/// # Errors
/// What the text lacks.
pub fn resolve_target(text: &str) -> Result<Target, String> {
    if text.starts_with("corbaloc:") {
        let loc = parse_corbaloc(text).map_err(|e| format!("{text}: {e:?}"))?;
        let ep = loc
            .endpoints
            .iter()
            .find(|e| e.protocol.is_empty() || e.protocol == "iiop")
            .ok_or_else(|| format!("{text}: no iiop endpoint"))?;
        return Ok(Target {
            host: ep.host.clone(),
            port: ep.port,
            object_key: loc.object_key,
        });
    }
    if text.starts_with("IOR:") {
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

// ============================================================================
// Object keys and IORs (§4.1, §4.5).
// ============================================================================

/// §4.5: `SHA-256(repo_id + "\0" + topics)[..16]`; the topics are
/// `request->reply` for request-reply, the notify topic otherwise.
#[must_use]
pub fn object_key(m: &Mapping) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(m.repo_id.as_bytes());
    h.update(b"\0");
    h.update(format!("{}->{}", m.request_topic.dds_name, m.reply_topic.dds_name).as_bytes());
    h.finalize()[..16].to_vec()
}

fn host_port(bind: &str, default_port: u16) -> (String, u16) {
    match bind.rsplit_once(':') {
        Some((h, p)) => (
            h.trim_matches(['[', ']']).to_string(),
            p.parse().unwrap_or(default_port),
        ),
        None => (bind.to_string(), default_port),
    }
}

/// §4.1: the IOR of a mapping: one IIOP profile for the IIOP listener, its
/// object key; `advertise_host` replaces a wildcard bind address.
///
/// # Errors
/// An encoding error of the IOR codec.
pub fn build_ior(cfg: &DaemonConfig, m: &Mapping, advertise_host: &str) -> Result<Ior, String> {
    let (host, port) = host_port(&cfg.iiop_bind, 6833);
    let host = if host == "0.0.0.0" || host == "::" || host.is_empty() {
        advertise_host.to_string()
    } else {
        host
    };
    let mut body = IiopProfileBody::new(
        IiopVersion::new(cfg.giop_version.major, cfg.giop_version.minor),
        host,
        port,
        object_key(m),
    );
    // IIOP 1.0 profiles carry no components (CORBA 3.3 Part 2 §15.7.2).
    if cfg.giop_version.minor >= 1 {
        body.components = profile_components(cfg).map_err(|e| format!("{e:?}"))?;
    }
    let profile = TaggedProfile::iiop(&body, Endianness::Little).map_err(|e| format!("{e:?}"))?;
    Ok(Ior {
        type_id: m.repo_id.clone(),
        profiles: vec![profile],
    })
}

/// UTF-8 (OSF code set registry `0x05010001`).
const CODE_SET_UTF8: u32 = 0x0501_0001;
/// ISO 8859-1 (`0x00010001`), the GIOP default for `char`.
const CODE_SET_ISO_8859_1: u32 = 0x0001_0001;
/// UTF-16 (`0x00010109`).
const CODE_SET_UTF16: u32 = 0x0001_0109;

/// The components of the bridge's IIOP profile:
///
/// * `TAG_CODE_SETS` (CORBA 3.3 Part 2 §13.10.2.4): `char` natively UTF-8 —
///   the bridge passes string bytes through to DDS, whose strings are UTF-8 —
///   with ISO 8859-1 as conversion; `wchar` UTF-16. Without it a client
///   assumes ISO 8859-1 and refuses `wchar`.
/// * `TAG_SSL_SEC_TRANS` (SSLIOP) when SSLIOP is enabled: its port, what it
///   supports (integrity, confidentiality, trust in the target; trust in
///   the client with a CA) and requires (trust in the client with
///   `require_client_cert`). Plain IIOP stays open next to it, so
///   `NoProtection` is supported and nothing else is required of a client
///   that does not ask for SSL. Without this component an SSLIOP client finds
///   no SSL port in the IOR.
///
/// No `TAG_ORB_TYPE`: its value is an OMG-assigned ORB type, and ZeroDDS has
/// none.
fn profile_components(
    cfg: &DaemonConfig,
) -> Result<
    Vec<zerodds_corba_iiop::profile_body::TaggedComponent>,
    zerodds_corba_iiop::profile_body::CdrError,
> {
    use zerodds_corba_csiv2::AssociationOptions as A;
    use zerodds_corba_ior::component_tags::ComponentId;
    use zerodds_corba_ior::components::{
        CodeSetComponent, CodeSetComponentInfo, Ssl, StructuredComponent,
    };
    let component = |tag: ComponentId, c: StructuredComponent| {
        Ok::<_, zerodds_corba_iiop::profile_body::CdrError>(
            zerodds_corba_iiop::profile_body::TaggedComponent {
                tag: tag.as_u32(),
                component_data: c.encode_encapsulation(Endianness::Little)?,
            },
        )
    };
    let mut out = vec![component(
        ComponentId::CodeSets,
        StructuredComponent::CodeSets(CodeSetComponentInfo {
            for_char_data: CodeSetComponent {
                native_code_set: CODE_SET_UTF8,
                conversion_code_sets: vec![CODE_SET_ISO_8859_1],
            },
            for_wchar_data: CodeSetComponent {
                native_code_set: CODE_SET_UTF16,
                conversion_code_sets: Vec::new(),
            },
        }),
    )?];
    if cfg.ssliop_enabled {
        let (_, ssl_port) = host_port(&cfg.ssliop_bind, 6834);
        let with_ca = !cfg.ssliop_ca_file.is_empty();
        let mut supports =
            A::NO_PROTECTION | A::INTEGRITY | A::CONFIDENTIALITY | A::ESTABLISH_TRUST_IN_TARGET;
        let mut requires = 0;
        if with_ca {
            supports |= A::ESTABLISH_TRUST_IN_CLIENT;
        }
        if with_ca && cfg.ssliop_require_client_cert {
            requires |= A::ESTABLISH_TRUST_IN_CLIENT;
        }
        out.push(component(
            ComponentId::SslSecTrans,
            StructuredComponent::Ssl(Ssl {
                target_supports: supports,
                target_requires: requires,
                port: ssl_port,
            }),
        )?);
    }
    if cfg.csiv2_enabled {
        out.push(component(
            ComponentId::CsiSecMechList,
            StructuredComponent::CsiSecMechList(csiv2_mech_list(cfg)?),
        )?);
    }
    Ok(out)
}

/// The `TAG_CSI_SEC_MECH_LIST` component body (CORBA 3.4 Part 2 §10.5):
/// one mechanism describing the GSSUP AS-layer and, when SSLIOP is also
/// enabled, its SSL port as the transport that carries it — `TAG_NULL_TAG`
/// (no transport-level protection required of CSIv2 itself; SSLIOP's own
/// `TAG_SSL_SEC_TRANS` component above still applies to the connection
/// independently) when it is not.
fn csiv2_mech_list(
    cfg: &DaemonConfig,
) -> Result<zerodds_corba_csiv2::CompoundSecMechList, zerodds_corba_iiop::profile_body::CdrError> {
    use zerodds_corba_csiv2::{
        AsContextSec, AssociationOptions as A, CompoundSecMech, CompoundSecMechList, SasContextSec,
    };
    let (transport_mech_tag, transport_mech_data) = if cfg.ssliop_enabled {
        let (_, ssl_port) = host_port(&cfg.ssliop_bind, 6834);
        let mut w = zerodds_wire::Writer::new(Endianness::Little);
        w.write_u8(1); // little-endian encapsulation octet
        w.write_u16(A::NO_PROTECTION | A::INTEGRITY | A::CONFIDENTIALITY);
        w.write_u16(0);
        w.write_u16(ssl_port);
        (
            zerodds_corba_ior::component_tags::ComponentId::SslSecTrans.as_u32(),
            w.finish(),
        )
    } else {
        (
            zerodds_corba_ior::component_tags::ComponentId::NullTag.as_u32(),
            Vec::new(),
        )
    };
    Ok(CompoundSecMechList {
        // Spec §10.3: this daemon evaluates a fresh EstablishContext on
        // every request (see `evaluate_csiv2`) — never `MessageInContext`.
        stateful: false,
        mechanism_list: vec![CompoundSecMech {
            target_requires: A::default().with(A::ESTABLISH_TRUST_IN_CLIENT),
            transport_mech_tag,
            transport_mech_data,
            as_context: AsContextSec {
                target_supports: A::default().with(A::ESTABLISH_TRUST_IN_CLIENT),
                target_requires: A::default().with(A::ESTABLISH_TRUST_IN_CLIENT),
                client_authentication_mech: zerodds_corba_csiv2::gssup::GSSUP_OID_DER.to_vec(),
                target_name: cfg.csiv2_target_name.as_bytes().to_vec(),
            },
            sas_context: SasContextSec {
                target_supports: if cfg.csiv2_target_supports == 0 {
                    A::default()
                } else {
                    A::default().with(A::IDENTITY_ASSERTION)
                },
                target_requires: A::default(),
                privilege_authorities: Vec::new(),
                supported_naming_mechanisms: Vec::new(),
                supported_identity_types: cfg.csiv2_target_supports,
            },
        }],
    })
}

/// The stringified IORs, one line per mapping that receives GIOP:
/// `<repo_id> <operation> IOR:…`.
///
/// # Errors
/// An encoding error of the IOR codec.
pub fn ior_lines(cfg: &DaemonConfig, advertise_host: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for m in cfg.mappings.iter().filter(|m| m.receives_giop()) {
        let ior = build_ior(cfg, m, advertise_host)?;
        let s = to_stringified(&ior, Endianness::Little).map_err(|e| format!("{e:?}"))?;
        out.push(format!("{} {} {s}", m.repo_id, m.operation));
    }
    Ok(out)
}

// ============================================================================
// Reply and exception bodies.
// ============================================================================

/// The completion status of a system exception (CORBA §4.12.3).
#[derive(Debug, Clone, Copy)]
enum Completion {
    No = 1,
    Maybe = 2,
}

/// A `SystemExceptionReplyBody` (GIOP §15.4.3.3) in `endianness`, as the
/// body of a GIOP 1.2 reply (8-aligned from the message start, so the
/// body's own offsets are the alignment).
fn system_exception_body(
    name: &str,
    minor: u32,
    completed: Completion,
    endianness: Endianness,
) -> Vec<u8> {
    let id = format!("IDL:omg.org/CORBA/{name}:1.0");
    let u32b = |v: u32| match endianness {
        Endianness::Little => v.to_le_bytes(),
        Endianness::Big => v.to_be_bytes(),
    };
    let mut b = Vec::new();
    b.extend_from_slice(&u32b(u32::try_from(id.len() + 1).unwrap_or(0)));
    b.extend_from_slice(id.as_bytes());
    b.push(0);
    while b.len() % 4 != 0 {
        b.push(0);
    }
    b.extend_from_slice(&u32b(minor));
    b.extend_from_slice(&u32b(completed as u32));
    b
}

// ============================================================================
// Logging (§8.1).
// ============================================================================

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

fn event_level(event: &str) -> &'static str {
    match event {
        "dds_write_failed" | "encode_error" | "reply_write_failed" => "error",
        "timeout"
        | "unknown_object"
        | "bad_operation"
        | "acl_denied"
        | "giop_version_refused"
        | "reply_without_request"
        | "reply_malformed"
        | "notify_failed"
        | "conn_error"
        | "message_too_large"
        | "notify_sample_refused" => "warn",
        "request" | "reply" | "notify" => "debug",
        _ => "info",
    }
}

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

// ============================================================================
// The daemon.
// ============================================================================

/// Why the daemon stopped.
#[derive(Debug)]
pub struct DaemonError {
    /// Exit code (§2).
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

fn err(code: u8, msg: impl Into<String>) -> DaemonError {
    DaemonError {
        code,
        msg: msg.into(),
    }
}

/// The writing half of a client connection, shared by the reader thread
/// of the connection and the reply dispatchers.
type ConnWriter = Arc<Mutex<Box<dyn Write + Send>>>;

/// A request waiting for its DDS reply.
struct Pending {
    conn: ConnWriter,
    giop_request_id: u32,
    deadline: Instant,
    /// Index of the mapping, for its metrics.
    mapping: usize,
    /// When the request went to DDS, for the latency histogram.
    started: Instant,
    /// §7.2: this request's CSIv2 SAS decision, attached to whichever
    /// reply it eventually gets (the DDS reply, or a TIMEOUT/TRANSIENT).
    sas_reply: Option<zerodds_corba_csiv2::sas::SasMessage>,
}

/// Reply status as the `status` label of `requests_total`.
#[derive(Clone, Copy)]
enum Outcome {
    Ok = 0,
    User = 1,
    System = 2,
}

/// The families of spec §8.2 for one mapping. Their label values come from
/// the configuration, never from the wire: an operation name a client sends
/// would otherwise grow the label set without bound.
struct OpMetrics {
    requests: [Arc<zerodds_monitor::Counter>; 3],
    replies_sent: Arc<zerodds_monitor::Counter>,
    pending: Arc<zerodds_monitor::Gauge>,
    latency: Arc<zerodds_monitor::LabeledHistogram>,
    timeouts: Arc<zerodds_monitor::Counter>,
    /// `dds_samples_published{dds_topic}` of the request topic.
    published: Arc<zerodds_monitor::Counter>,
    /// `dds_samples_received{dds_topic}` of the reply or notify topic.
    received: Arc<zerodds_monitor::Counter>,
}

/// Spec §8.2: `zerodds_corba_bridge_*`. No `csiv2_failures_total`: CSIv2
/// is not built.
struct SpecMetrics {
    ops: Vec<OpMetrics>,
    locate_requests: Arc<zerodds_monitor::Counter>,
    giop_message_errors: Arc<zerodds_monitor::Counter>,
}

impl SpecMetrics {
    fn register(registry: &zerodds_monitor::Registry, cfg: &DaemonConfig) -> Self {
        use zerodds_monitor::Labels;
        for (name, help) in [
            (
                "zerodds_corba_bridge_requests_total",
                "GIOP requests of a mapped operation, by reply status",
            ),
            (
                "zerodds_corba_bridge_replies_sent_total",
                "GIOP replies sent, TIMEOUT included",
            ),
            (
                "zerodds_corba_bridge_pending_requests",
                "Requests waiting for their DDS reply",
            ),
            (
                "zerodds_corba_bridge_request_latency_seconds",
                "Request to DDS reply",
            ),
            (
                "zerodds_corba_bridge_timeouts_total",
                "Requests answered with CORBA::TIMEOUT",
            ),
            (
                "zerodds_corba_bridge_dds_samples_received",
                "DDS samples taken from a reply or notify topic",
            ),
            (
                "zerodds_corba_bridge_dds_samples_published",
                "DDS samples written to a request topic",
            ),
            (
                "zerodds_corba_bridge_locate_requests_total",
                "GIOP LocateRequests",
            ),
            (
                "zerodds_corba_bridge_giop_message_errors_total",
                "GIOP messages refused as malformed or too large",
            ),
        ] {
            registry.set_help(name, help);
        }
        let ops = cfg
            .mappings
            .iter()
            .map(|m| {
                let op = || Labels::new().with("operation", m.operation.clone());
                let status = |s: &str| {
                    registry.counter(
                        "zerodds_corba_bridge_requests_total",
                        op().with("status", s.to_string()),
                    )
                };
                let received_topic = if m.receives_giop() {
                    &m.reply_topic.dds_name
                } else {
                    &m.notify_topic.dds_name
                };
                OpMetrics {
                    requests: [
                        status("no_exception"),
                        status("user_exception"),
                        status("system_exception"),
                    ],
                    replies_sent: registry.counter("zerodds_corba_bridge_replies_sent_total", op()),
                    pending: registry.gauge("zerodds_corba_bridge_pending_requests", op()),
                    latency: registry
                        .histogram("zerodds_corba_bridge_request_latency_seconds", op()),
                    timeouts: registry.counter("zerodds_corba_bridge_timeouts_total", op()),
                    published: registry.counter(
                        "zerodds_corba_bridge_dds_samples_published",
                        Labels::new().with("dds_topic", m.request_topic.dds_name.clone()),
                    ),
                    received: registry.counter(
                        "zerodds_corba_bridge_dds_samples_received",
                        Labels::new().with("dds_topic", received_topic.clone()),
                    ),
                }
            })
            .collect();
        Self {
            ops,
            locate_requests: registry
                .counter("zerodds_corba_bridge_locate_requests_total", Labels::new()),
            giop_message_errors: registry.counter(
                "zerodds_corba_bridge_giop_message_errors_total",
                Labels::new(),
            ),
        }
    }
}

/// A request that got its reply (or TIMEOUT, or a refusal after it was
/// pending): the families of its mapping.
fn count_reply(d: &Dispatch, p: &Pending, outcome: Outcome) {
    if let Some(m) = d.spec.as_ref().and_then(|s| s.ops.get(p.mapping)) {
        m.pending.dec();
        m.replies_sent.inc();
        m.requests[outcome as usize].inc();
        m.latency.record_seconds(p.started.elapsed().as_secs_f64());
    }
}

/// A request of mapping `index` refused before it went to DDS.
fn count_refused(d: &Dispatch, index: usize) {
    if let Some(m) = d.spec.as_ref().and_then(|s| s.ops.get(index)) {
        m.replies_sent.inc();
        m.requests[Outcome::System as usize].inc();
    }
}

/// Every request in flight, by the bridge's request id, and the condition
/// the timeout thread waits on.
struct InFlight {
    pending: Mutex<BTreeMap<u64, Pending>>,
    changed: Condvar,
}

/// An open client connection: its writer, the socket to close at
/// shutdown, and the GIOP version its last message used.
struct Conn {
    writer: ConnWriter,
    sock: TcpStream,
    version: Mutex<Version>,
}

/// How long the shutdown waits for replies still pending (§9.2).
const DRAIN_LIMIT: Duration = Duration::from_secs(30);

/// What a connection needs to dispatch requests.
struct Dispatch {
    cfg: DaemonConfig,
    runtime: Arc<DcpsRuntime>,
    /// Object key → mapping index and its request writer.
    by_key: BTreeMap<Vec<u8>, (usize, EntityId)>,
    in_flight: Arc<InFlight>,
    next_id: AtomicU64,
    metrics: BridgeMetrics,
    /// Spec §8.2, when the registry was handed over.
    spec: Option<SpecMetrics>,
    /// ACL and SSLIOP material, replaced by a SIGHUP (§9.2).
    reloadable: std::sync::RwLock<Reloadable>,
    /// Open connections, by an id of the bridge.
    conns: Mutex<BTreeMap<u64, Arc<Conn>>>,
    next_conn: AtomicU64,
    /// Set at shutdown: new requests are refused while the pending ones
    /// are answered (§9.2).
    draining: AtomicBool,
    /// §7.2: the GSSUP credential source, when `corba.csiv2.enabled`.
    csiv2_store: Option<Arc<zerodds_corba_csiv2::GssupUserStore>>,
    /// `corba.naming.serve`: a `CosNaming::NamingContext` hosted on this
    /// daemon's own listener, sharing transport and servant code with
    /// `zerodds-naming-svc`. `None` when not configured.
    naming_servant: Option<Arc<zerodds_corba_cosnaming::servant::NamingServant>>,
}

fn encode_reply_with_context(
    version: Version,
    endianness: Endianness,
    request_id: u32,
    status: ReplyStatusType,
    body: Vec<u8>,
    service_context: ServiceContextList,
) -> io::Result<Vec<u8>> {
    let reply = zerodds_corba_giop::Reply {
        request_id,
        reply_status: status,
        service_context,
        body,
    };
    encode_message(version, endianness, false, &Message::Reply(reply))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("encode reply: {e:?}")))
}

/// The reply's service-context list carrying the SAS message this
/// request's CSIv2 evaluation produced, if any (§7.2: attached to every
/// reply the request gets, success or refusal, once a SAS decision was
/// made for it).
fn sas_service_context(
    sas_reply: Option<&zerodds_corba_csiv2::sas::SasMessage>,
    endianness: Endianness,
) -> ServiceContextList {
    let mut list = ServiceContextList::default();
    if let Some(m) = sas_reply {
        if let Some(sc) = crate::csiv2_wire::service_context_for(m, endianness) {
            list.0.push(sc);
        }
    }
    list
}

/// Writes a GIOP message, split into fragments above `fragment_size`
/// (GIOP 1.2 §15.4.9: the first message with the fragment bit, then
/// Fragment messages carrying the request id).
fn write_giop(
    conn: &ConnWriter,
    message: &[u8],
    fragment_size: usize,
    request_id: u32,
    endianness: Endianness,
) -> io::Result<()> {
    let mut w = conn
        .lock()
        .map_err(|_| io::Error::other("connection writer poisoned"))?;
    if message.len() <= fragment_size.max(64) + 12 || message[4..6] != [1, 2] {
        w.write_all(message)?;
        return w.flush();
    }
    let body = &message[12..];
    let u32b = |v: u32| match endianness {
        Endianness::Little => v.to_le_bytes(),
        Endianness::Big => v.to_be_bytes(),
    };
    // GIOP 1.2 §15.4.9: fragments end on 8-octet boundaries of the message,
    // header included. The first message carries 12 header octets, so its
    // data is 4 short of a multiple of 8; every later fragment's data (after
    // its 16-octet header and FragmentHeader) is a multiple of 8.
    let step = (fragment_size.max(64) / 8) * 8;
    let first_len = step - 4;
    let (head, mut rest) = body.split_at(first_len.min(body.len()));
    let mut frame = Vec::with_capacity(head.len() + 12);
    frame.extend_from_slice(b"GIOP");
    frame.extend_from_slice(&[1, 2]);
    frame.push(
        u8::from(matches!(endianness, Endianness::Little)) | if rest.is_empty() { 0 } else { 0x02 },
    );
    frame.push(message[7]);
    frame.extend_from_slice(&u32b(u32::try_from(head.len()).unwrap_or(u32::MAX)));
    frame.extend_from_slice(head);
    w.write_all(&frame)?;
    while !rest.is_empty() {
        let (chunk, tail) = rest.split_at(step.min(rest.len()));
        rest = tail;
        let mut frame = Vec::with_capacity(chunk.len() + 16);
        frame.extend_from_slice(b"GIOP");
        frame.extend_from_slice(&[1, 2]);
        frame.push(
            u8::from(matches!(endianness, Endianness::Little))
                | if rest.is_empty() { 0 } else { 0x02 },
        );
        frame.push(7); // Fragment
        frame.extend_from_slice(&u32b(u32::try_from(chunk.len() + 4).unwrap_or(u32::MAX)));
        frame.extend_from_slice(&u32b(request_id));
        frame.extend_from_slice(chunk);
        w.write_all(&frame)?;
    }
    w.flush()
}

fn system_exception(
    d: &Dispatch,
    conn: &ConnWriter,
    version: Version,
    endianness: Endianness,
    request_id: u32,
    name: &str,
    completed: Completion,
) -> io::Result<()> {
    system_exception_with_sas(
        d, conn, version, endianness, request_id, name, completed, None,
    )
}

#[allow(clippy::too_many_arguments)]
fn system_exception_with_sas(
    d: &Dispatch,
    conn: &ConnWriter,
    version: Version,
    endianness: Endianness,
    request_id: u32,
    name: &str,
    completed: Completion,
    sas_reply: Option<&zerodds_corba_csiv2::sas::SasMessage>,
) -> io::Result<()> {
    let body = system_exception_body(name, 0, completed, endianness);
    let msg = encode_reply_with_context(
        version,
        endianness,
        request_id,
        ReplyStatusType::SystemException,
        body,
        sas_service_context(sas_reply, endianness),
    )?;
    d.metrics.frames_out_total.inc();
    write_giop(conn, &msg, d.cfg.fragment_size, request_id, endianness)
}

/// Encodes and writes a `corba.naming.serve` [`ServantReply`] the same
/// way `zerodds-naming-svc`'s own `serve_connection` does -- the wire
/// shape of every reply status a `NamingServant` produces.
///
/// [`ServantReply`]: zerodds_corba_cosnaming::servant::ServantReply
#[allow(clippy::too_many_arguments)]
fn naming_dispatch_reply(
    d: &Dispatch,
    conn: &ConnWriter,
    version: Version,
    endianness: Endianness,
    request_id: u32,
    respond: bool,
    dispatched: zerodds_corba_cosnaming::servant::ServantReply,
    sas_reply: Option<&zerodds_corba_csiv2::sas::SasMessage>,
) -> io::Result<()> {
    if !respond {
        return Ok(());
    }
    use zerodds_corba_cosnaming::servant::ServantReply;
    let (status, body) = match dispatched {
        ServantReply::Ok(b) => (ReplyStatusType::NoException, b),
        ServantReply::UserException { body, .. } => (ReplyStatusType::UserException, body),
        ServantReply::BadOperation => (
            ReplyStatusType::SystemException,
            system_exception_body("BAD_OPERATION", 0, Completion::No, endianness),
        ),
        ServantReply::Marshal => (
            ReplyStatusType::SystemException,
            system_exception_body("MARSHAL", 0, Completion::No, endianness),
        ),
        ServantReply::ObjectNotExist => (
            ReplyStatusType::SystemException,
            system_exception_body("OBJECT_NOT_EXIST", 0, Completion::No, endianness),
        ),
        ServantReply::InterfaceRepositoryUnavailable => (
            ReplyStatusType::SystemException,
            system_exception_body("INTF_REPOS", 0, Completion::No, endianness),
        ),
    };
    let msg = encode_reply_with_context(
        version,
        endianness,
        request_id,
        status,
        body,
        sas_service_context(sas_reply, endianness),
    )?;
    d.metrics.frames_out_total.inc();
    write_giop(conn, &msg, d.cfg.fragment_size, request_id, endianness)
}

/// Encodes and writes an [`ObjectOpReply`] (CORBA 3.3 Part 2 §15.4.2's
/// six `CORBA::Object` operations) the same way every other outcome in
/// `on_request` is answered — `Ok` becomes `NO_EXCEPTION`,
/// `Marshal`/`InterfaceRepositoryUnavailable` the matching
/// `SystemException`.
#[allow(clippy::too_many_arguments)]
fn object_op_reply(
    d: &Dispatch,
    conn: &ConnWriter,
    version: Version,
    endianness: Endianness,
    request_id: u32,
    respond: bool,
    reply: ObjectOpReply,
    sas_reply: Option<&zerodds_corba_csiv2::sas::SasMessage>,
) -> io::Result<()> {
    if !respond {
        return Ok(());
    }
    let (status, body) = match reply {
        ObjectOpReply::Ok(b) => (ReplyStatusType::NoException, b),
        ObjectOpReply::Marshal => (
            ReplyStatusType::SystemException,
            system_exception_body("MARSHAL", 0, Completion::No, endianness),
        ),
        ObjectOpReply::InterfaceRepositoryUnavailable => (
            ReplyStatusType::SystemException,
            system_exception_body("INTF_REPOS", 0, Completion::No, endianness),
        ),
    };
    let msg = encode_reply_with_context(
        version,
        endianness,
        request_id,
        status,
        body,
        sas_service_context(sas_reply, endianness),
    )?;
    d.metrics.frames_out_total.inc();
    write_giop(conn, &msg, d.cfg.fragment_size, request_id, endianness)
}

fn target_key(target: &TargetAddress) -> Option<Vec<u8>> {
    match target {
        TargetAddress::Key(k) => Some(k.clone()),
        TargetAddress::Profile(bytes) => {
            let p = TaggedProfile {
                tag: ProfileId::InternetIop,
                profile_data: bytes.clone(),
            };
            p.as_iiop().and_then(Result::ok).map(|b| b.object_key)
        }
        TargetAddress::Reference { .. } => None,
    }
}

#[cfg(test)]
fn subject_allowed(cfg: &DaemonConfig, identities: &[String], repo_id: &str) -> bool {
    acl_allows(cfg.acl_default_deny, &cfg.acl_rules, identities, repo_id)
}

/// `identities`: what the connection proved — the certificate's subject DN
/// and its fingerprint; empty without a client certificate.
fn acl_allows(default_deny: bool, rules: &[AclRule], identities: &[String], repo_id: &str) -> bool {
    let rules: Vec<&AclRule> = rules
        .iter()
        .filter(|r| r.subject == "*" || identities.contains(&r.subject))
        .collect();
    let hit = rules.iter().any(|r| {
        r.allow_invoke.iter().any(|p| match p.strip_suffix('*') {
            Some(prefix) => repo_id.starts_with(prefix),
            None => p == repo_id,
        })
    });
    hit || (!default_deny && rules.is_empty())
}

/// The part of the configuration a SIGHUP replaces (§9.2): the ACL, and the
/// SSLIOP server configuration for connections accepted afterwards.
struct Reloadable {
    acl_default_deny: bool,
    acl_rules: Vec<AclRule>,
    tls: Option<Arc<rustls::ServerConfig>>,
}

/// What `run_with_reload` needs to reload: the flag the signal watcher sets,
/// and how to read the configuration again (the same layered sources the
/// daemon started from).
pub struct Reload {
    /// Set on SIGHUP; cleared when the reload is done.
    pub flag: Arc<AtomicBool>,
    /// Reads and validates the configuration again.
    pub load: Box<dyn Fn() -> Result<DaemonConfig, String> + Send + Sync>,
}

fn ssliop_server_config(cfg: &DaemonConfig) -> Result<Arc<rustls::ServerConfig>, String> {
    let ca =
        (!cfg.ssliop_ca_file.is_empty()).then(|| std::path::PathBuf::from(&cfg.ssliop_ca_file));
    crate::daemon_runtime::build_corba_tls_config(
        std::path::PathBuf::from(&cfg.ssliop_cert_file),
        std::path::PathBuf::from(&cfg.ssliop_key_file),
        if cfg.ssliop_require_client_cert {
            ca
        } else {
            None
        },
    )
    .map_err(|e| format!("SSLIOP: {e}"))
}

/// One reload (§9.2): the new ACL and SSLIOP material take effect, the rest
/// needs a restart and is named. A configuration that does not load, or
/// SSLIOP material that does not, leaves everything as it was.
fn apply_reload(
    d: &Dispatch,
    started: &DaemonConfig,
    load: &dyn Fn() -> Result<DaemonConfig, String>,
) {
    let new = match load() {
        Ok(c) => c,
        Err(e) => {
            log("reload_refused", &[("err", &e)]);
            return;
        }
    };
    let tls = if new.ssliop_enabled && started.ssliop_enabled {
        match ssliop_server_config(&new) {
            Ok(t) => Some(t),
            Err(e) => {
                log("reload_refused", &[("err", &e)]);
                return;
            }
        }
    } else {
        None
    };
    let mut restart: Vec<&str> = Vec::new();
    if new.mappings != started.mappings {
        restart.push("mappings");
    }
    if new.iiop_bind != started.iiop_bind || new.ssliop_bind != started.ssliop_bind {
        restart.push("binds");
    }
    if new.ssliop_enabled != started.ssliop_enabled {
        restart.push("corba.ssliop.enabled");
    }
    if new.domain != started.domain {
        restart.push("domain");
    }
    if let Ok(mut r) = d.reloadable.write() {
        r.acl_default_deny = new.acl_default_deny;
        r.acl_rules = new.acl_rules.clone();
        if tls.is_some() {
            r.tls = tls;
        }
    }
    log("reloaded", &[("applied", "acl, ssliop certificate")]);
    if !restart.is_empty() {
        log("reload_needs_restart", &[("changed", &restart.join(", "))]);
    }
}

/// One GIOP request (§4.3).
fn on_request(
    d: &Dispatch,
    conn: &ConnWriter,
    version: Version,
    endianness: Endianness,
    req: Request,
    identities: &[String],
) -> io::Result<()> {
    d.metrics.frames_in_total.inc();
    let respond = req.response_flags.response_expected();
    if d.draining.load(Ordering::SeqCst) {
        log("refused_while_draining", &[("operation", &req.operation)]);
        return if respond {
            system_exception(
                d,
                conn,
                version,
                endianness,
                req.request_id,
                "TRANSIENT",
                Completion::No,
            )
        } else {
            Ok(())
        };
    }

    // §7.2: CSIv2 GSSUP, evaluated before the target is even looked up —
    // a rejected EstablishContext (bad credentials, or an unsupported
    // asserted identity) refuses the whole request. Once a SAS decision
    // is made, every reply this request gets — including one refused for
    // an unrelated reason below — carries it (the client's SasClient
    // state machine reads `CompleteEstablishContext`/`ContextError` off
    // every reply, not only a successful one).
    let mut identities: Vec<String> = identities.to_vec();
    let mut sas_reply: Option<zerodds_corba_csiv2::sas::SasMessage> = None;
    if let Some(store) = &d.csiv2_store {
        match crate::csiv2_wire::evaluate(&req.service_context, store, d.cfg.csiv2_target_supports)
        {
            crate::csiv2_wire::Csiv2Outcome::Accepted { principal, reply } => {
                identities.push(format!("gssup:{principal}"));
                sas_reply = Some(reply);
            }
            crate::csiv2_wire::Csiv2Outcome::Rejected { reply } => {
                log("csiv2_rejected", &[("operation", &req.operation)]);
                return if respond {
                    system_exception_with_sas(
                        d,
                        conn,
                        version,
                        endianness,
                        req.request_id,
                        "NO_PERMISSION",
                        Completion::No,
                        Some(&reply),
                    )
                } else {
                    Ok(())
                };
            }
            crate::csiv2_wire::Csiv2Outcome::NotOffered => {}
        }
    }
    let identities = identities.as_slice();

    let key = target_key(&req.target);
    let Some((index, writer)) = key.as_ref().and_then(|k| d.by_key.get(k).copied()) else {
        // corba.naming.serve: this object key is not a DDS<->GIOP mapping,
        // but it may be the NameService root or one of its sub-contexts /
        // live BindingIterators -- try the hosted NamingServant before
        // refusing OBJECT_NOT_EXIST. `dispatch` itself reports
        // `ObjectNotExist` for a key it does not know either, so this
        // never masks a genuinely wrong object key.
        if let (Some(servant), Some(k)) = (&d.naming_servant, key.as_deref()) {
            let dispatched = servant.dispatch(k, &req.operation, &req.body, endianness);
            if !matches!(
                dispatched,
                zerodds_corba_cosnaming::servant::ServantReply::ObjectNotExist
            ) {
                log("naming_dispatch", &[("operation", &req.operation)]);
                return naming_dispatch_reply(
                    d,
                    conn,
                    version,
                    endianness,
                    req.request_id,
                    respond,
                    dispatched,
                    sas_reply.as_ref(),
                );
            }
        }
        // CORBA 3.3 Part 2 §15.4.2: `_non_existent`/`_not_existent` answers
        // `true` for a key nothing here serves (neither a mapping nor,
        // above, the NameService), instead of the `OBJECT_NOT_EXIST` every
        // other operation gets on this path.
        if let Some(body) = object_ops::unknown_object_reply(&req.operation, endianness) {
            log("object_op", &[("operation", &req.operation)]);
            return object_op_reply(
                d,
                conn,
                version,
                endianness,
                req.request_id,
                respond,
                ObjectOpReply::Ok(body),
                sas_reply.as_ref(),
            );
        }
        log("unknown_object", &[("operation", &req.operation)]);
        return if respond {
            system_exception_with_sas(
                d,
                conn,
                version,
                endianness,
                req.request_id,
                "OBJECT_NOT_EXIST",
                Completion::No,
                sas_reply.as_ref(),
            )
        } else {
            Ok(())
        };
    };
    let m = &d.cfg.mappings[index];
    if object_ops::is_object_operation(&req.operation) {
        // Same ACL gate as the mapped operation below (§7.3) — a client
        // without `allow_invoke` for this `repo_id` learns nothing about
        // it, not even via `_is_a`/`_repository_id`.
        let allowed = d
            .reloadable
            .read()
            .map(|r| acl_allows(r.acl_default_deny, &r.acl_rules, identities, &m.repo_id))
            .unwrap_or(false);
        if !allowed {
            log(
                "acl_denied",
                &[
                    ("repo_id", &m.repo_id),
                    (
                        "subject",
                        identities.first().map_or("(none)", String::as_str),
                    ),
                ],
            );
            if respond {
                count_refused(d, index);
            }
            return if respond {
                system_exception_with_sas(
                    d,
                    conn,
                    version,
                    endianness,
                    req.request_id,
                    "NO_PERMISSION",
                    Completion::No,
                    sas_reply.as_ref(),
                )
            } else {
                Ok(())
            };
        }
        if let Some(reply) =
            object_ops::dispatch_with_bases(&req.operation, &req.body, endianness, &m.repo_id, &[])
        {
            log(
                "object_op",
                &[("operation", &req.operation), ("repo_id", &m.repo_id)],
            );
            return object_op_reply(
                d,
                conn,
                version,
                endianness,
                req.request_id,
                respond,
                reply,
                sas_reply.as_ref(),
            );
        }
    }
    if req.operation != m.operation {
        log(
            "bad_operation",
            &[("operation", &req.operation), ("repo_id", &m.repo_id)],
        );
        if respond {
            count_refused(d, index);
        }
        return if respond {
            system_exception_with_sas(
                d,
                conn,
                version,
                endianness,
                req.request_id,
                "BAD_OPERATION",
                Completion::No,
                sas_reply.as_ref(),
            )
        } else {
            Ok(())
        };
    }
    let allowed = d
        .reloadable
        .read()
        .map(|r| acl_allows(r.acl_default_deny, &r.acl_rules, identities, &m.repo_id))
        .unwrap_or(false);
    if !allowed {
        log(
            "acl_denied",
            &[
                ("repo_id", &m.repo_id),
                (
                    "subject",
                    identities.first().map_or("(none)", String::as_str),
                ),
            ],
        );
        if respond {
            count_refused(d, index);
        }
        return if respond {
            system_exception_with_sas(
                d,
                conn,
                version,
                endianness,
                req.request_id,
                "NO_PERMISSION",
                Completion::No,
                sas_reply.as_ref(),
            )
        } else {
            Ok(())
        };
    }
    if version != Version::V1_2 {
        // The DDS sample carries the body with GIOP 1.2's alignment; an
        // older version aligns it from another origin.
        log("giop_version_refused", &[("operation", &req.operation)]);
        if respond {
            count_refused(d, index);
        }
        return if respond {
            system_exception_with_sas(
                d,
                conn,
                version,
                endianness,
                req.request_id,
                "NO_IMPLEMENT",
                Completion::No,
                sas_reply.as_ref(),
            )
        } else {
            Ok(())
        };
    }
    let id = d.next_id.fetch_add(1, Ordering::Relaxed);
    let mut sample = Vec::with_capacity(8 + req.body.len());
    sample.extend_from_slice(&match endianness {
        Endianness::Little => id.to_le_bytes(),
        Endianness::Big => id.to_be_bytes(),
    });
    sample.extend_from_slice(&req.body);
    if respond {
        let mut p = d
            .in_flight
            .pending
            .lock()
            .map_err(|_| io::Error::other("pending map poisoned"))?;
        p.insert(
            id,
            Pending {
                conn: Arc::clone(conn),
                giop_request_id: req.request_id,
                deadline: Instant::now() + Duration::from_millis(m.timeout_ms),
                mapping: index,
                started: Instant::now(),
                sas_reply: sas_reply.clone(),
            },
        );
        if let Some(om) = d.spec.as_ref().and_then(|s| s.ops.get(index)) {
            om.pending.inc();
        }
        d.in_flight.changed.notify_all();
    }
    use zerodds_rtps::publication_data::data_representation as dr;
    let _ = d
        .runtime
        .set_user_writer_data_rep_override(writer, Some(vec![dr::XCDR]));
    let _ = d
        .runtime
        .set_user_writer_byte_order_override(writer, matches!(endianness, Endianness::Big));
    log(
        "request",
        &[("operation", &req.operation), ("id", &id.to_string())],
    );
    if let Err(e) = d.runtime.write_user_sample_borrowed(writer, &sample) {
        d.metrics.errors_total.inc();
        log(
            "dds_write_failed",
            &[
                ("topic", &m.request_topic.dds_name),
                ("err", &format!("{e:?}")),
            ],
        );
        if respond {
            let removed = d
                .in_flight
                .pending
                .lock()
                .ok()
                .and_then(|mut p| p.remove(&id));
            if let Some(p) = removed {
                count_reply(d, &p, Outcome::System);
            }
            return system_exception_with_sas(
                d,
                conn,
                version,
                endianness,
                req.request_id,
                "TRANSIENT",
                Completion::No,
                sas_reply.as_ref(),
            );
        }
        return Ok(());
    }
    d.metrics.dds_samples_out_total.inc();
    if let Some(om) = d.spec.as_ref().and_then(|s| s.ops.get(index)) {
        om.published.inc();
    }
    Ok(())
}

/// Reassembles fragmented GIOP 1.1/1.2 messages (§4.6): the first message
/// carries the fragment bit, Fragment messages (type 7) follow until one
/// without it.
#[derive(Default)]
struct Reassembly {
    first: Option<Vec<u8>>,
}

impl Reassembly {
    /// Feeds one raw message; returns a whole message when complete.
    fn feed(&mut self, raw: Vec<u8>) -> io::Result<Option<Vec<u8>>> {
        let more = raw[6] & 0x02 != 0;
        let is_fragment = raw[7] == 7;
        let le = raw[6] & 0x01 != 0;
        match (&mut self.first, is_fragment) {
            (None, false) if !more => Ok(Some(raw)),
            (None, false) => {
                self.first = Some(raw);
                Ok(None)
            }
            (None, true) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a Fragment without a fragmented message",
            )),
            (Some(_), false) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a new message inside a fragmented one",
            )),
            (Some(first), true) => {
                // GIOP 1.2 fragments start with the request id.
                let skip = if raw[4..6] == [1, 2] { 16 } else { 12 };
                if raw.len() < skip {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "a short Fragment",
                    ));
                }
                first.extend_from_slice(&raw[skip..]);
                if more {
                    return Ok(None);
                }
                let mut whole = self.first.take().unwrap_or_default();
                whole[6] &= !0x02;
                let size = u32::try_from(whole.len() - 12).unwrap_or(u32::MAX);
                let b = if le {
                    size.to_le_bytes()
                } else {
                    size.to_be_bytes()
                };
                whole[8..12].copy_from_slice(&b);
                Ok(Some(whole))
            }
        }
    }
}

/// Serves one client connection until it closes.
fn serve_connection(
    d: &Dispatch,
    mut reader: Box<dyn Read + Send>,
    entry: &Conn,
    identities: Vec<String>,
) -> io::Result<()> {
    let conn = Arc::clone(&entry.writer);
    let mut acc: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut frag = Reassembly::default();
    loop {
        // Whole messages in the buffer first.
        while acc.len() >= 12 {
            if &acc[..4] != b"GIOP" {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "not a GIOP message",
                ));
            }
            let le = acc[6] & 0x01 != 0;
            let size = if le {
                u32::from_le_bytes([acc[8], acc[9], acc[10], acc[11]])
            } else {
                u32::from_be_bytes([acc[8], acc[9], acc[10], acc[11]])
            } as usize;
            if size > d.cfg.max_message_size {
                log("message_too_large", &[("size", &size.to_string())]);
                if let Some(sm) = d.spec.as_ref() {
                    sm.giop_message_errors.inc();
                }
                let error = encode_message(
                    Version::V1_2,
                    if le {
                        Endianness::Little
                    } else {
                        Endianness::Big
                    },
                    false,
                    &Message::MessageError(zerodds_corba_giop::MessageError),
                )
                .unwrap_or_default();
                let _ = conn.lock().map(|mut w| w.write_all(&error));
                return Ok(());
            }
            if acc.len() < 12 + size {
                break;
            }
            let raw: Vec<u8> = acc.drain(..12 + size).collect();
            let Some(whole) = frag.feed(raw)? else {
                continue;
            };
            let version = Version {
                major: whole[4],
                minor: whole[5],
            };
            if let Ok(mut v) = entry.version.lock() {
                *v = version;
            }
            let (msg, endianness, _) = decode_message_ctx(&whole).map_err(|e| {
                if let Some(sm) = d.spec.as_ref() {
                    sm.giop_message_errors.inc();
                }
                io::Error::new(io::ErrorKind::InvalidData, format!("giop: {e:?}"))
            })?;
            match msg {
                Message::Request(req) => {
                    on_request(d, &conn, version, endianness, req, &identities)?
                }
                Message::LocateRequest(lr) => {
                    if let Some(sm) = d.spec.as_ref() {
                        sm.locate_requests.inc();
                    }
                    let status = if target_key(&lr.target).is_some_and(|k| {
                        d.by_key.contains_key(&k)
                            || d.naming_servant.as_ref().is_some_and(|s| {
                                s.graph().is_iterator_key(&k)
                                    || s.graph().context_for_key(&k).is_some()
                            })
                    }) {
                        LocateStatusType::ObjectHere
                    } else {
                        LocateStatusType::UnknownObject
                    };
                    let reply = LocateReply {
                        request_id: lr.request_id,
                        locate_status: status,
                        body: Vec::new(),
                    };
                    let bytes =
                        encode_message(version, endianness, false, &Message::LocateReply(reply))
                            .map_err(|e| {
                                io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}"))
                            })?;
                    let mut w = conn
                        .lock()
                        .map_err(|_| io::Error::other("connection writer poisoned"))?;
                    w.write_all(&bytes)?;
                    w.flush()?;
                }
                Message::CancelRequest(c) => {
                    if let Ok(mut p) = d.in_flight.pending.lock() {
                        let conn_ptr = Arc::as_ptr(&conn);
                        let mut cancelled: Vec<usize> = Vec::new();
                        p.retain(|_, v| {
                            let keep = !(v.giop_request_id == c.request_id
                                && Arc::as_ptr(&v.conn) == conn_ptr);
                            if !keep {
                                cancelled.push(v.mapping);
                            }
                            keep
                        });
                        for index in cancelled {
                            if let Some(om) = d.spec.as_ref().and_then(|s| s.ops.get(index)) {
                                om.pending.dec();
                            }
                        }
                    }
                }
                Message::CloseConnection(_) | Message::MessageError(_) => return Ok(()),
                other => {
                    log(
                        "unsupported_message",
                        &[("type", &format!("{:?}", other.message_type()))],
                    );
                }
            }
        }
        let n = match reader.read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        d.metrics.bytes_in_total.add(n as u64);
        acc.extend_from_slice(&buf[..n]);
    }
}

/// A reply sample on a reply topic (§4.3 step 7).
fn on_reply(d: &Dispatch, payload: &[u8], big_endian: bool, representation: u8) {
    let endianness = if big_endian {
        Endianness::Big
    } else {
        Endianness::Little
    };
    if payload.len() < 16 {
        d.metrics.errors_total.inc();
        log("reply_malformed", &[("len", &payload.len().to_string())]);
        return;
    }
    let u64v = |b: &[u8]| {
        let a: [u8; 8] = b.try_into().unwrap_or([0; 8]);
        if big_endian {
            u64::from_be_bytes(a)
        } else {
            u64::from_le_bytes(a)
        }
    };
    let i32v = |b: &[u8]| {
        let a: [u8; 4] = b.try_into().unwrap_or([0; 4]);
        if big_endian {
            i32::from_be_bytes(a)
        } else {
            i32::from_le_bytes(a)
        }
    };
    let id = u64v(&payload[..8]);
    let status = i32v(&payload[8..12]);
    let Some(p) = d
        .in_flight
        .pending
        .lock()
        .ok()
        .and_then(|mut m| m.remove(&id))
    else {
        // Timed out already, cancelled, or another bridge's.
        log("reply_without_request", &[("id", &id.to_string())]);
        return;
    };
    if let Some(om) = d.spec.as_ref().and_then(|s| s.ops.get(p.mapping)) {
        om.received.inc();
    }
    let (status, body) = match (status, representation) {
        (_, r) if r != 0 => {
            // XCDR2 aligns 8-byte members at 4: not the GIOP body.
            log(
                "reply_malformed",
                &[
                    ("id", &id.to_string()),
                    ("err", "XCDR2 sample; the reply type is XCDR1"),
                ],
            );
            (
                ReplyStatusType::SystemException,
                system_exception_body("MARSHAL", 0, Completion::Maybe, endianness),
            )
        }
        (0, _) => (ReplyStatusType::NoException, payload[16..].to_vec()),
        (1, _) => (ReplyStatusType::UserException, payload[16..].to_vec()),
        (2, _) => (ReplyStatusType::SystemException, payload[16..].to_vec()),
        (other, _) => {
            log(
                "reply_malformed",
                &[
                    ("id", &id.to_string()),
                    ("reply_status", &other.to_string()),
                ],
            );
            (
                ReplyStatusType::SystemException,
                system_exception_body("MARSHAL", 0, Completion::Maybe, endianness),
            )
        }
    };
    log("reply", &[("id", &id.to_string())]);
    d.metrics.dds_samples_in_total.inc();
    count_reply(
        d,
        &p,
        match status {
            ReplyStatusType::NoException => Outcome::Ok,
            ReplyStatusType::UserException => Outcome::User,
            _ => Outcome::System,
        },
    );
    match encode_reply_with_context(
        Version::V1_2,
        endianness,
        p.giop_request_id,
        status,
        body,
        sas_service_context(p.sas_reply.as_ref(), endianness),
    ) {
        Ok(msg) => {
            d.metrics.frames_out_total.inc();
            d.metrics.bytes_out_total.add(msg.len() as u64);
            if let Err(e) = write_giop(
                &p.conn,
                &msg,
                d.cfg.fragment_size,
                p.giop_request_id,
                endianness,
            ) {
                log("reply_write_failed", &[("err", &e.to_string())]);
            }
        }
        Err(e) => log("encode_error", &[("err", &e.to_string())]),
    }
}

/// Answers every request whose deadline passed with `CORBA::TIMEOUT`.
fn timeout_loop(d: &Dispatch, shutdown: &AtomicBool) {
    let Ok(mut pending) = d.in_flight.pending.lock() else {
        return;
    };
    while !shutdown.load(Ordering::SeqCst) {
        let now = Instant::now();
        let expired: Vec<u64> = pending
            .iter()
            .filter(|(_, p)| p.deadline <= now)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            if let Some(p) = pending.remove(&id) {
                log("timeout", &[("id", &id.to_string())]);
                count_reply(d, &p, Outcome::System);
                if let Some(om) = d.spec.as_ref().and_then(|s| s.ops.get(p.mapping)) {
                    om.timeouts.inc();
                }
                let body =
                    system_exception_body("TIMEOUT", 0, Completion::Maybe, Endianness::Little);
                let service_context = sas_service_context(p.sas_reply.as_ref(), Endianness::Little);
                if let Ok(msg) = encode_reply_with_context(
                    Version::V1_2,
                    Endianness::Little,
                    p.giop_request_id,
                    ReplyStatusType::SystemException,
                    body,
                    service_context,
                ) {
                    let _ = write_giop(
                        &p.conn,
                        &msg,
                        d.cfg.fragment_size,
                        p.giop_request_id,
                        Endianness::Little,
                    );
                }
            }
        }
        let wait = pending
            .values()
            .map(|p| p.deadline.saturating_duration_since(Instant::now()))
            .min()
            .unwrap_or(Duration::from_millis(500))
            .min(Duration::from_millis(500));
        pending = match d.in_flight.changed.wait_timeout(pending, wait) {
            Ok((g, _)) => g,
            Err(_) => return,
        };
    }
}

/// A forward chain longer than this is a misconfiguration (a cycle, or a
/// peer that never stops forwarding), not something to follow forever.
const MAX_FORWARD_HOPS: u32 = 8;

/// How long [`Notifier`] waits for a `LocateReply`, or for the
/// unsolicited `Reply` some peers send even to a oneway request, before
/// giving up and proceeding as if none is coming. Not every ORB answers
/// a `LocateRequest` at all (optional per spec — measured against
/// JacORB and omniORB, `tests/omniorb_e2e.rs`), so this is the
/// observable signal for "this peer does not forward", not an error.
const NOTIFY_READ_TIMEOUT: Duration = Duration::from_millis(500);

/// A oneway GIOP request with `body` to `target` (§4.4); connections are
/// kept per target and opened again after a failure.
///
/// `target` starts as whatever `dds_to_corba.targets` names — often a
/// `corbaloc:`, i.e. an *object key* at a *host:port*, not necessarily
/// the servant's own key. Measured against TAO 2.5
/// (`tests/tao_e2e.rs`, `tests/tao/listener_server.cpp`): TAO's
/// `IORTable` binds a logical name like this to the real,
/// system-assigned key of an activated servant and answers a request
/// against the logical name with `LOCATION_FORWARD` to it — to the
/// oneway `on_value` request itself, not only to a `LocateRequest`,
/// and regardless of `response_flags`; JacORB and omniORB answer the
/// logical name directly, no forward. [`Notifier::resolve`] follows a
/// forward ahead of the first send (GIOP §15.4.5/§15.4.6); the send
/// itself still reads for one anyway.
struct Notifier {
    target: Target,
    operation: String,
    conn: Option<Connection>,
    next_request_id: u32,
    /// `false` until a `LocateRequest` has confirmed `target` is the
    /// real servant — forces resolution before the next send. Cleared
    /// after a connection failure too: the peer may have moved the
    /// object by the time a new connection opens.
    resolved: bool,
}

/// Decodes a `LOCATION_FORWARD`/`LOCATION_FORWARD_PERM` reply body
/// (GIOP §15.4.3.3/§15.4.6.2 — the plain `Object` CDR form: `string
/// type_id; sequence<TaggedProfile> profiles`, in the reply's own byte
/// order, not a further-nested encapsulation) into a [`Target`]. `None`
/// on a decode error or an IOR with no IIOP profile.
fn decode_forward_target(body: &[u8], endianness: Endianness) -> Option<Target> {
    let mut r = zerodds_wire::Reader::new(body, endianness);
    let ior = Ior::decode(&mut r).ok()?;
    ior.profiles.iter().find_map(|p| {
        let iiop = p.as_iiop()?.ok()?;
        Some(Target {
            host: iiop.host,
            port: iiop.port,
            object_key: iiop.object_key,
        })
    })
}

impl Notifier {
    fn ensure_connected(&mut self) -> io::Result<()> {
        if self.conn.is_none() {
            let s = TcpStream::connect((self.target.host.as_str(), self.target.port))?;
            s.set_nodelay(true)?;
            let conn =
                Connection::from_stream(s).map_err(|e| io::Error::other(format!("{e:?}")))?;
            conn.set_read_timeout(Some(NOTIFY_READ_TIMEOUT))
                .map_err(|e| io::Error::other(format!("{e:?}")))?;
            self.conn = Some(conn);
        }
        Ok(())
    }

    fn next_id(&mut self) -> u32 {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1);
        id
    }

    /// Resolves `self.target` via a `LocateRequest` (GIOP §15.4.5),
    /// following `OBJECT_FORWARD`/`OBJECT_FORWARD_PERM` up to
    /// [`MAX_FORWARD_HOPS`]. A no-op once `self.resolved`.
    ///
    /// # Errors
    /// I/O error opening the connection, or a forward chain past the
    /// hop bound — every other outcome (an `OBJECT_HERE`, a peer that
    /// answers something else, or none at all within
    /// [`NOTIFY_READ_TIMEOUT`]) proceeds with whatever `target` is.
    fn resolve(&mut self, endianness: Endianness) -> io::Result<()> {
        if self.resolved {
            return Ok(());
        }
        for _ in 0..MAX_FORWARD_HOPS {
            self.ensure_connected()?;
            let request_id = self.next_id();
            let lr = Message::LocateRequest(LocateRequest {
                request_id,
                target: TargetAddress::Key(self.target.object_key.clone()),
            });
            let conn = self.conn.as_mut().ok_or_else(|| {
                io::Error::other("connection missing right after ensure_connected")
            })?;
            if conn
                .write_message(Version::V1_2, endianness, false, &lr)
                .is_err()
            {
                // The real send retries the connection itself; resolving
                // it here too would just duplicate that retry.
                self.conn = None;
                return Ok(());
            }
            match self.conn.as_mut().and_then(|c| c.read_message().ok()) {
                Some(Message::LocateReply(reply)) => match reply.locate_status {
                    LocateStatusType::ObjectHere => {
                        self.resolved = true;
                        return Ok(());
                    }
                    LocateStatusType::ObjectForward | LocateStatusType::ObjectForwardPerm => {
                        match decode_forward_target(&reply.body, endianness) {
                            Some(t) => {
                                self.target = t;
                                self.conn = None;
                            }
                            None => {
                                self.resolved = true;
                                return Ok(());
                            }
                        }
                    }
                    LocateStatusType::UnknownObject
                    | LocateStatusType::LocSystemException
                    | LocateStatusType::LocNeedsAddressingMode => {
                        self.resolved = true;
                        return Ok(());
                    }
                },
                _ => {
                    self.resolved = true;
                    return Ok(());
                }
            }
        }
        Err(io::Error::other(format!(
            "LocateRequest forward chain exceeded {MAX_FORWARD_HOPS} hops for object key {:02x?}",
            self.target.object_key
        )))
    }

    /// Sends one oneway GIOP request, resolving `target` first
    /// ([`Notifier::resolve`]) and following an unsolicited
    /// `LOCATION_FORWARD`/`LOCATION_FORWARD_PERM` `Reply` to the send
    /// itself (measured against TAO — see the struct doc) up to
    /// [`MAX_FORWARD_HOPS`].
    ///
    /// # Errors
    /// I/O error; a forward chain past the hop bound; or a `Reply` with
    /// any other status — a sample this cannot deliver must be
    /// reported, never silently treated as delivered.
    fn send(&mut self, body: &[u8], endianness: Endianness) -> io::Result<()> {
        self.resolve(endianness)?;
        let mut reconnected = false;
        for _ in 0..MAX_FORWARD_HOPS {
            self.ensure_connected()?;
            let request_id = self.next_id();
            let req = Message::Request(Request {
                request_id,
                response_flags: ResponseFlags::from_response_expected(false),
                target: TargetAddress::Key(self.target.object_key.clone()),
                operation: self.operation.clone(),
                requesting_principal: None,
                service_context: ServiceContextList::default(),
                body: body.to_vec(),
            });
            let conn = self.conn.as_mut().ok_or_else(|| {
                io::Error::other("connection missing right after ensure_connected")
            })?;
            if let Err(e) = conn.write_message(Version::V1_2, endianness, false, &req) {
                if reconnected {
                    return Err(io::Error::other(format!("{e:?}")));
                }
                reconnected = true;
                log(
                    "notify_failed",
                    &[("err", &format!("{e:?}")), ("retry", "reconnect")],
                );
                self.conn = None;
                self.resolved = false;
                self.resolve(endianness)?;
                continue;
            }
            match self.conn.as_mut().and_then(|c| c.read_message().ok()) {
                Some(Message::Reply(reply)) => match reply.reply_status {
                    ReplyStatusType::LocationForward | ReplyStatusType::LocationForwardPerm => {
                        let Some(t) = decode_forward_target(&reply.body, endianness) else {
                            return Err(io::Error::other(
                                "LOCATION_FORWARD reply body did not decode as an object reference",
                            ));
                        };
                        self.target = t;
                        self.conn = None;
                        // `resolved` stays true: this forward already
                        // names where the object lives now.
                        continue;
                    }
                    other => {
                        return Err(io::Error::other(format!(
                            "peer replied {other:?} to a oneway {}",
                            self.operation
                        )));
                    }
                },
                // No reply within the timeout, or a message this
                // connection has no other use for: the ordinary oneway
                // case.
                _ => return Ok(()),
            }
        }
        Err(io::Error::other(format!(
            "LOCATION_FORWARD chain exceeded {MAX_FORWARD_HOPS} hops for object key {:02x?}",
            self.target.object_key
        )))
    }
}

/// Runs the daemon until `shutdown` is set.
///
/// # Errors
/// [`DaemonError`] with the exit code of §2: 1 config, 2 bind, 3 DDS, 4 TLS.
pub fn run(
    cfg: &DaemonConfig,
    runtime_config: RuntimeConfig,
    shutdown: &Arc<AtomicBool>,
    metrics: &BridgeMetrics,
) -> Result<(), DaemonError> {
    run_with_reload(cfg, runtime_config, shutdown, metrics, None)
}

/// [`run`], reloading ACL and SSLIOP material on `reload` (§9.2).
///
/// # Errors
/// As [`run`].
pub fn run_with_reload(
    cfg: &DaemonConfig,
    runtime_config: RuntimeConfig,
    shutdown: &Arc<AtomicBool>,
    metrics: &BridgeMetrics,
    reload: Option<Reload>,
) -> Result<(), DaemonError> {
    LOG_LEVEL.store(level_rank(&cfg.log_level), Ordering::Relaxed);
    let prefix =
        zerodds_dcps::participant::bridge_guid_prefix(&format!("{}@{}", cfg.orb_id, cfg.iiop_bind));
    let runtime = DcpsRuntime::start(cfg.domain, prefix, runtime_config)
        .map_err(|e| err(3, format!("DDS participant: {e:?}")))?;
    let writer_cfg = |t: &TopicRef, m: &Mapping| UserWriterConfig {
        topic_name: t.dds_name.clone(),
        type_name: t.dds_type.clone(),
        reliable: m.reliable,
        durability: m.durability,
        ..Default::default()
    };
    let reader_cfg = |t: &TopicRef, m: &Mapping| UserReaderConfig {
        topic_name: t.dds_name.clone(),
        type_name: t.dds_type.clone(),
        reliable: m.reliable,
        durability: m.durability,
        ..Default::default()
    };
    let in_flight = Arc::new(InFlight {
        pending: Mutex::new(BTreeMap::new()),
        changed: Condvar::new(),
    });
    let mut by_key = BTreeMap::new();
    let mut reply_rx: Vec<mpsc::Receiver<UserSample>> = Vec::new();
    let mut notify_rx: Vec<(usize, mpsc::Receiver<UserSample>)> = Vec::new();
    for (i, m) in cfg.mappings.iter().enumerate() {
        if m.receives_giop() {
            let w = runtime
                .register_user_writer_kind(writer_cfg(&m.request_topic, m), false)
                .map_err(|e| err(3, format!("DDS writer {}: {e:?}", m.request_topic.dds_name)))?;
            let (_r, rx) = runtime
                .register_user_reader_kind(reader_cfg(&m.reply_topic, m), false)
                .map_err(|e| err(3, format!("DDS reader {}: {e:?}", m.reply_topic.dds_name)))?;
            by_key.insert(object_key(m), (i, w));
            reply_rx.push(rx);
        }
        if m.sends_giop() {
            let (_r, rx) = runtime
                .register_user_reader_kind(reader_cfg(&m.notify_topic, m), false)
                .map_err(|e| err(3, format!("DDS reader {}: {e:?}", m.notify_topic.dds_name)))?;
            notify_rx.push((i, rx));
        }
    }
    let csiv2_store = if cfg.csiv2_enabled {
        let text = std::fs::read_to_string(&cfg.csiv2_password_file)
            .map_err(|e| err(1, format!("corba.csiv2.gssup.password_file: {e}")))?;
        let store = zerodds_corba_csiv2::GssupUserStore::parse(&text)
            .map_err(|e| err(1, format!("corba.csiv2.gssup.password_file: {e}")))?;
        Some(Arc::new(store))
    } else {
        None
    };

    // corba.naming.serve: a CosNaming::NamingContext (object key
    // "NameService") hosted on this same IIOP/SSLIOP listener, sharing
    // its transport and the servant code with zerodds-naming-svc's
    // NamingServant -- same persistence option (corba.naming.persist_file,
    // corba-cosnaming's naming.persist_file under a different config
    // path), loaded here and saved after every mutating operation and
    // on a clean stop.
    let naming_graph = if cfg.naming_serve {
        let root = if cfg.naming_persist_file.is_empty() {
            None
        } else {
            let path = std::path::Path::new(&cfg.naming_persist_file);
            if path.exists() {
                Some(
                    zerodds_corba_cosnaming::persist::load(path)
                        .map_err(|e| err(1, format!("corba.naming.persist_file: {e}")))?,
                )
            } else {
                None
            }
        };
        Some(Arc::new(root.map_or_else(
            zerodds_corba_cosnaming::graph::NamingGraph::new,
            zerodds_corba_cosnaming::graph::NamingGraph::from_root,
        )))
    } else {
        None
    };
    let naming_servant = naming_graph.as_ref().map(|graph| {
        let (host, port) = host_port(&cfg.iiop_bind, 6833);
        let advertise_host = if host == "0.0.0.0" || host == "::" || host.is_empty() {
            cfg.iiop_advertise_host.clone()
        } else {
            host
        };
        let mut servant = zerodds_corba_cosnaming::servant::NamingServant::new(
            Arc::clone(graph),
            advertise_host,
            port,
        );
        if !cfg.naming_persist_file.is_empty() {
            servant = servant.with_persistence(zerodds_corba_cosnaming::servant::Persistence {
                path: std::path::PathBuf::from(&cfg.naming_persist_file),
            });
        }
        Arc::new(servant)
    });

    let dispatch = Arc::new(Dispatch {
        cfg: cfg.clone(),
        runtime: Arc::clone(&runtime),
        by_key,
        in_flight,
        next_id: AtomicU64::new(1),
        metrics: metrics.clone(),
        spec: metrics
            .registry
            .as_ref()
            .map(|r| SpecMetrics::register(r, cfg)),
        reloadable: std::sync::RwLock::new(Reloadable {
            acl_default_deny: cfg.acl_default_deny,
            acl_rules: cfg.acl_rules.clone(),
            tls: None,
        }),
        conns: Mutex::new(BTreeMap::new()),
        next_conn: AtomicU64::new(1),
        draining: AtomicBool::new(false),
        csiv2_store,
        naming_servant,
    });

    // IORs (§4.1).
    let lines =
        ior_lines(cfg, &cfg.iiop_advertise_host).map_err(|e| err(1, format!("IOR: {e}")))?;
    if !cfg.ior_file.is_empty() {
        std::fs::write(&cfg.ior_file, lines.join("\n") + "\n")
            .map_err(|e| err(1, format!("corba.ior.ior_file {}: {e}", cfg.ior_file)))?;
    }
    for l in &lines {
        log("ior", &[("ior", l)]);
    }

    // corba.ior.naming_service (§7.4 of this task's doc-map decision):
    // rebind every receiving mapping's servant, retrying with backoff
    // for corba.ior.naming_retry_ms before aborting the start. A
    // NameService that never comes up costs the daemon its own start,
    // deliberately — a bridge whose servants no client can look up is
    // not a working bridge, silently or otherwise.
    let bound_names = if cfg.publish_to_naming {
        match crate::naming_client::rebind_all(cfg, &cfg.iiop_advertise_host) {
            Ok(names) => {
                log(
                    "naming_bound",
                    &[
                        ("count", &names.len().to_string()),
                        ("service", &cfg.naming_service),
                    ],
                );
                names
            }
            Err(e) => return Err(err(1, format!("corba.ior.naming_service: {e}"))),
        }
    } else {
        Vec::new()
    };

    // Reply dispatchers (one thread per reply topic) and notifiers.
    for rx in reply_rx {
        let d = Arc::clone(&dispatch);
        std::thread::spawn(move || {
            while let Ok(s) = rx.recv() {
                if let UserSample::Alive {
                    payload,
                    big_endian,
                    representation,
                    ..
                } = s
                {
                    on_reply(&d, payload.as_ref(), big_endian, representation);
                }
            }
        });
    }
    for (i, rx) in notify_rx {
        let m = cfg.mappings[i].clone();
        let metrics = metrics.clone();
        let d = Arc::clone(&dispatch);
        std::thread::spawn(move || {
            let mut notifiers: Vec<Notifier> = m
                .targets
                .iter()
                .filter_map(|t| resolve_target(t).ok())
                .map(|target| Notifier {
                    target,
                    operation: m.operation.clone(),
                    conn: None,
                    next_request_id: 1,
                    resolved: false,
                })
                .collect();
            while let Ok(s) = rx.recv() {
                let UserSample::Alive {
                    payload,
                    big_endian,
                    representation,
                    ..
                } = s
                else {
                    continue;
                };
                if representation != 0 {
                    metrics.errors_total.inc();
                    log(
                        "notify_sample_refused",
                        &[
                            ("topic", &m.notify_topic.dds_name),
                            ("err", "XCDR2 sample; the GIOP body needs XCDR1"),
                        ],
                    );
                    continue;
                }
                metrics.dds_samples_in_total.inc();
                if let Some(om) = d.spec.as_ref().and_then(|s| s.ops.get(i)) {
                    om.received.inc();
                }
                let e = if big_endian {
                    Endianness::Big
                } else {
                    Endianness::Little
                };
                for n in &mut notifiers {
                    match n.send(payload.as_ref(), e) {
                        Ok(()) => {
                            metrics.frames_out_total.inc();
                            log(
                                "notify",
                                &[("target", &format!("{}:{}", n.target.host, n.target.port))],
                            );
                        }
                        Err(err) => {
                            metrics.errors_total.inc();
                            log(
                                "notify_failed",
                                &[
                                    ("target", &format!("{}:{}", n.target.host, n.target.port)),
                                    ("err", &err.to_string()),
                                ],
                            );
                        }
                    }
                }
            }
        });
    }
    // The timeouts keep running through the drain; they stop after it.
    let timers_stop = Arc::new(AtomicBool::new(false));
    {
        let d = Arc::clone(&dispatch);
        let sd = Arc::clone(&timers_stop);
        std::thread::spawn(move || timeout_loop(&d, &sd));
    }

    // Listeners: IIOP, and SSLIOP when enabled.
    let iiop = TcpListener::bind(&cfg.iiop_bind)
        .map_err(|e| err(2, format!("bind {}: {e}", cfg.iiop_bind)))?;
    log(
        "listening",
        &[("addr", &cfg.iiop_bind), ("transport", "iiop")],
    );
    let tls = if cfg.ssliop_enabled {
        let server = ssliop_server_config(cfg).map_err(|e| err(4, e))?;
        if let Ok(mut r) = dispatch.reloadable.write() {
            r.tls = Some(Arc::clone(&server));
        }
        let l = TcpListener::bind(&cfg.ssliop_bind)
            .map_err(|e| err(2, format!("bind {}: {e}", cfg.ssliop_bind)))?;
        log(
            "listening",
            &[("addr", &cfg.ssliop_bind), ("transport", "ssliop")],
        );
        Some((l, server))
    } else {
        None
    };
    {
        let d = Arc::clone(&dispatch);
        let sd = Arc::clone(shutdown);
        std::thread::spawn(move || accept_loop(&iiop, &d, &sd, false));
    }
    if let Some((l, _)) = tls {
        let d = Arc::clone(&dispatch);
        let sd = Arc::clone(shutdown);
        std::thread::spawn(move || accept_loop(&l, &d, &sd, true));
    }
    zerodds_dcps::cli::notify_service_manager("READY=1");
    while !shutdown.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
        if let Some(r) = &reload {
            if r.flag.swap(false, Ordering::SeqCst) {
                apply_reload(&dispatch, cfg, &*r.load);
            }
        }
    }
    zerodds_dcps::cli::notify_service_manager("STOPPING=1");
    if !bound_names.is_empty() {
        crate::naming_client::unbind_all(cfg, &bound_names);
        log(
            "naming_unbound",
            &[("count", &bound_names.len().to_string())],
        );
    }
    drain(&dispatch, DRAIN_LIMIT);
    timers_stop.store(true, Ordering::SeqCst);
    dispatch.in_flight.changed.notify_all();
    if !cfg.naming_persist_file.is_empty() {
        if let Some(graph) = &naming_graph {
            let _ = zerodds_corba_cosnaming::persist::save(
                &graph.root(),
                std::path::Path::new(&cfg.naming_persist_file),
            );
        }
    }
    log("shutdown", &[]);
    Ok(())
}

/// The shutdown of §9.2: new requests are refused with `TRANSIENT`, the
/// pending ones get their reply or their `TIMEOUT` for at most `limit`,
/// what is left then gets `TRANSIENT` (completion MAYBE: the backend has the
/// request). Then every connection gets `CloseConnection` and is closed.
fn drain(d: &Dispatch, limit: Duration) {
    d.draining.store(true, Ordering::SeqCst);
    let pending = d.in_flight.pending.lock().map(|p| p.len()).unwrap_or(0);
    log("draining", &[("pending", &pending.to_string())]);
    let deadline = Instant::now() + limit;
    loop {
        let left = d.in_flight.pending.lock().map(|p| p.len()).unwrap_or(0);
        if left == 0 || Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let left: Vec<Pending> = d
        .in_flight
        .pending
        .lock()
        .map(|mut p| core::mem::take(&mut *p).into_values().collect())
        .unwrap_or_default();
    if !left.is_empty() {
        log("drain_expired", &[("pending", &left.len().to_string())]);
    }
    for p in left {
        count_reply(d, &p, Outcome::System);
        let body = system_exception_body("TRANSIENT", 0, Completion::Maybe, Endianness::Little);
        let service_context = sas_service_context(p.sas_reply.as_ref(), Endianness::Little);
        if let Ok(msg) = encode_reply_with_context(
            Version::V1_2,
            Endianness::Little,
            p.giop_request_id,
            ReplyStatusType::SystemException,
            body,
            service_context,
        ) {
            let _ = write_giop(
                &p.conn,
                &msg,
                d.cfg.fragment_size,
                p.giop_request_id,
                Endianness::Little,
            );
        }
    }
    let conns: Vec<Arc<Conn>> = d
        .conns
        .lock()
        .map(|c| c.values().cloned().collect())
        .unwrap_or_default();
    for c in conns {
        let version = c.version.lock().map(|v| *v).unwrap_or(Version::V1_2);
        if let Ok(bytes) = encode_message(
            version,
            Endianness::Little,
            false,
            &Message::CloseConnection(zerodds_corba_giop::CloseConnection),
        ) {
            if let Ok(mut w) = c.writer.lock() {
                let _ = w.write_all(&bytes).and_then(|()| w.flush());
            }
        }
        let _ = c.sock.shutdown(std::net::Shutdown::Both);
    }
}

/// A TLS stream shared by the reading and the writing half of a
/// connection; the reader holds the lock only while TLS has plaintext.
struct TlsShared(Arc<Mutex<rustls::StreamOwned<rustls::ServerConnection, TcpStream>>>);

impl Read for TlsShared {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            {
                let mut s = self
                    .0
                    .lock()
                    .map_err(|_| io::Error::other("TLS stream poisoned"))?;
                match s.conn.reader().read(buf) {
                    Ok(n) => return Ok(n),
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(e) => return Err(e),
                }
            }
            // Wait for ciphertext without holding the lock.
            let sock = self
                .0
                .lock()
                .map_err(|_| io::Error::other("TLS stream poisoned"))?
                .sock
                .try_clone()?;
            let mut raw = [0u8; 16 * 1024];
            let n = (&sock).read(&mut raw)?;
            if n == 0 {
                return Ok(0);
            }
            let mut s = self
                .0
                .lock()
                .map_err(|_| io::Error::other("TLS stream poisoned"))?;
            let mut rest: &[u8] = &raw[..n];
            while !rest.is_empty() {
                s.conn.read_tls(&mut rest)?;
                s.conn
                    .process_new_packets()
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            }
        }
    }
}

impl Write for TlsShared {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("TLS stream poisoned"))?
            .write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("TLS stream poisoned"))?
            .flush()
    }
}

fn accept_loop(listener: &TcpListener, d: &Arc<Dispatch>, shutdown: &AtomicBool, ssl: bool) {
    let _ = listener.set_nonblocking(true);
    while !shutdown.load(Ordering::SeqCst) {
        let (stream, peer) = match listener.accept() {
            Ok(v) => v,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(e) => {
                log("accept_error", &[("err", &e.to_string())]);
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };
        let d = Arc::clone(d);
        // The SSLIOP configuration at accept time: a SIGHUP replaces it for
        // the connections that follow.
        let tls = if ssl {
            d.reloadable.read().ok().and_then(|r| r.tls.clone())
        } else {
            None
        };
        if ssl && tls.is_none() {
            log(
                "conn_error",
                &[
                    ("peer", &peer.to_string()),
                    ("err", "no SSLIOP configuration"),
                ],
            );
            continue;
        }
        std::thread::spawn(move || {
            let _ = stream.set_nonblocking(false);
            let _ = stream.set_nodelay(true);
            d.metrics.connections_total.inc();
            d.metrics.connections_active.inc();
            let id = d.next_conn.fetch_add(1, Ordering::Relaxed);
            let result = (|| -> io::Result<()> {
                let register = |writer: ConnWriter, sock: TcpStream| {
                    let entry = Arc::new(Conn {
                        writer,
                        sock,
                        version: Mutex::new(Version::V1_2),
                    });
                    if let Ok(mut c) = d.conns.lock() {
                        c.insert(id, Arc::clone(&entry));
                    }
                    entry
                };
                match tls {
                    None => {
                        let writer: ConnWriter =
                            Arc::new(Mutex::new(Box::new(stream.try_clone()?)));
                        let entry = register(writer, stream.try_clone()?);
                        serve_connection(&d, Box::new(stream), &entry, Vec::new())
                    }
                    Some(cfg) => {
                        let conn = rustls::ServerConnection::new(cfg)
                            .map_err(|e| io::Error::other(format!("TLS: {e}")))?;
                        let mut tls = rustls::StreamOwned::new(conn, stream);
                        while tls.conn.is_handshaking() {
                            tls.conn.complete_io(&mut tls.sock)?;
                        }
                        // §7.3: the subject DN and the fingerprint
                        // `mtls:<SHA-256>`; an ACL rule may name either.
                        let subject = zerodds_bridge_security::ctx::extract_mtls_subject(&tls.conn)
                            .map(|s| {
                                let mut ids = vec![s.name];
                                ids.extend(s.fingerprint);
                                ids
                            })
                            .unwrap_or_default();
                        let sock = tls.sock.try_clone()?;
                        let shared = Arc::new(Mutex::new(tls));
                        let writer: ConnWriter =
                            Arc::new(Mutex::new(Box::new(TlsShared(Arc::clone(&shared)))));
                        let entry = register(writer, sock);
                        serve_connection(&d, Box::new(TlsShared(shared)), &entry, subject)
                    }
                }
            })();
            if let Ok(mut c) = d.conns.lock() {
                c.remove(&id);
            }
            if let Err(e) = result {
                d.metrics.errors_total.inc();
                log(
                    "conn_error",
                    &[("peer", &peer.to_string()), ("err", &e.to_string())],
                );
            }
            d.metrics.connections_active.dec();
        });
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

    /// The shipped example, through the same reader.
    #[test]
    fn the_shipped_example_reads_as_it_says() {
        let c = load(include_str!(
            "../../../packaging/linux/configs/corba-bridged.yaml.example"
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
    fn the_slug_of_section_5_1() {
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
    fn what_the_daemon_does_not_build_is_refused() {
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

    #[test]
    fn a_system_exception_body_is_the_giop_layout() {
        let b = system_exception_body("TIMEOUT", 7, Completion::Maybe, Endianness::Little);
        let id = b"IDL:omg.org/CORBA/TIMEOUT:1.0";
        assert_eq!(
            u32::from_le_bytes(b[..4].try_into().unwrap()) as usize,
            id.len() + 1
        );
        assert_eq!(&b[4..4 + id.len()], id);
        let off = (4 + id.len() + 1).div_ceil(4) * 4;
        assert_eq!(u32::from_le_bytes(b[off..off + 4].try_into().unwrap()), 7);
        assert_eq!(
            u32::from_le_bytes(b[off + 4..off + 8].try_into().unwrap()),
            2
        );
    }

    /// Every fragment but the last ends on an 8-octet boundary of the
    /// message stream, header included (GIOP 1.2 §15.4.9), and the pieces
    /// put back together are the message.
    #[test]
    fn outbound_fragments_end_on_8_octet_boundaries() {
        struct Sink(Arc<Mutex<Vec<u8>>>);
        impl Write for Sink {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let body: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        let msg = encode_reply_with_context(
            Version::V1_2,
            Endianness::Little,
            9,
            ReplyStatusType::NoException,
            body,
            ServiceContextList::default(),
        )
        .unwrap();
        let out = Arc::new(Mutex::new(Vec::new()));
        let conn: ConnWriter = Arc::new(Mutex::new(Box::new(Sink(Arc::clone(&out)))));
        write_giop(&conn, &msg, 128, 9, Endianness::Little).unwrap();
        let bytes = out.lock().unwrap().clone();
        let mut r = Reassembly::default();
        let (mut at, mut whole) = (0, None);
        while at < bytes.len() {
            let size = u32::from_le_bytes(bytes[at + 8..at + 12].try_into().unwrap()) as usize;
            let frame = bytes[at..at + 12 + size].to_vec();
            let more = frame[6] & 0x02 != 0;
            if more {
                assert_eq!(
                    frame.len() % 8,
                    0,
                    "a fragment ending off an 8-octet boundary"
                );
            }
            at += frame.len();
            whole = r.feed(frame).unwrap();
        }
        assert_eq!(whole.unwrap(), msg);
    }

    #[test]
    fn a_fragmented_message_is_put_back_together() {
        // GIOP 1.2 LE Request split into a first message and one Fragment.
        let body: Vec<u8> = (0..40u8).collect();
        let mut first = b"GIOP\x01\x02\x03\x00".to_vec();
        first.extend_from_slice(&24u32.to_le_bytes());
        first.extend_from_slice(&body[..24]);
        let mut frag = b"GIOP\x01\x02\x01\x07".to_vec();
        frag.extend_from_slice(&(16u32 + 4).to_le_bytes());
        frag.extend_from_slice(&9u32.to_le_bytes());
        frag.extend_from_slice(&body[24..]);
        let mut r = Reassembly::default();
        assert!(r.feed(first).unwrap().is_none());
        let whole = r.feed(frag).unwrap().unwrap();
        assert_eq!(whole[6] & 0x02, 0, "fragment bit cleared");
        assert_eq!(u32::from_le_bytes(whole[8..12].try_into().unwrap()), 40);
        assert_eq!(&whole[12..], &body[..]);
    }

    /// §4.1: the profile carries TAG_CODE_SETS and, with SSLIOP,
    /// TAG_SSL_SEC_TRANS naming the SSL port and what it requires.
    #[test]
    fn the_ior_carries_code_sets_and_the_ssl_component() {
        use zerodds_corba_ior::component_tags::ComponentId;
        use zerodds_corba_ior::components::{StructuredComponent, TaggedComponent};
        let yaml = "corba:\n  iiop:\n    bind: \"127.0.0.1:7001\"\n  ssliop:\n    enabled: true\n    bind: \"127.0.0.1:7002\"\n    cert_file: c.pem\n    key_file: k.pem\n    ca_file: ca.pem\n    require_client_cert: true\n\
                    mappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: op\n";
        let cfg = load(yaml).unwrap();
        let ior = build_ior(&cfg, &cfg.mappings[0], "127.0.0.1").unwrap();
        let body = ior.profiles[0].as_iiop().unwrap().unwrap();
        let structured: Vec<StructuredComponent> = body
            .components
            .iter()
            .map(|c| {
                TaggedComponent {
                    tag: ComponentId::from_u32(c.tag),
                    component_data: c.component_data.clone(),
                }
                .structured()
                .unwrap()
            })
            .collect();
        match &structured[..] {
            [
                StructuredComponent::CodeSets(cs),
                StructuredComponent::Ssl(ssl),
            ] => {
                assert_eq!(cs.for_char_data.native_code_set, 0x0501_0001);
                assert_eq!(cs.for_wchar_data.native_code_set, 0x0001_0109);
                assert_eq!(ssl.port, 7002);
                assert_eq!(ssl.target_requires, 64, "EstablishTrustInClient");
                assert_eq!(ssl.target_supports & 0x0067, 0x0067);
            }
            other => panic!("components {other:?}"),
        }
        let plain = load("mappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: op\n").unwrap();
        let ior = build_ior(&plain, &plain.mappings[0], "127.0.0.1").unwrap();
        let tags: Vec<u32> = ior.profiles[0]
            .as_iiop()
            .unwrap()
            .unwrap()
            .components
            .iter()
            .map(|c| c.tag)
            .collect();
        assert_eq!(tags, [1], "without SSLIOP: code sets only");
    }

    /// §7.2 / CORBA 3.4 Part 2 §10.5: with `corba.csiv2.enabled` the IOR
    /// also carries `TAG_CSI_SEC_MECH_LIST`, naming the GSSUP AS-mechanism,
    /// the configured `target_name` and `target_supports`. Without SSLIOP
    /// its transport_mech is `TAG_NULL_TAG`; with SSLIOP it is the SSL port.
    #[test]
    fn the_ior_carries_the_csiv2_mech_list_when_enabled() {
        use zerodds_corba_ior::component_tags::ComponentId;
        use zerodds_corba_ior::components::{StructuredComponent, TaggedComponent};
        let yaml = "corba:\n  csiv2:\n    enabled: true\n    target_supports: [principal_name]\n    gssup:\n      password_file: /etc/zerodds/gssup.passwd\n      target_name: REALM.LAB\n\
                    mappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: op\n";
        let cfg = load(yaml).unwrap();
        let ior = build_ior(&cfg, &cfg.mappings[0], "127.0.0.1").unwrap();
        let body = ior.profiles[0].as_iiop().unwrap().unwrap();
        let structured: Vec<StructuredComponent> = body
            .components
            .iter()
            .map(|c| {
                TaggedComponent {
                    tag: ComponentId::from_u32(c.tag),
                    component_data: c.component_data.clone(),
                }
                .structured()
                .unwrap()
            })
            .collect();
        let found = structured
            .iter()
            .find(|c| matches!(c, StructuredComponent::CsiSecMechList(_)))
            .expect("TAG_CSI_SEC_MECH_LIST present");
        let StructuredComponent::CsiSecMechList(list) = found else {
            panic!("expected CsiSecMechList, got {found:?}");
        };
        assert!(!list.stateful, "SAS EstablishContext per request (§10.3)");
        assert_eq!(list.mechanism_list.len(), 1);
        let mech = &list.mechanism_list[0];
        assert_eq!(
            mech.transport_mech_tag,
            ComponentId::NullTag.as_u32(),
            "no SSLIOP configured"
        );
        assert_eq!(
            mech.as_context.client_authentication_mech,
            zerodds_corba_csiv2::gssup::GSSUP_OID_DER
        );
        assert_eq!(mech.as_context.target_name, b"REALM.LAB");
        assert_eq!(
            mech.sas_context.supported_identity_types,
            zerodds_corba_csiv2::IDENTITY_TYPE_PRINCIPAL_NAME
        );

        let with_ssl = "corba:\n  ssliop:\n    enabled: true\n    bind: \"127.0.0.1:7002\"\n    cert_file: c.pem\n    key_file: k.pem\n  csiv2:\n    enabled: true\n    gssup:\n      password_file: /etc/zerodds/gssup.passwd\n\
                    mappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: op\n";
        let cfg = load(with_ssl).unwrap();
        let ior = build_ior(&cfg, &cfg.mappings[0], "127.0.0.1").unwrap();
        let body = ior.profiles[0].as_iiop().unwrap().unwrap();
        let mech_list_component = body
            .components
            .iter()
            .find(|c| ComponentId::from_u32(c.tag) == ComponentId::CsiSecMechList)
            .expect("component present");
        let structured = TaggedComponent {
            tag: ComponentId::CsiSecMechList,
            component_data: mech_list_component.component_data.clone(),
        }
        .structured()
        .unwrap();
        let StructuredComponent::CsiSecMechList(list) = &structured else {
            panic!("expected CsiSecMechList, got {structured:?}");
        };
        assert_eq!(
            list.mechanism_list[0].transport_mech_tag,
            ComponentId::SslSecTrans.as_u32(),
            "SSLIOP configured: the mech list names its port"
        );
    }

    /// `corba.iiop.advertise_host` names the host of a wildcard bind; its
    /// environment variable maps onto that key.
    #[test]
    fn the_advertise_host_is_a_setting() {
        let yaml = "corba:\n  iiop:\n    bind: \"0.0.0.0:7001\"\n    advertise_host: bridge.example\n\
                    mappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: op\n";
        let cfg = load(yaml).unwrap();
        let ior = build_ior(&cfg, &cfg.mappings[0], &cfg.iiop_advertise_host).unwrap();
        let body = ior.profiles[0].as_iiop().unwrap().unwrap();
        assert_eq!((body.host.as_str(), body.port), ("bridge.example", 7001));
        let default = load("mappings: []\n").unwrap();
        assert_eq!(default.iiop_advertise_host, "127.0.0.1");
        for (_, key) in ENV {
            assert!(is_key(key), "{key}");
        }
    }

    #[test]
    fn corbaloc_and_ior_targets_resolve() {
        let t = resolve_target("corbaloc:iiop:1.2@127.0.0.1:2809/Listener").unwrap();
        assert_eq!(
            (t.host.as_str(), t.port, t.object_key.as_slice()),
            ("127.0.0.1", 2809, &b"Listener"[..])
        );
        let cfg = load("corba:\n  iiop:\n    bind: \"0.0.0.0:7001\"\nmappings:\n  - repo_id: \"IDL:x/Y:1.0\"\n    operation: op\n").unwrap();
        let line = &ior_lines(&cfg, "10.0.0.5").unwrap()[0];
        let ior = line.split(' ').nth(2).unwrap();
        let t = resolve_target(ior).unwrap();
        assert_eq!((t.host.as_str(), t.port), ("10.0.0.5", 7001));
        assert_eq!(t.object_key, object_key(&cfg.mappings[0]));
    }

    #[test]
    fn acl_rules_match_subjects_and_repo_id_prefixes() {
        let c = load("acl:\n  default_deny: true\n  rules:\n    - subject: \"CN=trader\"\n      allow_invoke: [\"IDL:zerodds/MarketData/*\"]\n").unwrap();
        assert!(subject_allowed(
            &c,
            &["CN=trader".to_string()],
            "IDL:zerodds/MarketData/Quote:1.0"
        ));
        assert!(!subject_allowed(
            &c,
            &["CN=trader".to_string()],
            "IDL:zerodds/Trading/X:1.0"
        ));
        assert!(!subject_allowed(
            &c,
            &[],
            "IDL:zerodds/MarketData/Quote:1.0"
        ));
        let open = load("acl:\n  default_deny: false\n").unwrap();
        assert!(subject_allowed(&open, &[], "IDL:any:1.0"));
    }

    // ========================================================================
    // Notifier: following OBJECT_FORWARD/LOCATION_FORWARD against a fake
    // GIOP peer over a real loopback TCP socket (measured against TAO
    // 2.5's IORTable, `tests/tao_e2e.rs` / `tests/tao/listener_server.cpp`
    // — see the struct doc).
    // ========================================================================

    fn a_notifier(port: u16, object_key: &[u8]) -> Notifier {
        Notifier {
            target: Target {
                host: "127.0.0.1".into(),
                port,
                object_key: object_key.to_vec(),
            },
            operation: "on_value".into(),
            conn: None,
            next_request_id: 1,
            resolved: false,
        }
    }

    /// The plain `Object` CDR form a `LOCATE_REPLY`/`Reply` forward body
    /// carries (see `decode_forward_target`'s doc).
    fn forward_body(host: &str, port: u16, object_key: &[u8], e: Endianness) -> Vec<u8> {
        let profile = TaggedProfile::iiop(
            &IiopProfileBody::new(
                IiopVersion::new(1, 2),
                host.to_string(),
                port,
                object_key.to_vec(),
            ),
            e,
        )
        .unwrap();
        let ior = Ior {
            type_id: "IDL:zerodds_test/Listener:1.0".into(),
            profiles: vec![profile],
        };
        let mut w = zerodds_wire::Writer::new(e);
        ior.encode(&mut w).unwrap();
        w.finish()
    }

    fn accept_one(listener: &TcpListener) -> Connection {
        let (stream, _) = listener.accept().unwrap();
        Connection::from_stream(stream).unwrap()
    }

    /// `Notifier::resolve` follows a `LocateReply`'s `OBJECT_FORWARD` to
    /// the real object key, then confirms it with a plain `OBJECT_HERE`
    /// on the reconnect — the steady state TAO's `IORTable` measured.
    #[test]
    fn resolve_follows_object_forward_then_confirms_object_here() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let real_key = b"RealListener".to_vec();
        let server = std::thread::spawn({
            let real_key = real_key.clone();
            move || {
                let mut conn = accept_one(&listener);
                let (msg, e, v) = conn.read_message_full().unwrap();
                let Message::LocateRequest(lr) = msg else {
                    panic!("expected a LocateRequest, got {:?}", msg.message_type())
                };
                assert_eq!(lr.target, TargetAddress::Key(b"Listener".to_vec()));
                let body = forward_body("127.0.0.1", addr.port(), &real_key, e);
                conn.write_message(
                    v,
                    e,
                    false,
                    &Message::LocateReply(LocateReply {
                        request_id: lr.request_id,
                        locate_status: LocateStatusType::ObjectForward,
                        body,
                    }),
                )
                .unwrap();

                let mut conn = accept_one(&listener);
                let (msg, e, v) = conn.read_message_full().unwrap();
                let Message::LocateRequest(lr) = msg else {
                    panic!("expected a LocateRequest, got {:?}", msg.message_type())
                };
                assert_eq!(lr.target, TargetAddress::Key(real_key));
                conn.write_message(
                    v,
                    e,
                    false,
                    &Message::LocateReply(LocateReply {
                        request_id: lr.request_id,
                        locate_status: LocateStatusType::ObjectHere,
                        body: Vec::new(),
                    }),
                )
                .unwrap();
            }
        });

        let mut notifier = a_notifier(addr.port(), b"Listener");
        notifier.resolve(Endianness::Big).unwrap();
        assert_eq!(notifier.target.object_key, real_key);
        assert!(notifier.resolved);
        server.join().unwrap();
    }

    /// A peer that answers `OBJECT_FORWARD` past [`MAX_FORWARD_HOPS`] is a
    /// misconfiguration (a forwarding loop) — a loud error, not an
    /// infinite retry.
    #[test]
    fn resolve_gives_up_loudly_past_the_forward_hop_bound() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for _ in 0..MAX_FORWARD_HOPS {
                let mut conn = accept_one(&listener);
                let (msg, e, v) = conn.read_message_full().unwrap();
                let Message::LocateRequest(lr) = msg else {
                    panic!("expected a LocateRequest, got {:?}", msg.message_type())
                };
                let body = forward_body("127.0.0.1", addr.port(), b"Loop", e);
                conn.write_message(
                    v,
                    e,
                    false,
                    &Message::LocateReply(LocateReply {
                        request_id: lr.request_id,
                        locate_status: LocateStatusType::ObjectForward,
                        body,
                    }),
                )
                .unwrap();
            }
        });

        let mut notifier = a_notifier(addr.port(), b"Loop");
        let err = notifier.resolve(Endianness::Big).unwrap_err();
        assert!(err.to_string().contains("hops"), "{err}");
        server.join().unwrap();
    }

    /// TAO answers `LOCATION_FORWARD` to the oneway request itself, not
    /// only to a `LocateRequest`, regardless of `response_flags` —
    /// `Notifier::send` must read for it and resend to the real servant.
    #[test]
    fn send_follows_location_forward_on_the_reply_to_the_oneway_itself() {
        let real_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let real_addr = real_listener.local_addr().unwrap();
        let real_key = b"RealListener".to_vec();
        let (tx, rx) = std::sync::mpsc::channel();
        let real_server = std::thread::spawn({
            let real_key = real_key.clone();
            move || {
                let mut conn = accept_one(&real_listener);
                let (msg, _e, _v) = conn.read_message_full().unwrap();
                let Message::Request(req) = msg else {
                    panic!("expected a Request, got {:?}", msg.message_type())
                };
                assert_eq!(req.target, TargetAddress::Key(real_key));
                tx.send((req.operation, req.body)).unwrap();
            }
        });

        let logical_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let logical_addr = logical_listener.local_addr().unwrap();
        let logical_server = std::thread::spawn({
            let real_key = real_key.clone();
            move || {
                let mut conn = accept_one(&logical_listener);
                // The LocateRequest sees no forward — it is the oneway
                // send itself that gets one, as TAO's IORTable does.
                let (msg, e, v) = conn.read_message_full().unwrap();
                let Message::LocateRequest(lr) = msg else {
                    panic!("expected a LocateRequest, got {:?}", msg.message_type())
                };
                conn.write_message(
                    v,
                    e,
                    false,
                    &Message::LocateReply(LocateReply {
                        request_id: lr.request_id,
                        locate_status: LocateStatusType::ObjectHere,
                        body: Vec::new(),
                    }),
                )
                .unwrap();

                let (msg, e, v) = conn.read_message_full().unwrap();
                let Message::Request(req) = msg else {
                    panic!("expected a Request, got {:?}", msg.message_type())
                };
                let body = forward_body("127.0.0.1", real_addr.port(), &real_key, e);
                conn.write_message(
                    v,
                    e,
                    false,
                    &Message::Reply(zerodds_corba_giop::Reply {
                        request_id: req.request_id,
                        reply_status: ReplyStatusType::LocationForward,
                        service_context: ServiceContextList::default(),
                        body,
                    }),
                )
                .unwrap();
            }
        });

        let mut notifier = a_notifier(logical_addr.port(), b"Listener");
        notifier.send(&[7, 8, 9], Endianness::Big).unwrap();
        assert_eq!(notifier.target.object_key, real_key);

        let (operation, body) = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(operation, "on_value");
        assert_eq!(body, vec![7, 8, 9]);

        logical_server.join().unwrap();
        real_server.join().unwrap();
    }

    /// Any reply status other than `LOCATION_FORWARD[_PERM]` to the
    /// oneway send is reported, never treated as a delivered sample.
    #[test]
    fn send_reports_an_unexpected_reply_status_instead_of_treating_it_as_delivered() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut conn = accept_one(&listener);
            let (msg, e, v) = conn.read_message_full().unwrap();
            let Message::LocateRequest(lr) = msg else {
                panic!("expected a LocateRequest, got {:?}", msg.message_type())
            };
            conn.write_message(
                v,
                e,
                false,
                &Message::LocateReply(LocateReply {
                    request_id: lr.request_id,
                    locate_status: LocateStatusType::ObjectHere,
                    body: Vec::new(),
                }),
            )
            .unwrap();

            let (msg, e, v) = conn.read_message_full().unwrap();
            let Message::Request(req) = msg else {
                panic!("expected a Request, got {:?}", msg.message_type())
            };
            conn.write_message(
                v,
                e,
                false,
                &Message::Reply(zerodds_corba_giop::Reply {
                    request_id: req.request_id,
                    reply_status: ReplyStatusType::SystemException,
                    service_context: ServiceContextList::default(),
                    body: Vec::new(),
                }),
            )
            .unwrap();
        });

        let mut notifier = a_notifier(addr.port(), b"X");
        let err = notifier.send(&[1], Endianness::Big).unwrap_err();
        assert!(err.to_string().contains("SystemException"), "{err}");
        server.join().unwrap();
    }
}
