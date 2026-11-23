// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The configuration of the daemon (spec §3): the keys, the reader over the
//! layered tree of `zerodds-config`, the checks the start and `--check-config`
//! share.

use super::*;

#[derive(Debug, Clone)]
pub(crate) struct DaemonConfig {
    pub bind: String,
    pub domain: i32,
    /// `grpc.max_concurrent_streams`, `grpc.max_recv_message_size`,
    /// `grpc.max_send_message_size`, `grpc.initial_window_size`,
    /// `grpc.max_frame_size`, `grpc.keepalive.permit_without_calls` —
    /// Spec §3, the limits of every connection.
    pub limits: ServerLimits,
    /// `grpc.keepalive.time_secs` — Spec §3, §6 Liveliness: a PING after
    /// this long without a frame from the client.
    pub keepalive_time_secs: u64,
    /// `grpc.keepalive.timeout_secs` — Spec §3: GOAWAY when the PING is
    /// not acknowledged within this long.
    pub keepalive_timeout_secs: u64,
    /// `grpc.drain_secs` — Spec §3, §9.2: how long the open calls may run
    /// after SIGTERM/SIGINT before they end UNAVAILABLE, and the bound of
    /// the wait for the acknowledgements of the DDS writes after them.
    pub drain_secs: u64,
    pub tls_enabled: bool,
    pub tls_cert_file: String,
    pub tls_key_file: String,
    /// `grpc.tls.client_ca_file` — CA for client certificates (mTLS).
    pub tls_client_ca_file: String,
    /// `grpc.tls.require_client_cert` — with a client CA, whether a
    /// client without a certificate fails the handshake.
    pub tls_require_client_cert: bool,
    /// `grpc.tls.alpn` — Spec §3, §7.1: the ALPN protocols the TLS
    /// listener offers, in its order of preference: `h2`, and `http/1.1`
    /// for the IDL endpoint of §5.2.
    pub tls_alpn: Vec<String>,
    /// `grpc.tls.min_version` and `grpc.tls.cipher_suites` — Spec §3,
    /// §7.1.
    pub tls_options: GrpcTlsOptions,
    pub reflection_enabled: bool,
    pub auth_mode: String,
    /// `auth.bearer_token` / `auth.bearer_subject` — `auth.mode: bearer`.
    pub auth_bearer_token: Option<String>,
    pub auth_bearer_subject: Option<String>,
    /// `auth.jwt.*` — `auth.mode: jwt`.
    pub auth_jwt: zerodds_bridge_security::JwtSettings,
    /// `log_level:` — Spec §3.
    pub log_level: String,
    /// `metrics.enabled:` — Spec §3/§8.2.
    pub metrics_enabled: bool,
    /// `metrics.listen:` — Spec §3/§8.2, an IP address and port; `None`
    /// when not given (or empty). `--metrics` overrides it.
    pub metrics_listen: Option<std::net::SocketAddr>,
    /// `metrics.path:` — Spec §3/§8.2, the path of the Prometheus page.
    pub metrics_path: String,
    pub topics: Vec<TopicMapping>,
    /// `tenants` — Spec §1 L6, §3: DomainParticipants beside the one of
    /// `domain`, each with its topics; a TLS connection whose client
    /// certificate a tenant names is bound to it.
    pub tenants: Vec<TenantConfig>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:50051".into(),
            domain: 0,
            limits: ServerLimits::default(),
            keepalive_time_secs: 60,
            keepalive_timeout_secs: 10,
            drain_secs: DEFAULT_DRAIN_SECS,
            tls_enabled: false,
            tls_cert_file: String::new(),
            tls_key_file: String::new(),
            tls_client_ca_file: String::new(),
            tls_require_client_cert: false,
            tls_alpn: vec!["h2".to_string(), "http/1.1".to_string()],
            tls_options: GrpcTlsOptions::default(),
            reflection_enabled: false,
            auth_mode: "none".into(),
            auth_bearer_token: None,
            auth_bearer_subject: None,
            auth_jwt: zerodds_bridge_security::JwtSettings::default(),
            log_level: "info".into(),
            metrics_enabled: false,
            metrics_listen: None,
            metrics_path: "/metrics".into(),
            topics: Vec::new(),
            tenants: Vec::new(),
        }
    }
}

/// A tenant (Spec §1 L6, §3 `tenants`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TenantConfig {
    /// `name:` — unique; the prefix of its labels and ACL entries.
    pub name: String,
    /// `domain:` — the DDS domain of its participant; default the
    /// top-level `domain`.
    pub domain: i32,
    /// `subjects:` — the client certificates bound to it: the subject DN
    /// (RFC 4514) or the fingerprint `mtls:<SHA-256 hex>`.
    pub subjects: Vec<String>,
    /// `topics:` — as the top-level `topics`.
    pub topics: Vec<TopicMapping>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TopicMapping {
    pub dds_name: String,
    /// `dds_type` (Spec §3): the type name the DDS endpoints register.
    pub dds_type: String,
    pub grpc_service: String,
    /// `grpc_package` (Spec §3, §5.1), default
    /// `zerodds.<lowercase first segment>.v1`: a fully-qualified service
    /// name has to name this package.
    pub grpc_package: String,
    pub direction: String,
    /// `qos.reliability` (Spec §3, §6): RELIABLE unless `best_effort`.
    pub reliable: bool,
    /// `qos.durability` (Spec §3, §6).
    pub durability: zerodds_qos::DurabilityKind,
    /// `qos.history` depth of the writer: the KEEP_LAST depth, `usize::MAX`
    /// for KEEP_ALL; `None` without a depth, the runtime's default.
    pub history_depth: Option<usize>,
    /// `qos.deadline_ms` (Spec §6 Deadline): the deadline the writer offers
    /// and each stream's reader requests.
    pub deadline_ms: Option<u64>,
    /// `qos.lifespan_ms` (Spec §6 Lifespan): the lifespan of the writer.
    pub lifespan_ms: Option<u64>,
    /// `acl: { publish: [...], subscribe: [...] }` — Spec §7.3.
    pub acl: Option<(Vec<String>, Vec<String>)>,
    /// `idl_file` (Spec §3, §5.2): the IDL file that declares `dds_type`,
    /// and its text as read at the start — served at `/idl/<dds_name>`.
    pub idl_file: String,
    pub idl_source: Option<String>,
    /// `keyed` (Spec §3, §10): whether `dds_type` declares a key; the DDS
    /// endpoints are WITH_KEY or NO_KEY endpoints accordingly (DDSI-RTPS
    /// 2.5 §9.3.1.2), as a peer of the type matches them. Default `true`.
    pub keyed: bool,
}

impl TopicMapping {
    /// The package of the service (Spec §5.1): `grpc_package`, or the
    /// default of the topic name.
    pub(crate) fn package(&self) -> String {
        if self.grpc_package.is_empty() {
            service_gen::default_package(&self.dds_name)
        } else {
            self.grpc_package.clone()
        }
    }

    /// The fully-qualified service name, `<package>.<service>`.
    pub(crate) fn full_service(&self) -> String {
        format!("{}.{}", self.package(), self.grpc_service)
    }
}

impl Default for TopicMapping {
    fn default() -> Self {
        Self {
            dds_name: String::new(),
            dds_type: dds_pump::BRIDGE_TYPE_NAME.to_string(),
            grpc_service: String::new(),
            grpc_package: String::new(),
            direction: "bidir".into(),
            reliable: true,
            durability: zerodds_qos::DurabilityKind::Volatile,
            history_depth: None,
            deadline_ms: None,
            lifespan_ms: None,
            acl: None,
            idl_file: String::new(),
            idl_source: None,
            keyed: true,
        }
    }
}

/// Spec §9.2: the drain of 30 s.
pub(crate) const DEFAULT_DRAIN_SECS: u64 = 30;

/// The longest `grpc.drain_secs` (Spec §3): five minutes.
pub(crate) const MAX_DRAIN_SECS: u64 = 300;

/// The base name of the daemon's file: `grpc-bridged.yaml`, `grpc-bridged.d/`.
pub(crate) const CONFIG_STEM: &str = "grpc-bridged";

/// Every key this daemon reads (Spec §3); `topics` and `tenants` stop at
/// the list.
pub(crate) const KEYS: &[&str] = &[
    "domain",
    "log_level",
    "grpc.bind",
    "grpc.max_concurrent_streams",
    "grpc.max_recv_message_size",
    "grpc.max_send_message_size",
    "grpc.initial_window_size",
    "grpc.max_frame_size",
    "grpc.keepalive.time_secs",
    "grpc.keepalive.timeout_secs",
    "grpc.keepalive.permit_without_calls",
    "grpc.drain_secs",
    "grpc.tls.enabled",
    "grpc.tls.cert_file",
    "grpc.tls.key_file",
    "grpc.tls.client_ca_file",
    "grpc.tls.require_client_cert",
    "grpc.tls.alpn",
    "grpc.tls.min_version",
    "grpc.tls.cipher_suites",
    "grpc.reflection.enabled",
    "auth.mode",
    "auth.bearer_token",
    "auth.bearer_subject",
    "auth.jwt.public_key",
    "auth.jwt.issuer",
    "auth.jwt.audience",
    "auth.jwt.required_claim",
    "metrics.enabled",
    "metrics.listen",
    "metrics.path",
    "topics",
    "tenants",
];

pub(crate) fn is_key(path: &str) -> bool {
    KEYS.contains(&path)
}

/// A refused value at `setting`, with the origin of `node`.
pub(crate) fn invalid(
    setting: String,
    node: Option<&zerodds_config::Node>,
    message: String,
) -> zerodds_config::ConfigError {
    zerodds_config::ConfigError::Invalid {
        setting,
        origin: node.map_or(
            zerodds_config::Origin::Default,
            zerodds_config::read::first_origin,
        ),
        message,
    }
}

/// A positive number of `key` in `section` that lies in `range`.
pub(crate) fn bounded(
    section: &zerodds_config::read::Section<'_>,
    key: &str,
    what: &str,
    range: std::ops::RangeInclusive<u64>,
) -> Result<Option<u64>, zerodds_config::ConfigError> {
    let Some(v) = section.parse::<u64>(key, what)? else {
        return Ok(None);
    };
    if !range.contains(&v) {
        return Err(invalid(
            section.path_of(key),
            section.node(key),
            format!("{v} lies outside {}..={}", range.start(), range.end()),
        ));
    }
    Ok(Some(v))
}

/// One entry of `topics` (Spec §3): a map, or the shorthand `DDS=SERVICE`
/// / `DDS:SERVICE` of `--topic` (Spec §2).
pub(crate) fn read_topic(
    path: &str,
    item: &zerodds_config::Node,
) -> Result<TopicMapping, zerodds_config::ConfigError> {
    use zerodds_config::read::{Section, first_origin, split_topic_shorthand};
    let mut t = TopicMapping::default();
    if let zerodds_config::Node::Scalar(text, _) = item {
        // `--topic DDS=SERVICE` (Spec §2), or `DDS:SERVICE` split at the
        // lone colon so that `Chat::Message` stays whole.
        let (dds, service) = match text.rsplit_once('=') {
            Some((d, s)) => (d.trim().to_string(), Some(s.trim().to_string())),
            None => split_topic_shorthand(text),
        };
        t.dds_name = dds;
        t.grpc_service = service.unwrap_or_default();
    } else {
        let entry = Section::of(Some(item), path.to_string())?;
        entry.only(&[
            "dds_name",
            "dds_type",
            "grpc_service",
            "grpc_package",
            "direction",
            "qos",
            "acl",
            "idl_file",
            "keyed",
        ])?;
        if let Some(file) = entry.string("idl_file")? {
            // Spec §5.2: the text the IDL endpoint serves, read now so that
            // a file that cannot be read stops the start.
            let source = std::fs::read_to_string(&file).map_err(|e| {
                invalid(
                    entry.path_of("idl_file"),
                    entry.node("idl_file"),
                    format!("{file}: {e}"),
                )
            })?;
            t.idl_file = file;
            t.idl_source = Some(source);
        }
        t.dds_name = entry.string("dds_name")?.unwrap_or_default();
        if let Some(v) = entry.string("dds_type")? {
            t.dds_type = v;
        }
        t.grpc_service = entry.string("grpc_service")?.unwrap_or_default();
        t.grpc_package = entry.string("grpc_package")?.unwrap_or_default();
        let qos = entry.section("qos")?;
        qos.only(&[
            "reliability",
            "durability",
            "history",
            "deadline_ms",
            "lifespan_ms",
        ])?;
        let i32_max = u64::try_from(i32::MAX).unwrap_or(u64::MAX);
        t.deadline_ms = bounded(&qos, "deadline_ms", "milliseconds", 1..=i32_max)?;
        t.lifespan_ms = bounded(&qos, "lifespan_ms", "milliseconds", 1..=i32_max)?;
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
        let keep_last = history.choice("kind", &["keep_last", "keep_all"])?;
        let depth = history.parse::<usize>("depth", "a history depth")?;
        t.history_depth = match keep_last.as_deref() {
            Some("keep_all") => {
                if depth.is_some() {
                    return Err(invalid(
                        history.path_of("depth"),
                        history.node("depth"),
                        "KEEP_ALL has no depth".to_string(),
                    ));
                }
                Some(usize::MAX)
            }
            _ => depth,
        };
        if let Some(v) = entry.choice("direction", &["in", "out", "bidir"])? {
            t.direction = v;
        }
        if let Some(v) = entry.boolean("keyed")? {
            t.keyed = v;
        }
        let acl = entry.section("acl")?;
        acl.only(&["publish", "subscribe"])?;
        if entry.node("acl").is_some() {
            t.acl = Some((
                acl.strings("publish")?.unwrap_or_default(),
                acl.strings("subscribe")?.unwrap_or_default(),
            ));
        }
    }
    if t.dds_name.is_empty() {
        return Err(zerodds_config::ConfigError::Invalid {
            setting: path.to_string(),
            origin: first_origin(item),
            message: "a topic entry needs dds_name".to_string(),
        });
    }
    // A dotted `grpc_service` is the fully-qualified name
    // `<package>.<Service>` (Spec §5.1), unless `grpc_package` names the
    // package itself.
    if let Some((package, service)) = t.grpc_service.clone().rsplit_once('.') {
        if !t.grpc_package.is_empty() {
            return Err(zerodds_config::ConfigError::Invalid {
                setting: format!("{path}.grpc_service"),
                origin: first_origin(item),
                message: format!(
                    "`{}` names a package and grpc_package names one too",
                    t.grpc_service
                ),
            });
        }
        t.grpc_package = package.to_string();
        t.grpc_service = service.to_string();
    }
    // Spec §5.1: without `grpc_service` / `grpc_package` the defaults of
    // the topic name.
    if t.grpc_service.is_empty() {
        t.grpc_service = service_gen::default_service(&t.dds_name);
    }
    if t.grpc_package.is_empty() {
        t.grpc_package = service_gen::default_package(&t.dds_name);
    }
    for (what, value) in [
        ("grpc_service", &t.grpc_service),
        ("grpc_package", &t.grpc_package),
    ] {
        if !is_proto_full_ident(value, what == "grpc_package") {
            return Err(zerodds_config::ConfigError::Invalid {
                setting: format!("{path}.{what}"),
                origin: first_origin(item),
                message: format!(
                    "`{value}` is no protobuf {} (letters, digits and `_`, beginning with a letter or `_`)",
                    if what == "grpc_package" {
                        "package name"
                    } else {
                        "identifier"
                    }
                ),
            });
        }
    }
    Ok(t)
}

/// A protobuf identifier (`dotted`: a full name, identifiers joined by
/// `.`), Protocol Buffers Language Specification, "Identifiers".
pub(crate) fn is_proto_full_ident(s: &str, dotted: bool) -> bool {
    let ident = |p: &str| {
        p.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    if dotted {
        s.split('.').all(ident)
    } else {
        ident(s)
    }
}

/// Spec §5.1: two mappings of one participant may not name the same
/// fully-qualified service.
pub(crate) fn refuse_duplicate_services(
    topics: &[TopicMapping],
    path: String,
    node: Option<&zerodds_config::Node>,
) -> Result<(), zerodds_config::ConfigError> {
    for (i, t) in topics.iter().enumerate() {
        if topics[..i]
            .iter()
            .any(|o| o.full_service() == t.full_service())
        {
            return Err(invalid(
                path,
                node,
                format!(
                    "two topic entries map to the service {} ({} is the second)",
                    t.full_service(),
                    t.dds_name
                ),
            ));
        }
    }
    Ok(())
}

impl DaemonConfig {
    /// Reads this daemon's keys out of a merged tree, over the defaults.
    pub(crate) fn from_config(
        config: Option<&zerodds_config::Config>,
    ) -> Result<Self, zerodds_config::ConfigError> {
        use zerodds_config::read::Section;
        let root = Section::root(config.and_then(zerodds_config::Config::root))?;
        let mut cfg = Self::default();
        if let Some(v) = root.parse::<i32>("domain", "a DDS domain id")? {
            cfg.domain = v;
        }
        if let Some(v) = root.choice("log_level", &["trace", "debug", "info", "warn", "error"])? {
            cfg.log_level = v;
        }
        let grpc = root.section("grpc")?;
        grpc.only(&[
            "bind",
            "max_concurrent_streams",
            "max_recv_message_size",
            "max_send_message_size",
            "initial_window_size",
            "max_frame_size",
            "keepalive",
            "drain_secs",
            "tls",
            "reflection",
        ])?;
        if let Some(v) = grpc.string("bind")? {
            cfg.bind = v;
        }
        // Spec §3 with the ranges of RFC 9113 §6.5.2: a value no peer
        // accepts stops the start.
        let u32_max = u64::from(u32::MAX);
        if let Some(v) = bounded(
            &grpc,
            "max_concurrent_streams",
            "a stream count",
            1..=u32_max,
        )? {
            cfg.limits.max_concurrent_streams = v as u32;
        }
        if let Some(v) = bounded(&grpc, "max_recv_message_size", "octets", 1..=u32_max)? {
            cfg.limits.max_recv_message_size = v as u32;
        }
        if let Some(v) = bounded(&grpc, "max_send_message_size", "octets", 1..=u32_max)? {
            cfg.limits.max_send_message_size = v as u32;
        }
        if let Some(v) = bounded(&grpc, "initial_window_size", "octets", 1..=0x7fff_ffff)? {
            cfg.limits.initial_window_size = v as u32;
        }
        if let Some(v) = bounded(&grpc, "max_frame_size", "octets", 16_384..=16_777_215)? {
            cfg.limits.max_frame_size = v as u32;
        }
        if let Some(v) = bounded(&grpc, "drain_secs", "seconds", 0..=MAX_DRAIN_SECS)? {
            cfg.drain_secs = v;
        }
        let keepalive = grpc.section("keepalive")?;
        keepalive.only(&["time_secs", "timeout_secs", "permit_without_calls"])?;
        if let Some(v) = bounded(&keepalive, "time_secs", "seconds", 1..=u64::from(u32::MAX))? {
            cfg.keepalive_time_secs = v;
        }
        if let Some(v) = bounded(
            &keepalive,
            "timeout_secs",
            "seconds",
            1..=u64::from(u32::MAX),
        )? {
            cfg.keepalive_timeout_secs = v;
        }
        if let Some(v) = keepalive.boolean("permit_without_calls")? {
            cfg.limits.permit_pings_without_calls = v;
        }
        let tls = grpc.section("tls")?;
        tls.only(&[
            "enabled",
            "cert_file",
            "key_file",
            "client_ca_file",
            "require_client_cert",
            "alpn",
            "min_version",
            "cipher_suites",
        ])?;
        if let Some(v) = tls.boolean("enabled")? {
            cfg.tls_enabled = v;
        }
        if let Some(v) = tls.string("cert_file")? {
            cfg.tls_cert_file = v;
        }
        if let Some(v) = tls.string("key_file")? {
            cfg.tls_key_file = v;
        }
        if let Some(v) = tls.string("client_ca_file")? {
            cfg.tls_client_ca_file = v;
        }
        if let Some(v) = tls.boolean("require_client_cert")? {
            cfg.tls_require_client_cert = v;
        }
        if let Some(v) = tls.strings("alpn")? {
            // gRPC runs over HTTP/2 (gRPC over HTTP/2, "Protocol"); the
            // listener also speaks HTTP/1.1 for the IDL endpoint (§5.2). A
            // protocol it does not speak would end the connection after the
            // handshake.
            let unknown = v.iter().any(|p| p != "h2" && p != "http/1.1");
            let twice = v.iter().enumerate().any(|(i, p)| v[..i].contains(p));
            if unknown || twice || !v.iter().any(|p| p == "h2") {
                return Err(invalid(
                    tls.path_of("alpn"),
                    tls.node("alpn"),
                    format!("{v:?}: the list names h2 and may name http/1.1, each once (spec §3)"),
                ));
            }
            cfg.tls_alpn = v;
        }
        // Spec §7.1: at least TLS 1.2; the suites by their IANA names. A
        // name this build does not offer stops the start (and
        // `--check-config`).
        if let Some(v) = tls.choice("min_version", &["1.2", "1.3"])? {
            cfg.tls_options.min_version = TlsMinVersion::of(&v).unwrap_or_default();
        }
        if let Some(v) = tls.strings("cipher_suites")? {
            // Spec §7.1: an empty list names no suite; the provider's
            // suites are the default of a missing key.
            if v.is_empty() {
                return Err(invalid(
                    tls.path_of("cipher_suites"),
                    tls.node("cipher_suites"),
                    "an empty list names no cipher suite (spec §7.1)".to_string(),
                ));
            }
            cfg.tls_options.cipher_suites = v;
        }
        if let Err(e) = select_tls_suites(&cfg.tls_options) {
            let key = if tls.node("cipher_suites").is_some() {
                "cipher_suites"
            } else {
                "min_version"
            };
            return Err(invalid(tls.path_of(key), tls.node(key), e));
        }
        let reflection = grpc.section("reflection")?;
        reflection.only(&["enabled"])?;
        if let Some(v) = reflection.boolean("enabled")? {
            cfg.reflection_enabled = v;
        }
        let auth = root.section("auth")?;
        auth.only(&["mode", "bearer_token", "bearer_subject", "jwt"])?;
        if let Some(v) = auth.choice("mode", &["none", "bearer", "jwt", "mtls"])? {
            cfg.auth_mode = v;
        }
        cfg.auth_bearer_token = auth.string("bearer_token")?;
        cfg.auth_bearer_subject = auth.string("bearer_subject")?;
        let jwt = auth.section("jwt")?;
        jwt.only(&["public_key", "issuer", "audience", "required_claim"])?;
        cfg.auth_jwt = zerodds_bridge_security::JwtSettings {
            public_key: jwt.string("public_key")?.unwrap_or_default(),
            issuer: jwt.string("issuer")?,
            audience: jwt.string("audience")?,
            required_claim: jwt.string("required_claim")?,
        };
        let metrics = root.section("metrics")?;
        metrics.only(&["enabled", "listen", "path"])?;
        if let Some(v) = metrics.boolean("enabled")? {
            cfg.metrics_enabled = v;
        }
        if let Some(v) = metrics.string("listen")?.filter(|v| !v.is_empty()) {
            // Spec §8.2: the admin listener binds an IP address and port; a
            // host name is refused, not resolved.
            let addr = v.parse::<std::net::SocketAddr>().map_err(|_| {
                invalid(
                    metrics.path_of("listen"),
                    metrics.node("listen"),
                    format!("`{v}` is no IP address and port (such as {DEFAULT_METRICS_LISTEN})"),
                )
            })?;
            cfg.metrics_listen = Some(addr);
        }
        if let Some(v) = metrics.string("path")? {
            let reserved = ["/", "/catalog", "/healthz"];
            if !v.starts_with('/') || reserved.contains(&v.as_str()) {
                return Err(invalid(
                    metrics.path_of("path"),
                    metrics.node("path"),
                    format!("`{v}` is no absolute path apart from / /catalog /healthz"),
                ));
            }
            cfg.metrics_path = v;
        }
        for (path, item) in root.items("topics")? {
            cfg.topics.push(read_topic(&path, item)?);
        }
        refuse_duplicate_services(&cfg.topics, root.path_of("topics"), root.node("topics"))?;
        // Spec §1 L6: the tenants, each a participant with its topics.
        for (path, item) in root.items("tenants")? {
            let entry = Section::of(Some(item), path.clone())?;
            entry.only(&["name", "domain", "subjects", "topics"])?;
            let name = entry
                .string("name")?
                .filter(|n| !n.is_empty())
                .ok_or_else(|| {
                    invalid(
                        entry.path_of("name"),
                        Some(item),
                        "a tenant needs a name".to_string(),
                    )
                })?;
            if name.contains('/') || cfg.tenants.iter().any(|t| t.name == name) {
                return Err(invalid(
                    entry.path_of("name"),
                    entry.node("name"),
                    format!("`{name}` is a second tenant of that name or contains `/`"),
                ));
            }
            let domain = entry
                .parse::<i32>("domain", "a DDS domain id")?
                .unwrap_or(cfg.domain);
            let subjects = entry.strings("subjects")?.unwrap_or_default();
            if subjects.is_empty() {
                return Err(invalid(
                    entry.path_of("subjects"),
                    Some(item),
                    "a tenant needs the client certificates bound to it".to_string(),
                ));
            }
            if let Some(s) = subjects
                .iter()
                .find(|s| cfg.tenants.iter().any(|t| t.subjects.contains(s)))
            {
                return Err(invalid(
                    entry.path_of("subjects"),
                    entry.node("subjects"),
                    format!("`{s}` is bound to another tenant"),
                ));
            }
            let mut topics = Vec::new();
            for (topic_path, topic) in entry.items("topics")? {
                topics.push(read_topic(&topic_path, topic)?);
            }
            refuse_duplicate_services(&topics, entry.path_of("topics"), entry.node("topics"))?;
            cfg.tenants.push(TenantConfig {
                name,
                domain,
                subjects,
                topics,
            });
        }
        // Spec §1 L6: a connection is bound to a tenant by its client
        // certificate, which the server asks for only over TLS with a
        // client CA; without both no connection could reach a tenant.
        if !cfg.tenants.is_empty() && (!cfg.tls_enabled || cfg.tls_client_ca_file.is_empty()) {
            return Err(invalid(
                root.path_of("tenants"),
                root.node("tenants"),
                "tenants are bound by the client certificate and need grpc.tls.enabled and grpc.tls.client_ca_file".to_string(),
            ));
        }
        Ok(cfg)
    }
}

/// Spec §4.1: cleartext h2c only in dev mode — `grpc.tls.enabled: false`
/// with a loopback `grpc.bind`. A configuration error (exit 1), refused by
/// `--check-config` as well.
pub(crate) fn transport_settings(cfg: &DaemonConfig) -> Result<(), (u8, String)> {
    if cfg.tls_enabled || bind_is_loopback(&cfg.bind) {
        return Ok(());
    }
    Err((
        1,
        format!(
            "config: grpc.bind {}: cleartext h2c only on a loopback address (spec §4.1); \
             set grpc.tls.enabled or bind 127.0.0.1",
            cfg.bind
        ),
    ))
}

/// Whether `bind` (`host:port`) is a loopback address: 127.0.0.0/8, `::1`
/// or the name `localhost`.
pub(crate) fn bind_is_loopback(bind: &str) -> bool {
    if let Ok(addr) = bind.parse::<std::net::SocketAddr>() {
        return addr.ip().is_loopback();
    }
    bind.rsplit_once(':')
        .is_some_and(|(host, _)| host.eq_ignore_ascii_case("localhost"))
}

/// Documented fallback when `metrics.enabled: true` names no `listen:`.
pub(crate) const DEFAULT_METRICS_LISTEN: &str = "127.0.0.1:9095";

/// [`DEFAULT_METRICS_LISTEN`] as an address.
pub(crate) const DEFAULT_METRICS_ADDR: std::net::SocketAddr = std::net::SocketAddr::V4(
    std::net::SocketAddrV4::new(std::net::Ipv4Addr::new(127, 0, 0, 1), 9095),
);

/// Admin-endpoint bind address. Precedence: CLI `--metrics` beats the
/// config file `metrics.listen:` (Spec §3 / §8.2); `metrics.enabled: false`
/// keeps the endpoint off. An enabled block without `listen:` falls back
/// to the documented default.
pub(crate) fn metrics_addr_for(cfg: &DaemonConfig) -> Option<std::net::SocketAddr> {
    if !cfg.metrics_enabled {
        return None;
    }
    Some(cfg.metrics_listen.unwrap_or(DEFAULT_METRICS_ADDR))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::bridge_daemon::test_support::parse_yaml_subset;
    use crate::bridge_daemon::*;

    #[test]
    fn grpcbridge_3_config_default_has_default_bind() {
        let c = DaemonConfig::default();
        assert_eq!(c.bind, "0.0.0.0:50051");
        assert!(!c.tls_enabled);
    }

    #[test]
    fn grpcbridge_3_parse_minimal_grpc_config() {
        let yaml = "\
domain: 5
grpc:
  bind: \"127.0.0.1:50059\"
  reflection:
    enabled: true
auth:
  mode: \"jwt\"
";
        let cfg = parse_yaml_subset(yaml).expect("parse");
        assert_eq!(cfg.domain, 5);
        assert_eq!(cfg.bind, "127.0.0.1:50059");
        assert!(cfg.reflection_enabled);
        assert_eq!(cfg.auth_mode, "jwt");
    }

    #[test]
    fn grpcbridge_3_parse_topics_array() {
        let yaml = "\
topics:
  - dds_name: \"Chat::Message\"
    grpc_service: \"ChatMessageStream\"
    direction: \"bidir\"
  - dds_name: \"Sensor::Reading\"
    grpc_service: \"SensorReadingStream\"
    direction: \"out\"
";
        let cfg = parse_yaml_subset(yaml).expect("parse");
        assert_eq!(cfg.topics.len(), 2);
        assert_eq!(cfg.topics[0].dds_name, "Chat::Message");
        assert_eq!(cfg.topics[0].grpc_service, "ChatMessageStream");
        assert_eq!(cfg.topics[1].direction, "out");
    }

    /// Spec §3: `dds_type`, `grpc_package` and `qos` of a topic entry reach
    /// the mapping; a fully-qualified service name has to name the package.
    #[test]
    fn a_topic_entry_takes_type_package_and_qos() {
        let yaml = "\
topics:
  - dds_name: \"Chat::Message\"
    dds_type: \"chat::Message\"
    grpc_service: \"ChatMessageStream\"
    grpc_package: \"zerodds.chat.v1\"
    direction: \"bidir\"
    qos:
      reliability: \"best_effort\"
      durability: \"transient_local\"
      history: { kind: \"keep_last\", depth: 7 }
";
        let cfg = parse_yaml_subset(yaml).expect("parse");
        let t = &cfg.topics[0];
        assert_eq!(t.dds_type, "chat::Message");
        assert_eq!(t.grpc_package, "zerodds.chat.v1");
        assert!(!t.reliable);
        assert_eq!(t.durability, zerodds_qos::DurabilityKind::TransientLocal);
        assert_eq!(t.history_depth, Some(7));
        assert!(topic_for(&cfg.topics, "zerodds.chat.v1.ChatMessageStream").is_some());
        assert!(topic_for(&cfg.topics, "zerodds.other.v1.ChatMessageStream").is_none());
        assert!(topic_for(&cfg.topics, "ChatMessageStream").is_some());
    }

    /// Spec §3, §6: `keep_all` is KEEP_ALL on the writer into DDS
    /// (`usize::MAX`, the runtime's KEEP_ALL), not the runtime's default
    /// depth; a depth beside it is refused.
    #[test]
    fn grpcbridge_3_keep_all_reaches_the_writer_and_takes_no_depth() {
        let base = "topics:\n  - dds_name: T\n    grpc_service: S\n    qos:\n      history: ";
        let cfg = parse_yaml_subset(&format!("{base}{{ kind: keep_all }}\n")).expect("parse");
        assert_eq!(cfg.topics[0].history_depth, Some(usize::MAX));
        let cfg = parse_yaml_subset(&format!("{base}{{ kind: keep_last }}\n")).expect("parse");
        assert_eq!(cfg.topics[0].history_depth, None);
        let e = parse_yaml_subset(&format!("{base}{{ kind: keep_all, depth: 3 }}\n"))
            .expect_err("keep_all with a depth");
        assert!(e.to_string().contains("KEEP_ALL has no depth"), "{e}");
        assert!(e.to_string().contains("history.depth"), "{e}");
    }

    /// Spec §3 `keyed` (§10): `false` makes the topic's endpoints NO_KEY
    /// endpoints; the default is `true`; a value that is no boolean stops
    /// the start.
    #[test]
    fn grpcbridge_10_keyed_is_read_and_defaults_to_true() {
        let cfg = parse_yaml_subset(
            "topics:\n  - dds_name: \"A::B\"\n    keyed: false\n  - dds_name: \"C::D\"\n",
        )
        .expect("parse");
        assert!(!cfg.topics[0].keyed);
        assert!(cfg.topics[1].keyed);
        assert!(parse_yaml_subset("topics:\n  - dds_name: \"A::B\"\n    keyed: maybe\n").is_err());
    }

    #[test]
    fn grpcbridge_3_parse_invalid_yaml_returns_err() {
        let res = parse_yaml_subset("just a string");
        assert!(res.is_err());
    }

    /// §4.1: cleartext h2c only on a loopback bind; with TLS any bind.
    #[test]
    fn grpcbridge_4_1_h2c_only_on_a_loopback_bind() {
        for bind in [
            "127.0.0.1:50051",
            "127.3.0.1:1",
            "[::1]:50051",
            "localhost:7",
        ] {
            assert!(bind_is_loopback(bind), "{bind}");
        }
        for bind in [
            "0.0.0.0:50051",
            "[::]:50051",
            "10.0.0.5:1",
            "example.org:443",
        ] {
            assert!(!bind_is_loopback(bind), "{bind}");
        }
        let mut cfg = DaemonConfig::default();
        assert_eq!(cfg.bind, "0.0.0.0:50051");
        let err = transport_settings(&cfg).unwrap_err();
        assert_eq!(err.0, 1);
        assert!(err.1.contains("loopback"), "{}", err.1);
        cfg.tls_enabled = true;
        assert!(transport_settings(&cfg).is_ok());
        cfg.tls_enabled = false;
        cfg.bind = "127.0.0.1:50051".into();
        assert!(transport_settings(&cfg).is_ok());
    }

    // ---------------------------------------------------------------
    // Loadback of the shipped example config (Spec §3 ↔ parser ↔
    // packaging). The example historically declared `participant:` /
    // `routes:` / `logging:` / `observability:` — none of which this
    // parser reads, so the declared routes never reached the daemon.
    // ---------------------------------------------------------------

    /// The shipped example YAML, embedded against the real file.
    const EXAMPLE_YAML: &str =
        include_str!("../../../../packaging/linux/configs/grpc-bridged.yaml.example");

    /// Top-level (column-0) mapping keys of a YAML document.
    fn top_level_keys(yaml: &str) -> Vec<String> {
        yaml.lines()
            .filter(|l| !l.starts_with([' ', '\t', '#', '-']) && !l.trim().is_empty())
            .filter_map(|l| l.split_once(':').map(|(k, _)| k.trim().to_string()))
            .collect()
    }

    #[test]
    fn grpcbridge_3_example_yaml_parses_without_error() {
        let res = parse_yaml_subset(EXAMPLE_YAML);
        assert!(
            res.is_ok(),
            "grpc-bridged.yaml.example does not parse: {:?}",
            res.err()
        );
    }

    #[test]
    fn grpcbridge_3_example_yaml_key_fields_match_intent() {
        let cfg = parse_yaml_subset(EXAMPLE_YAML).expect("example yaml must parse");
        assert_eq!(cfg.domain, 0);
        assert_eq!(cfg.log_level, "info");
        // §4.1: cleartext h2c only on a loopback bind, so the example
        // without TLS starts.
        assert_eq!(cfg.bind, "127.0.0.1:50051");
        assert!(!cfg.tls_enabled);
        assert!(transport_settings(&cfg).is_ok());
        assert_eq!(cfg.tls_cert_file, "/etc/zerodds/certs/grpc-bridged.crt");
        assert_eq!(cfg.tls_key_file, "/etc/zerodds/certs/grpc-bridged.key");
        // reflection deviates from the default (false).
        assert!(cfg.reflection_enabled);
        assert_eq!(cfg.auth_mode, "none");
        // metrics — endpoint off, but the address has to arrive (Spec §3).
        assert!(!cfg.metrics_enabled);
        assert_eq!(cfg.metrics_listen, Some(DEFAULT_METRICS_ADDR));
    }

    #[test]
    fn grpcbridge_3_example_yaml_topics_reach_the_daemon() {
        let cfg = parse_yaml_subset(EXAMPLE_YAML).expect("example yaml must parse");
        assert_eq!(cfg.topics.len(), 1, "example shows exactly one demo route");
        assert_eq!(cfg.topics[0].dds_name, "Inventory");
        assert_eq!(cfg.topics[0].grpc_service, "InventoryStream");
        assert_eq!(cfg.topics[0].direction, "out");
    }

    #[test]
    fn grpcbridge_3_example_yaml_has_no_unknown_top_level_keys() {
        for key in top_level_keys(EXAMPLE_YAML) {
            assert!(
                KEYS.iter()
                    .any(|k| k.split('.').next() == Some(key.as_str())),
                "grpc-bridged.yaml.example declares top-level key {key:?}, \
                 which parse_yaml_subset does not read"
            );
        }
    }

    #[test]
    fn grpcbridge_3_unknown_top_level_key_does_not_abort_the_parse() {
        let yaml = "\
participant:
  domain_id: 7
observability:
  metrics_bind: \"127.0.0.1:9999\"
domain: 5
";
        let cfg = parse_yaml_subset(yaml).expect("unknown keys must not abort");
        assert_eq!(cfg.domain, 5);
        assert!(!cfg.metrics_enabled, "observability: is not a known key");
    }

    #[test]
    fn grpcbridge_3_env_substitution_applies_before_the_parse() {
        let yaml = "\
grpc:
  bind: \"${ZERODDS_UNSET_PROBE_a41f:-127.0.0.1:50099}\"
";
        let cfg = parse_yaml_subset(yaml).expect("parse");
        assert_eq!(cfg.bind, "127.0.0.1:50099");
    }

    /// Regression: comments used to be cut at the first `#`. The shared
    /// parser keeps a `#` inside quotes and still removes a trailing comment.
    #[test]
    fn a_hash_inside_quotes_is_part_of_the_value() {
        let cfg = parse_yaml_subset("grpc:\n  bind: \"0.0.0.0:1\"   # listen\n").expect("parse");
        assert_eq!(cfg.bind, "0.0.0.0:1");
    }

    #[test]
    fn what_the_old_parser_ignored_is_refused() {
        for (yaml, needle) in [
            ("grpc:\n  tls:\n    enabled: yes\n", "grpc.tls.enabled"),
            ("grpc:\n  reflecton:\n    enabled: true\n", "grpc.reflecton"),
            ("auth:\n  mode: basic\n", "auth.mode"),
            (
                "topics:\n  - dds_name: T\n    grpc_servce: S\n",
                "topics[0].grpc_servce",
            ),
        ] {
            let err = parse_yaml_subset(yaml).expect_err(yaml);
            assert!(err.contains(needle), "{yaml}: {err}");
        }
    }

    #[test]
    fn a_topic_shorthand_keeps_the_idl_scope() {
        let cfg = parse_yaml_subset(
            "topics:\n  - \"Chat::Message=chat.Chat\"\n  - \"Chat::Message:chat.Other\"\n",
        )
        .expect("parse");
        assert_eq!(cfg.topics[0].dds_name, "Chat::Message");
        assert_eq!(cfg.topics[0].grpc_service, "Chat");
        assert_eq!(cfg.topics[0].grpc_package, "chat");
        assert_eq!(cfg.topics[1].dds_name, "Chat::Message");
        assert_eq!(cfg.topics[1].grpc_service, "Other");
        assert_eq!(cfg.topics[1].grpc_package, "chat");
    }

    /// Spec §5.1: an entry without `grpc_service` and `grpc_package` takes
    /// the defaults of its topic name, in the map form and as `--topic DDS`.
    #[test]
    fn grpcbridge_5_1_an_entry_without_service_takes_the_default() {
        let cfg = parse_yaml_subset(
            "topics:\n  - dds_name: \"Chat::Message\"\n  - \"Sensor::Reading\"\n",
        )
        .expect("parse");
        assert_eq!(cfg.topics[0].grpc_service, "ChatMessageStream");
        assert_eq!(cfg.topics[0].grpc_package, "zerodds.chat.v1");
        assert_eq!(cfg.topics[1].grpc_service, "SensorReadingStream");
        assert_eq!(cfg.topics[1].grpc_package, "zerodds.sensor.v1");
        assert_eq!(
            topic_index(&cfg.topics, "zerodds.chat.v1.ChatMessageStream"),
            Some(0)
        );
        assert_eq!(topic_index(&cfg.topics, "ChatMessageStream"), Some(0));
        // The package is part of the name: another qualifier is another
        // service.
        assert_eq!(
            topic_index(&cfg.topics, "zerodds.bridge.v1.ChatMessageStream"),
            None
        );
    }

    /// Spec §5.1: `grpc_service` and `grpc_package` override the defaults;
    /// a dotted `grpc_service` is `<package>.<Service>`.
    #[test]
    fn grpcbridge_5_1_the_fields_override_the_default() {
        let cfg = parse_yaml_subset(
            "topics:\n  - dds_name: \"Chat::Message\"\n    grpc_service: Talk\n  - dds_name: \"Chat::Other\"\n    grpc_package: acme.chat.v2\n  - dds_name: \"Chat::Third\"\n    grpc_service: acme.v3.ThirdStream\n",
        )
        .expect("parse");
        assert_eq!(
            (
                cfg.topics[0].grpc_package.as_str(),
                cfg.topics[0].grpc_service.as_str()
            ),
            ("zerodds.chat.v1", "Talk")
        );
        assert_eq!(
            (
                cfg.topics[1].grpc_package.as_str(),
                cfg.topics[1].grpc_service.as_str()
            ),
            ("acme.chat.v2", "ChatOtherStream")
        );
        assert_eq!(
            (
                cfg.topics[2].grpc_package.as_str(),
                cfg.topics[2].grpc_service.as_str()
            ),
            ("acme.v3", "ThirdStream")
        );
        let both = parse_yaml_subset(
            "topics:\n  - dds_name: X\n    grpc_service: a.B\n    grpc_package: c\n",
        );
        assert!(both.is_err(), "{both:?}");
        let not_an_identifier =
            parse_yaml_subset("topics:\n  - dds_name: X\n    grpc_service: \"B-C\"\n");
        assert!(not_an_identifier.is_err(), "{not_an_identifier:?}");
    }

    /// Spec §5.1: two mappings of one participant that name the same
    /// fully-qualified service stop the start.
    #[test]
    fn grpcbridge_5_1_two_mappings_of_one_service_are_refused() {
        let e = parse_yaml_subset(
            "topics:\n  - dds_name: \"Chat::Message\"\n  - dds_name: \"Chat::Other\"\n    grpc_service: ChatMessageStream\n",
        )
        .expect_err("refused");
        assert!(e.contains("zerodds.chat.v1.ChatMessageStream"), "{e}");
        // The same service name in two packages is two services.
        parse_yaml_subset(
            "topics:\n  - dds_name: \"Chat::Message\"\n  - dds_name: \"Chat::Message\"\n    grpc_package: other.v1\n",
        )
        .expect("two packages");
    }

    /// The YAML example of spec §3, cut out of the specification.
    /// The `idl_file` of the example is read at the start (§5.2): it is
    /// pointed at a file of the test with the same name.
    fn spec_example() -> String {
        let spec = include_str!("../../../../docs/specs/zerodds-grpc-bridge-1.0.md");
        let section = &spec[spec.find("## §3 Config file format").expect("§3")..];
        let start = section.find("```yaml\n").expect("the example") + "```yaml\n".len();
        let text = &section[start..start + section[start..].find("```").expect("its end")];
        let idl =
            std::env::temp_dir().join(format!("zd-grpc-spec-{}-chat.idl", std::process::id()));
        std::fs::write(&idl, SPEC_EXAMPLE_IDL).expect("the IDL of the example");
        text.replace(
            "/etc/zerodds/idl/chat.idl",
            idl.to_str().expect("a UTF-8 path"),
        )
    }

    /// The IDL the example's `idl_file` stands for.
    const SPEC_EXAMPLE_IDL: &str = "module Chat { struct Message { string text; }; };\n";

    /// Spec §3: the example of the specification loads (it used to stop at
    /// `grpc.max_concurrent_streams`, the first key the reader refused).
    #[test]
    fn grpcbridge_3_the_example_of_the_spec_loads() {
        let cfg = parse_yaml_subset(&spec_example()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(cfg.topics.len(), 2);
        assert_eq!(cfg.tenants.len(), 1);
    }

    /// Spec §3: every key of the example reaches the configuration with
    /// the value the example names.
    #[test]
    fn grpcbridge_3_every_key_of_the_spec_example_reaches_the_config() {
        let cfg = parse_yaml_subset(&spec_example()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(cfg.domain, 0);
        assert_eq!(cfg.log_level, "info");
        assert_eq!(cfg.bind, "0.0.0.0:50051");
        assert_eq!(
            cfg.limits,
            ServerLimits {
                max_concurrent_streams: 1024,
                max_recv_message_size: 4_194_304,
                max_send_message_size: 4_194_304,
                initial_window_size: 1_048_576,
                max_frame_size: 16_384,
                permit_pings_without_calls: false,
            }
        );
        assert_eq!(cfg.keepalive_time_secs, 60);
        assert_eq!(cfg.keepalive_timeout_secs, 10);
        assert_eq!(cfg.drain_secs, 30);
        assert!(cfg.tls_enabled);
        assert_eq!(cfg.tls_cert_file, "/etc/zerodds/grpc-cert.pem");
        assert_eq!(cfg.tls_key_file, "/etc/zerodds/grpc-key.pem");
        assert_eq!(cfg.tls_client_ca_file, "/etc/zerodds/grpc-client-ca.pem");
        assert!(!cfg.tls_require_client_cert);
        assert_eq!(cfg.tls_alpn, ["h2", "http/1.1"]);
        assert_eq!(cfg.tls_options.min_version, TlsMinVersion::Tls12);
        assert_eq!(
            cfg.tls_options.cipher_suites,
            ["TLS_AES_128_GCM_SHA256", "TLS_AES_256_GCM_SHA384"]
        );
        assert!(cfg.reflection_enabled);
        assert_eq!(cfg.auth_mode, "jwt");
        assert_eq!(cfg.auth_jwt.public_key, "/etc/zerodds/jwt-pub.pem");
        assert_eq!(
            cfg.auth_jwt.issuer.as_deref(),
            Some("https://idp.example.org")
        );
        assert_eq!(cfg.auth_jwt.audience.as_deref(), Some("zerodds-grpc"));
        assert_eq!(
            cfg.auth_jwt.required_claim.as_deref(),
            Some("scope=dds.read")
        );
        assert!(cfg.metrics_enabled);
        assert_eq!(cfg.metrics_listen, Some(DEFAULT_METRICS_ADDR));
        assert_eq!(cfg.metrics_path, "/metrics");
        let chat = &cfg.topics[0];
        assert_eq!(chat.dds_name, "Chat::Message");
        assert_eq!(chat.dds_type, "Chat::Message");
        assert_eq!(chat.grpc_service, "ChatMessageStream");
        assert_eq!(chat.grpc_package, "zerodds.chat.v1");
        assert_eq!(chat.direction, "bidir");
        assert!(chat.reliable);
        assert_eq!(chat.durability, zerodds_qos::DurabilityKind::Volatile);
        assert_eq!(chat.history_depth, Some(10));
        assert_eq!(chat.deadline_ms, Some(1000));
        assert_eq!(chat.lifespan_ms, Some(60_000));
        assert_eq!(chat.idl_source.as_deref(), Some(SPEC_EXAMPLE_IDL));
        assert_eq!(
            chat.acl,
            Some((
                vec!["alice".to_string(), "bob".to_string()],
                vec!["*".to_string()]
            ))
        );
        let sensor = &cfg.topics[1];
        assert_eq!(sensor.direction, "out");
        assert!(!sensor.reliable);
        assert_eq!(sensor.grpc_package, "zerodds.sensor.v1");
        let plant = &cfg.tenants[0];
        assert_eq!(plant.name, "plant-a");
        assert_eq!(plant.domain, 7);
        assert_eq!(plant.subjects, vec!["CN=plant-a,O=Example".to_string()]);
        assert_eq!(plant.topics[0].dds_name, "Plant::Status");
        assert_eq!(plant.topics[0].grpc_service, "PlantStatusStream");
    }

    /// Spec §3, §9.2: `grpc.drain_secs` is 30 by default and takes 0 (the
    /// open calls end UNAVAILABLE at once) to 300.
    #[test]
    fn grpcbridge_9_2_drain_secs_is_read() {
        assert_eq!(
            parse_yaml_subset("domain: 0\n")
                .expect("default")
                .drain_secs,
            30
        );
        for v in [0u64, 7, 300] {
            let cfg =
                parse_yaml_subset(&format!("grpc:\n  drain_secs: {v}\n")).expect("drain_secs");
            assert_eq!(cfg.drain_secs, v);
        }
    }

    /// Spec §3: a value outside the range the keys allow (RFC 9113
    /// §6.5.2 for the two settings) stops the start with the key.
    #[test]
    fn grpcbridge_3_a_value_outside_its_range_is_refused() {
        for (yaml, needle) in [
            ("grpc:\n  drain_secs: 301\n", "grpc.drain_secs"),
            ("grpc:\n  drain_secs: -1\n", "grpc.drain_secs"),
            ("grpc:\n  max_frame_size: 1000\n", "grpc.max_frame_size"),
            ("grpc:\n  max_frame_size: 16777216\n", "grpc.max_frame_size"),
            (
                "grpc:\n  initial_window_size: 2147483648\n",
                "grpc.initial_window_size",
            ),
            (
                "grpc:\n  max_concurrent_streams: 0\n",
                "grpc.max_concurrent_streams",
            ),
            (
                "grpc:\n  max_recv_message_size: 0\n",
                "grpc.max_recv_message_size",
            ),
            (
                "grpc:\n  max_send_message_size: -1\n",
                "grpc.max_send_message_size",
            ),
            (
                "grpc:\n  keepalive:\n    time_secs: 0\n",
                "grpc.keepalive.time_secs",
            ),
            (
                "grpc:\n  keepalive:\n    timeout_secs: 0\n",
                "grpc.keepalive.timeout_secs",
            ),
            (
                "grpc:\n  keepalive:\n    interval: 3\n",
                "grpc.keepalive.interval",
            ),
            ("grpc:\n  tls:\n    alpn: [\"http/1.1\"]\n", "grpc.tls.alpn"),
            ("grpc:\n  tls:\n    alpn: []\n", "grpc.tls.alpn"),
            (
                "grpc:\n  tls:\n    alpn: [\"h2\", \"h2\"]\n",
                "grpc.tls.alpn",
            ),
            (
                "grpc:\n  tls:\n    alpn: [\"h2\", \"spdy/3\"]\n",
                "grpc.tls.alpn",
            ),
            (
                "grpc:\n  tls:\n    cipher_suites: []\n",
                "grpc.tls.cipher_suites",
            ),
            ("metrics:\n  path: metrics\n", "metrics.path"),
            ("metrics:\n  path: /healthz\n", "metrics.path"),
            // Spec §8.2: an IP address and port, no host name.
            (
                "metrics:\n  listen: \"localhost:9095\"\n",
                "`localhost:9095` is no IP address",
            ),
            ("metrics:\n  listen: \"127.0.0.1\"\n", "metrics.listen"),
        ] {
            let err = parse_yaml_subset(yaml).expect_err(yaml);
            assert!(err.contains(needle), "{yaml}: {err}");
        }
        let cfg = parse_yaml_subset(
            "grpc:\n  max_frame_size: 16777215\n  initial_window_size: 2147483647\n  keepalive:\n    permit_without_calls: true\nmetrics:\n  path: /prom\n",
        )
        .expect("the bounds themselves");
        assert_eq!(cfg.limits.max_frame_size, 16_777_215);
        assert_eq!(cfg.limits.initial_window_size, 0x7fff_ffff);
        // Spec §3: `h2` alone, or `h2` and `http/1.1` in either order.
        for (yaml, alpn) in [
            ("grpc:\n  tls:\n    alpn: [\"h2\"]\n", &["h2"][..]),
            (
                "grpc:\n  tls:\n    alpn: [\"http/1.1\", \"h2\"]\n",
                &["http/1.1", "h2"][..],
            ),
        ] {
            assert_eq!(parse_yaml_subset(yaml).expect(yaml).tls_alpn, alpn);
        }
        assert!(cfg.limits.permit_pings_without_calls);
        assert_eq!(cfg.metrics_path, "/prom");
        let cfg = parse_yaml_subset("metrics:\n  enabled: true\n  listen: \"[::1]:9195\"\n")
            .expect("an IPv6 address");
        assert_eq!(
            metrics_addr_for(&cfg),
            Some("[::1]:9195".parse().expect("an address"))
        );
    }

    /// Spec §1 L6, §3: the tenants and what their entries refuse.
    #[test]
    fn grpcbridge_1_l6_the_tenants() {
        let yaml = "domain: 3\ngrpc:\n  tls:\n    enabled: true\n    client_ca_file: ca.pem\ntenants:\n  - name: plant-a\n    domain: 9\n    subjects: [\"CN=a\"]\n    topics:\n      - \"A::B=AStream\"\n  - name: plant-b\n    subjects: [\"mtls:00ff\"]\n";
        let cfg = parse_yaml_subset(yaml).expect("parse");
        assert_eq!(cfg.tenants.len(), 2);
        assert_eq!(cfg.tenants[0].domain, 9);
        assert_eq!(cfg.tenants[0].topics[0].grpc_service, "AStream");
        assert_eq!(cfg.tenants[1].domain, 3, "the top-level domain");
        for (yaml, needle) in [
            ("tenants:\n  - subjects: [x]\n", "tenants[0].name"),
            ("tenants:\n  - name: a\n", "tenants[0].subjects"),
            (
                "tenants:\n  - name: a/b\n    subjects: [x]\n",
                "tenants[0].name",
            ),
            (
                "tenants:\n  - name: a\n    subjects: [x]\n  - name: a\n    subjects: [y]\n",
                "tenants[1].name",
            ),
            (
                "tenants:\n  - name: a\n    subjects: [x]\n    bind: z\n",
                "tenants[0].bind",
            ),
            (
                "tenants:\n  - name: a\n    subjects: [x]\n    topics:\n      - dds_name: T\n        grpc_service: \"x-y\"\n",
                "tenants[0].topics[0]",
            ),
        ] {
            let err = parse_yaml_subset(yaml).expect_err(yaml);
            assert!(err.contains(needle), "{yaml}: {err}");
        }
    }

    /// Spec §1 L6: tenants without TLS or without a client CA are refused,
    /// since no connection would carry a client certificate to bind.
    #[test]
    fn grpcbridge_1_l6_tenants_need_tls_with_a_client_ca() {
        let tenant = "tenants:\n  - name: t\n    subjects: [\"CN=t\"]\n";
        for tls in [
            "",
            "grpc:\n  tls:\n    enabled: true\n",
            "grpc:\n  tls:\n    client_ca_file: ca.pem\n",
            "grpc:\n  tls:\n    enabled: false\n    client_ca_file: ca.pem\n",
        ] {
            let yaml = format!("{tls}{tenant}");
            let err = parse_yaml_subset(&yaml).expect_err(&yaml);
            assert!(err.contains("tenants"), "{yaml}: {err}");
            assert!(err.contains("grpc.tls.client_ca_file"), "{yaml}: {err}");
        }
        let yaml =
            format!("grpc:\n  tls:\n    enabled: true\n    client_ca_file: ca.pem\n{tenant}");
        assert_eq!(
            parse_yaml_subset(&yaml)
                .expect("TLS with a client CA")
                .tenants
                .len(),
            1
        );
        // Without tenants TLS stays optional.
        assert!(parse_yaml_subset("topics: []\n").is_ok());
    }

    /// Spec §1 L6: a subject is bound to one tenant at most; a connection
    /// goes to the tenant naming its certificate's subject DN or its
    /// fingerprint, else to the participant of `domain`.
    #[test]
    fn grpcbridge_1_l6_a_subject_is_bound_to_one_tenant_at_most() {
        let err = parse_yaml_subset(
            "grpc:\n  tls:\n    enabled: true\n    client_ca_file: ca.pem\ntenants:\n  - name: a\n    subjects: [\"CN=x\"]\n  - name: b\n    subjects: [\"CN=y\", \"CN=x\"]\n",
        )
        .expect_err("a subject twice");
        assert!(err.contains("tenants[1].subjects"), "{err}");
        let participant = |tenant: Option<&str>, subjects: &[&str]| Participant {
            tenant: tenant.map(str::to_string),
            subjects: subjects.iter().map(|s| s.to_string()).collect(),
            topics: Vec::new(),
            dds: None,
        };
        let parts = [
            participant(None, &[]),
            participant(Some("a"), &["CN=x"]),
            participant(Some("b"), &["mtls:abcd"]),
        ];
        let subject = |name: &str, fingerprint: &str| {
            let mut s = AuthSubject::new(name.to_string());
            s.fingerprint = Some(fingerprint.to_string());
            s
        };
        let of = |s: Option<&AuthSubject>| participant_of(&parts, s).tenant.clone();
        assert_eq!(of(Some(&subject("CN=x", "mtls:1"))), Some("a".to_string()));
        assert_eq!(
            of(Some(&subject("CN=z", "mtls:abcd"))),
            Some("b".to_string())
        );
        assert_eq!(of(Some(&subject("CN=z", "mtls:1"))), None);
        assert_eq!(of(None), None);
        assert_eq!(parts[1].label("T::M"), "a/T::M");
        assert_eq!(parts[0].label("T::M"), "T::M");
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod metrics_default_tests {
    use super::{DEFAULT_METRICS_ADDR, DEFAULT_METRICS_LISTEN};

    /// The default address and its text are one.
    #[test]
    fn grpcbridge_8_2_the_default_address_is_its_text() {
        assert_eq!(DEFAULT_METRICS_LISTEN.parse(), Ok(DEFAULT_METRICS_ADDR));
    }

    /// Each bridge daemon has its own admin port, so that several with
    /// `metrics.enabled: true` and no `metrics.listen` run on one host;
    /// man5 names it as the default.
    #[test]
    fn the_metrics_default_is_the_one_man5_names() {
        let page = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../man/man5/zerodds-grpc-bridged.yaml.5"
        ))
        .unwrap_or_else(|e| panic!("man5 page: {e}"));
        let named = format!("Default\n.IR \"{DEFAULT_METRICS_LISTEN}\"\nwhen");
        assert!(
            page.contains(&named),
            "man5 does not name {DEFAULT_METRICS_LISTEN}"
        );
    }
}
