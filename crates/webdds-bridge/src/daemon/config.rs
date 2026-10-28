// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Configuration of `zerodds-webdds-bridged`.
//!
//! Read by the shared layered loader (`zerodds-config`): base file,
//! `webdds-bridged.d/` drop-ins, includes, `${VAR}` substitution, the
//! precedence defaults < environment < files < `--set`, the same
//! scheme every `zerodds-*-bridged` daemon uses (`crates/coap-bridge`,
//! `crates/websocket-bridge`).
//!
//! **Authorization granularity.** The bridge-security substrate
//! (`zerodds_bridge_security::Acl`) is topic-shaped (one ACL entry per
//! DDS topic name) — the right unit for a bridge that proxies exactly
//! one topic per wire resource. DDS-WEB's object model is not
//! topic-shaped: an `Application`/`DomainParticipant`/`Publisher`
//! operation names no topic at all, and a `DataWriter`/`DataReader`
//! reaches its topic only two levels down a path the client chose the
//! names of. This daemon's ACL is keyed by **Application name**
//! instead — Spec §7.3's own access-control model is scoped through
//! the object tree rooted at `Application` ("access control per
//! Principal"), so the Application is the natural access-controlled
//! unit here, not the topic underneath it. `acl.<app>.read`/`.write`
//! name the subjects allowed a `GET`/everything-else on that
//! application's subtree.

use std::collections::HashMap;
use std::path::Path;

pub use zerodds_config::ConfigError;
use zerodds_config::read::Section;

/// The base name of the daemon's file in the system configuration
/// directory: `webdds-bridged.yaml`, `webdds-bridged.d/`.
pub const CONFIG_STEM: &str = "webdds-bridged";

/// Every key this daemon reads, as a dotted path; list-valued keys stop
/// at the list (`types`, `interfaces`).
pub const KEYS: &[&str] = &[
    "listen",
    "domain",
    "log_level",
    "max_body_bytes",
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
    "types",
    "interfaces",
];

/// Whether `path` is one of [`KEYS`].
#[must_use]
pub fn is_key(path: &str) -> bool {
    KEYS.contains(&path)
}

/// A config-named type, from either source
/// (`crate::types_registry::TypeRegistry::load_idl_file` /
/// `load_xml_file`) — exactly one of `idl_file`/`xml_file` is set; the
/// loader refuses an entry that names both or neither.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeEntry {
    /// The IDL scoped name (`Mod::Type`), also the DDS wire type name.
    pub type_name: String,
    /// Path to the `.idl` file it is declared in.
    pub idl_file: Option<String>,
    /// Path to the DDS-XML 1.0 `<types>`/`<dds>` document it is
    /// declared in.
    pub xml_file: Option<String>,
}

/// Parsed daemon config.
#[derive(Debug, Clone, Default)]
pub struct DaemonConfig {
    /// `listen: <addr>` — HTTP bind address, serving REST + SOAP + WSDL
    /// on the one port (dispatched by path prefix).
    pub listen: String,
    /// `domain: <id>` — DDS domain id.
    pub domain: i32,
    /// `log_level: <level>`.
    pub log_level: String,
    /// `max_body_bytes` — request body cap (`crate::http1::read_request`).
    pub max_body_bytes: usize,
    /// `tls.enabled` — if true, `tls_cert_file`+`tls_key_file` must be
    /// set.
    pub tls_enabled: bool,
    /// `tls.cert_file` — PEM cert path.
    pub tls_cert_file: String,
    /// `tls.key_file` — PEM key path.
    pub tls_key_file: String,
    /// `tls.client_ca_file` — PEM CA bundle for mTLS client auth.
    pub tls_client_ca_file: String,
    /// `auth.mode` — `none|bearer|jwt|mtls`.
    pub auth_mode: String,
    /// `auth.bearer_token` — single-token form.
    pub auth_bearer_token: Option<String>,
    /// `auth.bearer_subject` — who is behind the bearer.
    pub auth_bearer_subject: Option<String>,
    /// `auth.jwt.*`.
    pub auth_jwt: zerodds_bridge_security::JwtSettings,
    /// `acl.<app>.{read,write}` — see the module doc.
    pub app_acl: HashMap<String, (Vec<String>, Vec<String>)>,
    /// `metrics.enabled` — toggles the Prometheus/catalog/healthz
    /// endpoint.
    pub metrics_enabled: bool,
    /// Bind address for the admin endpoint. Empty with
    /// `metrics_enabled=true` defaults to
    /// [`crate::daemon::server::DEFAULT_METRICS_LISTEN`].
    pub metrics_addr: String,
    /// `types:` — every type this gateway can resolve from a
    /// config-named IDL file.
    pub types: Vec<TypeEntry>,
    /// `interfaces:` — IDL files whose `interface`s are described in
    /// the SOAP WSDL this gateway serves (`crate::wsdl_bridge`).
    pub interfaces: Vec<String>,
}

impl DaemonConfig {
    /// Default config for development / tests without a file.
    #[must_use]
    pub fn default_for_dev() -> Self {
        Self {
            listen: "127.0.0.1:8080".to_string(),
            domain: 0,
            log_level: "info".to_string(),
            max_body_bytes: crate::http1::DEFAULT_MAX_BODY,
            tls_enabled: false,
            tls_cert_file: String::new(),
            tls_key_file: String::new(),
            tls_client_ca_file: String::new(),
            auth_mode: "none".to_string(),
            auth_bearer_token: None,
            auth_bearer_subject: None,
            auth_jwt: zerodds_bridge_security::JwtSettings::default(),
            app_acl: HashMap::new(),
            metrics_enabled: false,
            metrics_addr: String::new(),
            types: Vec::new(),
            interfaces: Vec::new(),
        }
    }

    /// Loads one file (and its sibling `<stem>.d/`), for tests and
    /// tools. The daemon itself loads through
    /// `zerodds_dcps::cli::Configured`.
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
    /// [`Self::default_for_dev`]. `None` means nothing was loaded.
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] naming the setting, its origin and what
    /// it accepts.
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
        if let Some(v) = root.parse::<i64>("max_body_bytes", "a byte count >= 1")? {
            if v < 1 {
                let origin = root
                    .node("max_body_bytes")
                    .map(zerodds_config::read::first_origin)
                    .unwrap_or(zerodds_config::Origin::Default);
                return Err(ConfigError::Invalid {
                    setting: "max_body_bytes".to_string(),
                    origin,
                    message: format!("got `{v}`, expected an integer >= 1"),
                });
            }
            out.max_body_bytes = v as usize;
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
        if let Some(v) = metrics.string("listen")? {
            out.metrics_addr = v;
        }

        if let Some(zerodds_config::Node::Map(apps)) = root.node("acl") {
            for (app_name, node) in apps {
                let entry = Section::of(Some(node), format!("acl.{app_name}"))?;
                entry.only(&["read", "write"])?;
                let read = entry.strings("read")?.unwrap_or_default();
                let write = entry.strings("write")?.unwrap_or_default();
                out.app_acl.insert(app_name.clone(), (read, write));
            }
        }

        for (path, item) in root.items("types")? {
            let entry = Section::of(Some(item), &path)?;
            entry.only(&["name", "idl_file", "xml_file"])?;
            let name = entry.string("name")?.unwrap_or_default();
            let idl_file = entry.string("idl_file")?.filter(|s| !s.is_empty());
            let xml_file = entry.string("xml_file")?.filter(|s| !s.is_empty());
            if name.is_empty() || (idl_file.is_none() && xml_file.is_none()) {
                return Err(ConfigError::Invalid {
                    setting: path.clone(),
                    origin: zerodds_config::read::first_origin(item),
                    message: "a types entry needs name and exactly one of idl_file/xml_file"
                        .to_string(),
                });
            }
            if idl_file.is_some() && xml_file.is_some() {
                return Err(ConfigError::Invalid {
                    setting: path.clone(),
                    origin: zerodds_config::read::first_origin(item),
                    message: "a types entry names both idl_file and xml_file — exactly one"
                        .to_string(),
                });
            }
            out.types.push(TypeEntry {
                type_name: name,
                idl_file,
                xml_file,
            });
        }

        out.interfaces = root.strings("interfaces")?.unwrap_or_default();

        Ok(out)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_no_types_or_acl() {
        let c = DaemonConfig::default_for_dev();
        assert!(c.types.is_empty());
        assert!(c.app_acl.is_empty());
        assert_eq!(c.listen, "127.0.0.1:8080");
    }

    #[test]
    fn loads_listen_domain_and_types() {
        let c = DaemonConfig::load_from_str(
            "listen: \"0.0.0.0:9000\"\ndomain: 7\ntypes:\n  - name: M::Sensor\n    idl_file: /etc/zerodds/sensor.idl\n",
        )
        .expect("valid");
        assert_eq!(c.listen, "0.0.0.0:9000");
        assert_eq!(c.domain, 7);
        assert_eq!(c.types.len(), 1);
        assert_eq!(c.types[0].type_name, "M::Sensor");
        assert_eq!(
            c.types[0].idl_file,
            Some("/etc/zerodds/sensor.idl".to_string())
        );
        assert_eq!(c.types[0].xml_file, None);
    }

    #[test]
    fn loads_a_type_from_an_xml_file() {
        let c = DaemonConfig::load_from_str(
            "types:\n  - name: M::Sensor\n    xml_file: /etc/zerodds/sensor.xml\n",
        )
        .expect("valid");
        assert_eq!(
            c.types[0].xml_file,
            Some("/etc/zerodds/sensor.xml".to_string())
        );
        assert_eq!(c.types[0].idl_file, None);
    }

    #[test]
    fn a_types_entry_naming_both_idl_and_xml_is_refused() {
        let err = DaemonConfig::load_from_str(
            "types:\n  - name: M::Sensor\n    idl_file: /a.idl\n    xml_file: /a.xml\n",
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::Invalid { .. }));
    }

    #[test]
    fn a_types_entry_naming_neither_idl_nor_xml_is_refused() {
        let err = DaemonConfig::load_from_str("types:\n  - name: M::Sensor\n").unwrap_err();
        assert!(matches!(err, ConfigError::Invalid { .. }));
    }

    #[test]
    fn loads_app_acl() {
        let c = DaemonConfig::load_from_str(
            "acl:\n  app1:\n    read: [\"alice\"]\n    write: [\"alice\"]\n",
        )
        .expect("valid");
        assert_eq!(
            c.app_acl.get("app1"),
            Some(&(vec!["alice".to_string()], vec!["alice".to_string()]))
        );
    }

    #[test]
    fn interfaces_list_loads() {
        let c =
            DaemonConfig::load_from_str("interfaces:\n  - /etc/zerodds/calc.idl\n").expect("valid");
        assert_eq!(c.interfaces, vec!["/etc/zerodds/calc.idl".to_string()]);
    }
}
