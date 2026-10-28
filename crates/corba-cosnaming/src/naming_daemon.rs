// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-naming-svc`: a standalone OMG CosNaming 1.3 server. Feature
//! `daemon`.
//!
//! Config (YAML), an IIOP (and optional SSLIOP) accept loop over
//! `zerodds-corba-iiop`'s `Acceptor`/`Connection`, per-request CSIv2
//! (`zerodds-corba-csiv2::authenticate_gssup`), a persisted
//! [`crate::graph::NamingGraph`], SIGTERM/SIGHUP handling.
//!
//! No DDS dependency at all — this daemon does not touch a DDS domain,
//! so it does not depend on `zerodds-dcps`; the config loader is built
//! directly on `zerodds-config`'s layered-source primitives instead of
//! `zerodds_dcps::cli::Configured`, which is coupled to `RuntimeConfig`.
//!
//! Unlike `corba-dds-bridge` (which has ACL rules and mTLS identities
//! to fall back on), this daemon has no other identity mechanism, so
//! when `naming.csiv2.enabled` is set, every request must present valid
//! GSSUP credentials — CSIv2 here is required, not additive.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use zerodds_config::ConfigError;
use zerodds_config::read::{Section, first_origin};
use zerodds_corba_csiv2::{GssupUserStore, SasOutcome, authenticate_gssup};
use zerodds_corba_giop::{
    LocateReply, LocateStatusType, Message, Reply, ReplyStatusType, ServiceContext,
    ServiceContextList, ServiceContextTag, TargetAddress, Version,
};
use zerodds_corba_iiop::{Acceptor, AcceptorConfig, Connection};
use zerodds_wire::Endianness;

use crate::graph::NamingGraph;
use crate::servant::{NamingServant, Persistence, ServantReply};

/// The daemon's settings — `corba.ior.naming_service`'s host address is
/// intentionally not here: this daemon *is* the NameService, it does
/// not resolve into another one.
#[derive(Debug, Clone, PartialEq)]
pub struct DaemonConfig {
    /// `log_level`.
    pub log_level: String,
    /// `naming.bind` — IIOP listen address.
    pub bind: String,
    /// `naming.advertise_host` — the host named in every IOR this
    /// daemon hands out, when `bind`'s host is a wildcard.
    pub advertise_host: String,
    /// `naming.persist_file` — empty means no persistence (an
    /// in-memory-only graph, lost on restart).
    pub persist_file: String,
    /// `naming.ssliop.enabled`.
    pub ssliop_enabled: bool,
    /// `naming.ssliop.bind`.
    pub ssliop_bind: String,
    /// `naming.ssliop.cert_file`.
    pub ssliop_cert_file: String,
    /// `naming.ssliop.key_file`.
    pub ssliop_key_file: String,
    /// `naming.ssliop.ca_file`.
    pub ssliop_ca_file: String,
    /// `naming.ssliop.require_client_cert`.
    pub ssliop_require_client_cert: bool,
    /// `naming.csiv2.enabled`.
    pub csiv2_enabled: bool,
    /// `naming.csiv2.target_supports` — accepted `IdentityToken` forms
    /// for identity assertion, as an `IDENTITY_TYPE_*` mask.
    pub csiv2_target_supports: u32,
    /// `naming.csiv2.gssup.password_file`.
    pub csiv2_password_file: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            log_level: "info".into(),
            bind: "0.0.0.0:2809".into(),
            advertise_host: "127.0.0.1".into(),
            persist_file: String::new(),
            ssliop_enabled: false,
            ssliop_bind: "0.0.0.0:2810".into(),
            ssliop_cert_file: String::new(),
            ssliop_key_file: String::new(),
            ssliop_ca_file: String::new(),
            ssliop_require_client_cert: false,
            csiv2_enabled: false,
            csiv2_target_supports: 0,
            csiv2_password_file: String::new(),
        }
    }
}

/// Every key this daemon reads.
pub const KEYS: &[&str] = &[
    "log_level",
    "naming.bind",
    "naming.advertise_host",
    "naming.persist_file",
    "naming.ssliop.enabled",
    "naming.ssliop.bind",
    "naming.ssliop.cert_file",
    "naming.ssliop.key_file",
    "naming.ssliop.ca_file",
    "naming.ssliop.require_client_cert",
    "naming.csiv2.enabled",
    "naming.csiv2.target_supports",
    "naming.csiv2.gssup.password_file",
];

/// Environment variables this daemon reads as settings.
pub const ENV: &[(&str, &str)] = &[("ZERODDS_NAMING_ADVERTISE_HOST", "naming.advertise_host")];

/// Whether `path` is a key [`DaemonConfig::from_config`] reads.
#[must_use]
pub fn is_key(path: &str) -> bool {
    KEYS.contains(&path)
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

const IDENTITY_TYPE_NAMES: &[(&str, u32)] = &[
    ("anonymous", zerodds_corba_csiv2::IDENTITY_TYPE_ANONYMOUS),
    (
        "principal_name",
        zerodds_corba_csiv2::IDENTITY_TYPE_PRINCIPAL_NAME,
    ),
    (
        "x509_cert_chain",
        zerodds_corba_csiv2::IDENTITY_TYPE_X509_CERT_CHAIN,
    ),
    (
        "distinguished_name",
        zerodds_corba_csiv2::IDENTITY_TYPE_DISTINGUISHED_NAME,
    ),
];

impl DaemonConfig {
    /// Reads and validates the configuration.
    ///
    /// # Errors
    /// [`ConfigError`] naming the offending key.
    pub fn from_config(root: Option<&zerodds_config::Node>) -> Result<Self, ConfigError> {
        let mut cfg = Self::default();
        let root = Section::root(root)?;
        root.only(&["log_level", "naming"])?;
        if let Some(v) = root.string("log_level")? {
            cfg.log_level = v;
        }
        let naming = root.section("naming")?;
        naming.only(&["bind", "advertise_host", "persist_file", "ssliop", "csiv2"])?;
        if let Some(v) = naming.string("bind")? {
            cfg.bind = v;
        }
        if let Some(v) = naming.string("advertise_host")? {
            cfg.advertise_host = v;
        }
        if let Some(v) = naming.string("persist_file")? {
            cfg.persist_file = v;
        }
        let ssliop = naming.section("ssliop")?;
        ssliop.only(&[
            "enabled",
            "bind",
            "cert_file",
            "key_file",
            "ca_file",
            "require_client_cert",
        ])?;
        if let Some(v) = ssliop.boolean("enabled")? {
            cfg.ssliop_enabled = v;
        }
        if let Some(v) = ssliop.string("bind")? {
            cfg.ssliop_bind = v;
        }
        if let Some(v) = ssliop.string("cert_file")? {
            cfg.ssliop_cert_file = v;
        }
        if let Some(v) = ssliop.string("key_file")? {
            cfg.ssliop_key_file = v;
        }
        if let Some(v) = ssliop.string("ca_file")? {
            cfg.ssliop_ca_file = v;
        }
        if let Some(v) = ssliop.boolean("require_client_cert")? {
            cfg.ssliop_require_client_cert = v;
        }
        if cfg.ssliop_enabled && (cfg.ssliop_cert_file.is_empty() || cfg.ssliop_key_file.is_empty())
        {
            return Err(invalid(
                "naming.ssliop",
                ssliop
                    .node("enabled")
                    .map_or(zerodds_config::Origin::Default, first_origin),
                "ssliop.enabled needs cert_file and key_file",
            ));
        }
        if cfg.ssliop_require_client_cert && cfg.ssliop_ca_file.is_empty() {
            return Err(invalid(
                "naming.ssliop.require_client_cert",
                ssliop
                    .node("require_client_cert")
                    .map_or(zerodds_config::Origin::Default, first_origin),
                "require_client_cert needs ca_file",
            ));
        }
        let csiv2 = naming.section("csiv2")?;
        csiv2.only(&["enabled", "target_supports", "gssup"])?;
        if let Some(v) = csiv2.boolean("enabled")? {
            cfg.csiv2_enabled = v;
        }
        for (path, item) in csiv2.items("target_supports")? {
            let s = item.as_str().unwrap_or_default();
            let bit = IDENTITY_TYPE_NAMES
                .iter()
                .find(|(name, _)| *name == s)
                .map(|(_, bit)| *bit)
                .ok_or_else(|| {
                    invalid(
                        path.clone(),
                        first_origin(item),
                        "expected one of: anonymous, principal_name, x509_cert_chain, distinguished_name",
                    )
                })?;
            cfg.csiv2_target_supports |= bit;
        }
        let gssup = csiv2.section("gssup")?;
        gssup.only(&["password_file"])?;
        if let Some(v) = gssup.string("password_file")? {
            cfg.csiv2_password_file = v;
        }
        if cfg.csiv2_enabled && cfg.csiv2_password_file.is_empty() {
            return Err(invalid(
                "naming.csiv2.enabled",
                csiv2
                    .node("enabled")
                    .map_or(zerodds_config::Origin::Default, first_origin),
                "csiv2.enabled needs csiv2.gssup.password_file",
            ));
        }
        Ok(cfg)
    }
}

fn host_port(bind: &str, default_port: u16) -> (String, u16) {
    match bind.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().unwrap_or(default_port)),
        None => (bind.to_string(), default_port),
    }
}

fn log(event: &str, fields: &[(&str, &str)]) {
    let mut line = alloc::format!("{{\"event\":\"{event}\"");
    for (k, v) in fields {
        line.push_str(&alloc::format!(
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

fn system_exception_body(name: &str, endianness: Endianness) -> Vec<u8> {
    let mut w = zerodds_wire::Writer::new(endianness);
    w.write_string(&alloc::format!("IDL:omg.org/CORBA/{name}:1.0"));
    w.write_u32(0);
    w.write_u32(1);
    w.finish()
}

fn security_context_id() -> u32 {
    ServiceContextTag::SecurityAttributeService.as_u32()
}

fn locate_status(servant: &NamingServant, key: &[u8]) -> LocateStatusType {
    if servant.graph().context_for_key(key).is_some() || servant.graph().is_iterator_key(key) {
        LocateStatusType::ObjectHere
    } else {
        LocateStatusType::UnknownObject
    }
}

/// Serves one accepted connection until it closes: reads a full GIOP
/// message, runs CSIv2 (when `store` is set), dispatches through
/// `servant`, writes the reply.
fn serve_connection(
    servant: &Arc<NamingServant>,
    csiv2: Option<(&Arc<GssupUserStore>, u32)>,
    mut conn: Connection,
) {
    loop {
        let Ok((msg, endianness, req_version)) = conn.read_message_full() else {
            return;
        };
        let reply_version = if req_version.is_at_least(1, 3) {
            Version::V1_2
        } else {
            req_version
        };
        let req = match msg {
            Message::LocateRequest(lr) => {
                let status = match &lr.target {
                    TargetAddress::Key(k) => locate_status(servant, k),
                    _ => LocateStatusType::UnknownObject,
                };
                let reply = Message::LocateReply(LocateReply {
                    request_id: lr.request_id,
                    locate_status: status,
                    body: Vec::new(),
                });
                let _ = conn.write_message(reply_version, endianness, false, &reply);
                continue;
            }
            Message::Request(req) => req,
            _ => continue,
        };
        let respond = req.response_flags.response_expected();
        let object_key = match &req.target {
            TargetAddress::Key(k) => k.clone(),
            _ => Vec::new(),
        };

        let mut sas_reply = None;
        if let Some((store, target_supports)) = csiv2 {
            let sas_bytes = req
                .service_context
                .0
                .iter()
                .find(|c| c.context_id == security_context_id())
                .map(|c| c.context_data.clone());
            let Some(bytes) = sas_bytes else {
                // CSIv2 enabled and no context offered: refused outright
                // (this daemon has no other identity mechanism).
                if respond {
                    write_no_permission(&mut conn, reply_version, endianness, req.request_id, None);
                }
                continue;
            };
            match authenticate_gssup(&bytes, store, target_supports) {
                SasOutcome::Accepted { reply, .. } => sas_reply = Some(reply),
                SasOutcome::Rejected { reply } => {
                    if respond {
                        write_no_permission(
                            &mut conn,
                            reply_version,
                            endianness,
                            req.request_id,
                            Some(&reply),
                        );
                    }
                    continue;
                }
                SasOutcome::NoContext => {
                    // Not reachable here: `sas_bytes` above is always
                    // `Some(_)` on this path.
                }
            }
        }

        let dispatched = servant.dispatch(&object_key, &req.operation, &req.body, endianness);
        if !respond {
            continue;
        }
        let (reply_status, body) = match dispatched {
            ServantReply::Ok(b) => (ReplyStatusType::NoException, b),
            ServantReply::UserException { body, .. } => (ReplyStatusType::UserException, body),
            ServantReply::BadOperation => (
                ReplyStatusType::SystemException,
                system_exception_body("BAD_OPERATION", endianness),
            ),
            ServantReply::Marshal => (
                ReplyStatusType::SystemException,
                system_exception_body("MARSHAL", endianness),
            ),
            ServantReply::ObjectNotExist => (
                ReplyStatusType::SystemException,
                system_exception_body("OBJECT_NOT_EXIST", endianness),
            ),
            ServantReply::InterfaceRepositoryUnavailable => (
                ReplyStatusType::SystemException,
                system_exception_body("INTF_REPOS", endianness),
            ),
        };
        let service_context = sas_service_context(sas_reply.as_ref(), endianness);
        let reply_msg = Message::Reply(Reply {
            request_id: req.request_id,
            reply_status,
            service_context,
            body,
        });
        let _ = conn.write_message(reply_version, endianness, false, &reply_msg);
    }
}

fn sas_service_context(
    sas: Option<&zerodds_corba_csiv2::sas::SasMessage>,
    endianness: Endianness,
) -> ServiceContextList {
    let mut list = ServiceContextList::default();
    if let Some(sas) = sas {
        if let Ok(bytes) = sas.encode_encapsulation(endianness) {
            list.0
                .push(ServiceContext::new(security_context_id(), bytes));
        }
    }
    list
}

fn write_no_permission(
    conn: &mut Connection,
    version: Version,
    endianness: Endianness,
    request_id: u32,
    sas: Option<&zerodds_corba_csiv2::sas::SasMessage>,
) {
    let reply = Message::Reply(Reply {
        request_id,
        reply_status: ReplyStatusType::SystemException,
        service_context: sas_service_context(sas, endianness),
        body: system_exception_body("NO_PERMISSION", endianness),
    });
    let _ = conn.write_message(version, endianness, false, &reply);
}

/// Errors returned by [`run`].
#[derive(Debug)]
pub struct DaemonError {
    /// Process exit code.
    pub code: u8,
    /// Message.
    pub msg: String,
}

impl core::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.msg)
    }
}

fn err(code: u8, msg: impl Into<String>) -> DaemonError {
    DaemonError {
        code,
        msg: msg.into(),
    }
}

/// Runs the daemon until `shutdown` is set. Blocks the calling thread.
/// On a clean stop, persists the graph one last time (if configured).
///
/// # Errors
/// [`DaemonError`] on a bind/TLS/persistence-load failure.
pub fn run(cfg: &DaemonConfig, shutdown: &Arc<AtomicBool>) -> Result<(), DaemonError> {
    let store: Option<Arc<GssupUserStore>> = if cfg.csiv2_enabled {
        let text = std::fs::read_to_string(&cfg.csiv2_password_file)
            .map_err(|e| err(1, alloc::format!("csiv2 password file: {e}")))?;
        let s = GssupUserStore::parse(&text)
            .map_err(|e| err(1, alloc::format!("csiv2 password file: {e}")))?;
        Some(Arc::new(s))
    } else {
        None
    };

    let root = if cfg.persist_file.is_empty() {
        None
    } else {
        let path = std::path::Path::new(&cfg.persist_file);
        if path.exists() {
            Some(
                crate::persist::load(path)
                    .map_err(|e| err(1, alloc::format!("persisted graph: {e}")))?,
            )
        } else {
            None
        }
    };
    let graph = Arc::new(root.map_or_else(NamingGraph::new, NamingGraph::from_root));

    let (bind_host, bind_port) = host_port(&cfg.bind, 2809);
    let advertise_host = if bind_host.is_empty() || bind_host == "0.0.0.0" || bind_host == "::" {
        cfg.advertise_host.clone()
    } else {
        bind_host.clone()
    };
    let mut servant = NamingServant::new(Arc::clone(&graph), advertise_host, bind_port);
    if !cfg.persist_file.is_empty() {
        servant = servant.with_persistence(Persistence {
            path: std::path::PathBuf::from(&cfg.persist_file),
        });
    }
    let servant = Arc::new(servant);

    let bind_addr: std::net::SocketAddr = alloc::format!("{bind_host}:{bind_port}")
        .parse()
        .map_err(|e| err(2, alloc::format!("naming.bind: {e}")))?;
    let csiv2 = store.clone().map(|s| (s, cfg.csiv2_target_supports));
    let acceptor_servant = Arc::clone(&servant);
    let acceptor_csiv2 = csiv2.clone();
    let acceptor = Acceptor::start(AcceptorConfig::new(bind_addr), move |conn| {
        let pair = acceptor_csiv2.as_ref().map(|(s, t)| (s, *t));
        serve_connection(&acceptor_servant, pair, conn);
    })
    .map_err(|e| err(2, alloc::format!("naming.bind {bind_addr}: {e}")))?;
    log("listening", &[("addr", &bind_addr.to_string())]);

    let ssl_acceptor = if cfg.ssliop_enabled {
        let ca =
            (!cfg.ssliop_ca_file.is_empty()).then(|| std::path::PathBuf::from(&cfg.ssliop_ca_file));
        let tls_config = if cfg.ssliop_require_client_cert {
            let ca = ca.ok_or_else(|| err(4, "ssliop.require_client_cert needs ca_file"))?;
            zerodds_bridge_security::tls::load_server_config_with_client_auth(
                std::path::Path::new(&cfg.ssliop_cert_file),
                std::path::Path::new(&cfg.ssliop_key_file),
                &ca,
            )
        } else {
            zerodds_bridge_security::tls::load_server_config(
                std::path::Path::new(&cfg.ssliop_cert_file),
                std::path::Path::new(&cfg.ssliop_key_file),
            )
        }
        .map_err(|e| err(4, alloc::format!("ssliop: {e}")))?;
        let (ssl_host, ssl_port) = host_port(&cfg.ssliop_bind, 2810);
        let ssl_addr: std::net::SocketAddr = alloc::format!("{ssl_host}:{ssl_port}")
            .parse()
            .map_err(|e| err(2, alloc::format!("naming.ssliop.bind: {e}")))?;
        let acceptor_servant = Arc::clone(&servant);
        let acceptor_csiv2 = csiv2;
        let a = Acceptor::start_tls(AcceptorConfig::new(ssl_addr), tls_config, move |conn| {
            let pair = acceptor_csiv2.as_ref().map(|(s, t)| (s, *t));
            serve_connection(&acceptor_servant, pair, conn);
        })
        .map_err(|e| err(2, alloc::format!("naming.ssliop.bind {ssl_addr}: {e}")))?;
        log("listening_ssliop", &[("addr", &ssl_addr.to_string())]);
        Some(a)
    } else {
        None
    };

    while !shutdown.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }
    acceptor.shutdown();
    if let Some(a) = ssl_acceptor {
        a.shutdown();
    }
    if !cfg.persist_file.is_empty() {
        let _ = crate::persist::save(&graph.root(), std::path::Path::new(&cfg.persist_file));
    }
    log("shutdown", &[]);
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn load(yaml: &str) -> Result<DaemonConfig, String> {
        let mut c = zerodds_config::Config::default();
        c.push(
            zerodds_config::parse_str(yaml, "<string>").map_err(|e| e.to_string())?,
            "<string>".into(),
        )
        .map_err(|e| e.to_string())?;
        DaemonConfig::from_config(c.root()).map_err(|e| e.to_string())
    }

    #[test]
    fn defaults_match_the_documented_ones() {
        let c = DaemonConfig::default();
        assert_eq!(c.bind, "0.0.0.0:2809");
        assert_eq!(c.advertise_host, "127.0.0.1");
        assert_eq!(c.ssliop_bind, "0.0.0.0:2810");
        assert!(!c.csiv2_enabled);
        assert!(!c.ssliop_enabled);
    }

    #[test]
    fn csiv2_enabled_without_a_password_file_is_refused() {
        let err = load("naming:\n  csiv2:\n    enabled: true\n").unwrap_err();
        assert!(err.contains("password_file"));
    }

    #[test]
    fn an_unknown_target_supports_value_is_refused() {
        let err = load("naming:\n  csiv2:\n    target_supports: [nope]\n").unwrap_err();
        assert!(err.contains("expected one of"));
    }

    #[test]
    fn ssliop_require_client_cert_without_ca_file_is_refused() {
        let err = load(
            "naming:\n  ssliop:\n    enabled: true\n    bind: \"0.0.0.0:2810\"\n    \
             cert_file: c.pem\n    key_file: k.pem\n    require_client_cert: true\n",
        )
        .unwrap_err();
        assert!(err.contains("ca_file"));
    }

    #[test]
    fn full_config_parses() {
        let c = load(
            "log_level: debug\n\
             naming:\n\
             \x20 bind: \"127.0.0.1:2809\"\n\
             \x20 advertise_host: \"naming.example\"\n\
             \x20 persist_file: \"/var/lib/zerodds/naming/graph.txt\"\n\
             \x20 csiv2:\n\
             \x20   enabled: true\n\
             \x20   target_supports: [principal_name, distinguished_name]\n\
             \x20   gssup:\n\
             \x20     password_file: \"/etc/zerodds/gssup.passwd\"\n",
        )
        .unwrap();
        assert_eq!(c.log_level, "debug");
        assert_eq!(c.bind, "127.0.0.1:2809");
        assert_eq!(c.advertise_host, "naming.example");
        assert_eq!(c.persist_file, "/var/lib/zerodds/naming/graph.txt");
        assert!(c.csiv2_enabled);
        assert_eq!(
            c.csiv2_target_supports,
            zerodds_corba_csiv2::IDENTITY_TYPE_PRINCIPAL_NAME
                | zerodds_corba_csiv2::IDENTITY_TYPE_DISTINGUISHED_NAME
        );
        assert_eq!(c.csiv2_password_file, "/etc/zerodds/gssup.passwd");
    }

    /// The shipped example, through the same reader — catches drift
    /// between the documented default config and what the daemon
    /// actually accepts.
    #[test]
    fn the_shipped_example_reads_as_it_says() {
        let c = load(include_str!(
            "../../../packaging/linux/configs/naming-svc.yaml.example"
        ))
        .unwrap();
        assert_eq!(c.bind, "0.0.0.0:2809");
        assert_eq!(c.persist_file, "/var/lib/zerodds/naming/graph.txt");
        assert!(!c.csiv2_enabled);
        assert!(!c.ssliop_enabled);
    }
}
