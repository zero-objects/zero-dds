// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Config for `zerodds-xrce-agentd`.
//!
//! Read by the shared layered loader (`zerodds-config`): base file,
//! `xrce-agentd.d/` drop-ins, includes, `${VAR}`, and the precedence
//! defaults < environment < files < `--set`, exactly like
//! `crates/coap-bridge`'s daemon. The DDS runtime's own keys may sit in
//! the same file; [`DaemonConfig::from_config`] reads this daemon's keys
//! and refuses an unknown key with its file and line.

use std::path::PathBuf;
use std::string::{String, ToString};
use std::vec::Vec;

pub use zerodds_config::ConfigError;
use zerodds_config::read::Section;

/// The base name of the daemon's file: `xrce-agentd.yaml`, `xrce-agentd.d/`.
pub const CONFIG_STEM: &str = "xrce-agentd";

/// Every key this daemon reads; list-valued keys stop at the list.
pub const KEYS: &[&str] = &[
    "log_level",
    "vendor_id",
    "session_timeout_secs",
    "read_without_delivery_control",
    "unknown_topic_types",
    "same_session_create_client",
    "update_of_missing_object",
    "timestamp_interval_secs",
    "unannotated_types",
    "representation_timestamps",
    "representation_formats",
    "unsupported_qos",
    "status_agent",
    "delete_client_reply",
    "no_client_threshold",
    "max_elapsed_time_unit",
    "sample_info",
    "vendor_forms",
    "limits.max_clients",
    "limits.max_objects_per_client",
    "limits.max_pending_samples",
    "discovery.enabled",
    "discovery.port",
    "discovery.address",
    "authorized_client_keys",
    "required_client_properties",
    "access_rules",
    "client_applications",
    "listeners",
    "type_files",
    "xml_profile_files",
    "qos_library_files",
];

/// Whether `path` is one of [`KEYS`] (or a `listeners[]`/`type_files[]`
/// entry's own sub-keys, checked separately by [`read_listener`]).
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

/// One `listeners[]` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListenerConfig {
    /// `kind: udp` — spec item 2, default port 2018 (eProsima's own
    /// default), IPv4 or IPv6 depending on `bind`.
    Udp {
        /// `host:port` to bind.
        bind: String,
    },
    /// `kind: tcp` — spec §11.3 2-byte length-prefix framing.
    Tcp {
        /// `host:port` to bind.
        bind: String,
    },
    /// `kind: dtls` — the UDP transport inside DTLS 1.2 (DDS-XRCE 1.0
    /// §11.2): the agent authenticates itself to the client with its
    /// certificate, and the messages, the `ClientKey` among them, travel
    /// encrypted (§8.3.2.4). Needs the `dtls` feature.
    Dtls {
        /// `host:port` to bind.
        bind: String,
        /// PEM file with the agent's certificate chain.
        cert_file: String,
        /// PEM file with the agent's private key (PKCS#8).
        key_file: String,
        /// PEM file with the trust anchors a client certificate must chain
        /// to; unset asks for no client certificate.
        client_ca_file: Option<String>,
    },
    /// `kind: tls` — the TCP transport inside TLS (§11.3), with the
    /// 2-byte length prefix inside the encrypted stream. Needs the `tls`
    /// feature.
    Tls {
        /// `host:port` to bind.
        bind: String,
        /// PEM file with the agent's certificate chain.
        cert_file: String,
        /// PEM file with the agent's private key.
        key_file: String,
        /// PEM file with the trust anchors a client certificate must chain
        /// to; unset asks for no client certificate.
        client_ca_file: Option<String>,
    },
    /// `kind: serial` — Annex C framing over a real serial device
    /// (feature `serial`).
    Serial {
        /// Device path (e.g. `/dev/ttyUSB0`).
        device: String,
        /// Baud rate.
        baud_rate: u32,
        /// The agent's address on the line, the `sourceAddress` of its
        /// frames (Annex C.1.1.2); 0 by default, the address the
        /// Micro-XRCE-DDS clients send to.
        address: u8,
        /// How `frameCheck` is computed: Annex C (`rfc1662`, the default)
        /// or as Micro-XRCE-DDS does (`crc16-arc-payload`).
        frame_check: zerodds_xrce::transport_serial::FrameCheck,
    },
}

/// A `type_files[]` entry: a precompiled `TypeObject` (XCDR2, as
/// `TypeObject::to_bytes_le` produces) named for topic-type resolution.
/// Compiling IDL/DDS-XML type definitions into a `TypeObject` at daemon
/// startup is out of scope here — see the crate README's open items; this
/// path is for a `TypeObject` produced ahead of time by another ZeroDDS
/// tool (`idlc`, `wire-probe`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeFileConfig {
    /// The DDS type name this `TypeObject` is registered under.
    pub type_name: String,
    /// Path to the binary `TypeObject`.
    pub file: PathBuf,
}

/// Daemon config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonConfig {
    /// `log_level:`.
    pub log_level: String,
    /// `vendor_id:` — this agent's `XrceVendorId`, `major.minor` as two
    /// decimal octets (e.g. `"1.15"` for eProsima's own registered id);
    /// `"0.0"` (default) for none.
    pub vendor_id: [u8; 2],
    /// `session_timeout_secs:` — spec §8.4.9-adjacent liveliness.
    pub session_timeout_secs: u64,
    /// `limits.max_clients:`.
    pub max_clients: usize,
    /// `limits.max_objects_per_client:`.
    pub max_objects_per_client: usize,
    /// `limits.max_pending_samples:` — maps onto
    /// `XrceAgent::with_max_pending_samples`.
    pub max_pending_samples: usize,
    /// `discovery.enabled:` — answer `GET_INFO` on the multicast
    /// discovery group (§11.2.4).
    pub discovery_enabled: bool,
    /// `discovery.port:` — default 7400 (spec's `XRCE_DISCOVERY_PORT`).
    pub discovery_port: u16,
    /// `discovery.address:` — the multicast group the agent joins for
    /// discovery (DDS-XRCE 1.0 §11.2.4); default 239.255.0.2.
    pub discovery_address: std::net::Ipv4Addr,
    /// `authorized_client_keys:`, `required_client_properties:` and
    /// `access_rules:` — the AccessController of DDS-XRCE 1.0 §7.4. Without
    /// any of them the agent restricts nothing.
    pub access: crate::daemon::access::AccessController,
    /// `client_applications:` — entries `client: 0xAABBCCDD` and
    /// `application: Library::Application` (or the reference of a
    /// DomainParticipant): what the agent makes for that client when its
    /// `CREATE_CLIENT` arrives (DDS-XRCE 1.0 §8.4.2, §8.4.3). The definitions
    /// come from `xml_profile_files:`.
    pub client_applications: Vec<([u8; 4], String)>,
    /// `listeners:`.
    pub listeners: Vec<ListenerConfig>,
    /// `type_files:`.
    pub type_files: Vec<TypeFileConfig>,
    /// `xml_profile_files:` — §9.3 file configuration (participant/topic/
    /// pub/sub/writer/reader hierarchy + inline types), loaded at startup
    /// so `REPRESENTATION_BY_REFERENCE` CREATEs can resolve a profile
    /// name.
    pub xml_profile_files: Vec<PathBuf>,
    /// `qos_library_files:` — DDS-XML `<qos_library>` files, resolved
    /// against `xml_profile_files`' `qos_profile="Lib::Profile"`
    /// references.
    pub qos_library_files: Vec<PathBuf>,
    /// `read_without_delivery_control:` — `once` (default, DDS-XRCE 1.0
    /// §8.3.5.9.2: one read call, the samples already in the cache) or
    /// `next-sample` (the next sample there is, as Micro-XRCE-DDS 3.0.1
    /// does).
    pub read_without_delivery_control: crate::daemon::session::ReadWithoutDeliveryControl,
    /// `unknown_topic_types:` — `refuse` (default, DDS-XRCE 1.0 §7.7.3.7.3:
    /// "The referenced type shall be known to the XRCE Agent":
    /// `STATUS_ERR_UNKNOWN_REFERENCE`) or `forward` (a topic whose type the
    /// agent does not know is created and its samples are forwarded opaque,
    /// as the eProsima client expects for the types it names).
    pub forward_unknown_topic_types: bool,
    /// `same_session_create_client:` — `keep` (default, DDS-XRCE 1.0
    /// §7.8.2.1: a `CREATE_CLIENT` with the `session_id` of the client's
    /// session "shall not perform any action") or `restart` (the streams
    /// of the session start again, for a client process that restarts with
    /// the same `ClientKey` and `session_id`).
    pub restart_on_same_session: bool,
    /// `update_of_missing_object:` — `create` (default, DDS-XRCE 1.0 §7.8.3.1
    /// Table 5: a CREATE with `reuse` and `replace` of an object that does not
    /// exist creates it) or `refuse` (§7.8.3.2: the update of an object that
    /// does not exist is `STATUS_ERR_UNKNOWN_REFERENCE`).
    pub refuse_update_of_missing_object: bool,
    /// `timestamp_interval_secs:` — how often the agent sends a `TIMESTAMP` to
    /// each client (DDS-XRCE 1.0 §8.3.5.15); `0` (default) sends none.
    pub timestamp_interval_secs: u64,
    /// `unannotated_types:` — `appendable` (default, DDS-XRCE 1.0 Annex A with
    /// DDS-XTypes 1.3 §7.3.1.2.1.8: the types without `@extensibility`
    /// (`Time_t`, `ResultStatus`, `Property`, `TransportLocator*`,
    /// `ACKNACK_Payload`) are appendable and have a DHEADER) or `final` (the
    /// form of the eProsima Micro-XRCE-DDS reference implementation, which
    /// writes none: a `STATUS` payload of 6 octets, an `ACKNACK` of 5). The
    /// agent reads either; this sets what it writes. Needed for clients that
    /// read only the `final` form.
    pub unannotated_types: zerodds_xrce::encoding::UnannotatedTypes,
    /// `vendor_forms:` — `refuse` (default) or `accept`. The DDS-XML readers
    /// of the agent follow the schemas of DDS-XML 1.0; a form other DDS
    /// products write instead (`<domainparticipant_qos>`, the
    /// `qos_profile_ref` attribute, `baseType`, the IDL names `long` and
    /// `octet` as a type, and others) is refused with an error that names it
    /// and the form of the specification. `accept` reads them (the profile
    /// files of eProsima need it) and the agent lists each one at start.
    pub vendor_forms: bool,
    /// `representation_timestamps:` — `absent` (default: the form of the IDL
    /// of Annex A and the bytes of Annex B.1, which have no
    /// `client_timestamp` / `agent_timestamp`) or `present` (the form of the
    /// prose of DDS-XRCE 1.0 §7.8.2.1 and §8.3.5.1.2: a `Time_t` after
    /// `xrce_vendor_id`). The specification contradicts itself and the two
    /// forms cannot be told apart by their bytes, so the agent reads and
    /// writes the form this names.
    pub representation_timestamps: zerodds_xrce::encoding::RepresentationTimestamps,
    /// `representation_formats:` — `spec` (default: each object kind has the
    /// representation formats DDS-XRCE 1.0 §7.7.3.1 and Annex A give it; a
    /// CREATE in another is `STATUS_ERR_INVALID_DATA`) or `any` (every format
    /// for every kind). The eProsima Micro XRCE-DDS client creates its
    /// Publishers, Subscribers, DataWriters and DataReaders by reference,
    /// which the specification does not have for them; it needs `any`.
    pub allow_any_representation_format: bool,
    /// `sample_info:` — `annex_a` (default: the `SampleInfo` of Annex A, a
    /// `state` and a `SampleInfoDetail` union switched on a discriminator;
    /// written as `FORMAT_SEQN_TIMS`, which is 12 octets like the flat
    /// struct, and read by its discriminator) or `flat` (the struct of DDS-XRCE
    /// 1.0 §7.7.1: always 12 octets, the octet after `state` is padding). A
    /// client that sends the flat form with zero padding needs `flat`: the
    /// zero reads as `FORMAT_EMPTY` otherwise.
    pub sample_info_form: zerodds_xrce::encoding::SampleInfoForm,
    /// `unsupported_qos:` — `refuse` (default: a CREATE whose QoS the agent
    /// cannot apply is `STATUS_ERR_INCOMPATIBLE`) or `ignore` (the object is
    /// created without that part, and the agent reports what it left out;
    /// for the profile files of other vendors that carry elements of their
    /// own).
    pub ignore_unsupported_qos: bool,
    /// `max_elapsed_time_unit:` — `seconds` (default, DDS-XRCE 1.0 Table 9) or
    /// `milliseconds` (the reading of the example of Annex B.4.2, which writes
    /// 30000 for 30 seconds).
    pub max_elapsed_time_in_milliseconds: bool,
    /// `status_agent:` — `annex_a` (default: the payload of `STATUS_AGENT` is
    /// the `agent_info` of the IDL of Annex A, a failure carries the
    /// `ResultStatus`) or `with_result_status` (always a `ResultStatus` before
    /// the `AGENT_Representation`, as the text of §8.3.5.5.2 and the eProsima
    /// client have it).
    pub status_agent_with_result_status: bool,
    /// `delete_client_reply:` — `status` (default, §8.3.5.4: `DELETE` is
    /// answered with `STATUS`) or `status_agent` (Table 14 of DDS-XRCE 1.0
    /// names `STATUS_AGENT` for `delete_client`).
    pub delete_client_reply_is_status_agent: bool,
    /// `no_client_threshold:` — after this many messages of one connection
    /// to a ClientKey without a ProxyClient the agent closes the connection
    /// (DDS-XRCE 1.0 §7.8.1); such a message is dropped without an answer.
    /// Default 16; 0 never closes.
    pub no_client_threshold: u32,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            vendor_id: [0, 0],
            session_timeout_secs: 60,
            max_clients: 1024,
            max_objects_per_client: 256,
            max_pending_samples: 256,
            discovery_enabled: true,
            discovery_port: 7400,
            discovery_address: zerodds_xrce::discovery::XRCE_DISCOVERY_GROUP,
            access: crate::daemon::access::AccessController::default(),
            client_applications: Vec::new(),
            listeners: vec![ListenerConfig::Udp {
                bind: "0.0.0.0:2018".to_string(),
            }],
            type_files: Vec::new(),
            xml_profile_files: Vec::new(),
            qos_library_files: Vec::new(),
            read_without_delivery_control: crate::daemon::session::ReadWithoutDeliveryControl::Once,
            forward_unknown_topic_types: false,
            restart_on_same_session: false,
            refuse_update_of_missing_object: false,
            timestamp_interval_secs: 0,
            unannotated_types: zerodds_xrce::encoding::UnannotatedTypes::Appendable,
            vendor_forms: false,
            representation_timestamps: zerodds_xrce::encoding::RepresentationTimestamps::Absent,
            allow_any_representation_format: false,
            sample_info_form: zerodds_xrce::encoding::SampleInfoForm::AnnexA,
            ignore_unsupported_qos: false,
            max_elapsed_time_in_milliseconds: false,
            status_agent_with_result_status: false,
            delete_client_reply_is_status_agent: false,
            no_client_threshold: 16,
        }
    }
}

impl DaemonConfig {
    /// Parses from a string, for tests (mirrors
    /// `crates/coap-bridge`'s `DaemonConfig::load_from_str`).
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

    /// Parses the daemon's keys from a layered [`zerodds_config::Config`].
    ///
    /// # Errors
    /// [`ConfigError`] — an unknown key, or a value of the wrong shape.
    pub fn from_config(config: Option<&zerodds_config::Config>) -> Result<Self, ConfigError> {
        let root = Section::root(config.and_then(zerodds_config::Config::root))?;
        root.only(&[
            "log_level",
            "vendor_id",
            "session_timeout_secs",
            "read_without_delivery_control",
            "unknown_topic_types",
            "same_session_create_client",
            "update_of_missing_object",
            "timestamp_interval_secs",
            "unannotated_types",
            "representation_timestamps",
            "representation_formats",
            "unsupported_qos",
            "status_agent",
            "delete_client_reply",
            "no_client_threshold",
            "max_elapsed_time_unit",
            "sample_info",
            "vendor_forms",
            "limits",
            "discovery",
            "authorized_client_keys",
            "required_client_properties",
            "access_rules",
            "client_applications",
            "listeners",
            "type_files",
            "xml_profile_files",
            "qos_library_files",
        ])?;
        let mut out = Self::default();
        if let Some(v) = root.choice("log_level", &["trace", "debug", "info", "warn", "error"])? {
            out.log_level = v;
        }
        if let Some((text, origin)) = root.scalar("vendor_id")? {
            out.vendor_id = parse_vendor_id(text).ok_or_else(|| {
                invalid(
                    root.path_of("vendor_id"),
                    origin.clone(),
                    "expected \"major.minor\", each 0..=255".to_string(),
                )
            })?;
        }
        if let Some(v) = root.parse::<u64>("session_timeout_secs", "seconds, an integer")? {
            out.session_timeout_secs = v;
        }
        if let Some(v) = root.choice("read_without_delivery_control", &["once", "next-sample"])? {
            out.read_without_delivery_control = if v == "next-sample" {
                crate::daemon::session::ReadWithoutDeliveryControl::NextSample
            } else {
                crate::daemon::session::ReadWithoutDeliveryControl::Once
            };
        }

        if let Some(v) = root.choice("unknown_topic_types", &["forward", "refuse"])? {
            out.forward_unknown_topic_types = v == "forward";
        }
        if let Some(v) = root.choice("same_session_create_client", &["keep", "restart"])? {
            out.restart_on_same_session = v == "restart";
        }
        if let Some(v) = root.choice("update_of_missing_object", &["create", "refuse"])? {
            out.refuse_update_of_missing_object = v == "refuse";
        }
        if let Some(v) = root.parse::<u64>("timestamp_interval_secs", "a number of seconds")? {
            out.timestamp_interval_secs = v;
        }
        if let Some(v) = root.choice("unannotated_types", &["appendable", "final"])? {
            out.unannotated_types = if v == "final" {
                zerodds_xrce::encoding::UnannotatedTypes::Final
            } else {
                zerodds_xrce::encoding::UnannotatedTypes::Appendable
            };
        }

        if let Some(v) = root.choice("vendor_forms", &["refuse", "accept"])? {
            out.vendor_forms = v == "accept";
        }

        if let Some(v) = root.choice("representation_timestamps", &["absent", "present"])? {
            out.representation_timestamps = if v == "present" {
                zerodds_xrce::encoding::RepresentationTimestamps::Present
            } else {
                zerodds_xrce::encoding::RepresentationTimestamps::Absent
            };
        }

        if let Some(v) = root.choice("representation_formats", &["spec", "any"])? {
            out.allow_any_representation_format = v == "any";
        }

        if let Some(v) = root.choice("unsupported_qos", &["refuse", "ignore"])? {
            out.ignore_unsupported_qos = v == "ignore";
        }
        if let Some(v) = root.choice("status_agent", &["annex_a", "with_result_status"])? {
            out.status_agent_with_result_status = v == "with_result_status";
        }
        if let Some(v) = root.parse::<u32>("no_client_threshold", "a count")? {
            out.no_client_threshold = v;
        }
        if let Some(v) = root.choice("delete_client_reply", &["status", "status_agent"])? {
            out.delete_client_reply_is_status_agent = v == "status_agent";
        }
        if let Some(v) = root.choice("max_elapsed_time_unit", &["seconds", "milliseconds"])? {
            out.max_elapsed_time_in_milliseconds = v == "milliseconds";
        }
        if let Some(v) = root.choice("sample_info", &["annex_a", "flat"])? {
            out.sample_info_form = if v == "flat" {
                zerodds_xrce::encoding::SampleInfoForm::Flat
            } else {
                zerodds_xrce::encoding::SampleInfoForm::AnnexA
            };
        }

        let limits = root.section("limits")?;
        limits.only(&[
            "max_clients",
            "max_objects_per_client",
            "max_pending_samples",
        ])?;
        if let Some(v) = limits.parse::<usize>("max_clients", "an integer")? {
            out.max_clients = v;
        }
        if let Some(v) = limits.parse::<usize>("max_objects_per_client", "an integer")? {
            out.max_objects_per_client = v;
        }
        if let Some(v) = limits.parse::<usize>("max_pending_samples", "an integer")? {
            out.max_pending_samples = v;
        }

        let discovery = root.section("discovery")?;
        discovery.only(&["enabled", "port", "address"])?;
        if let Some(v) = discovery.boolean("enabled")? {
            out.discovery_enabled = v;
        }
        if let Some(v) = discovery.parse::<u16>("port", "a port number")? {
            out.discovery_port = v;
        }
        if let Some(v) =
            discovery.parse::<std::net::Ipv4Addr>("address", "an IPv4 multicast address")?
        {
            if !v.is_multicast() {
                return Err(invalid(
                    discovery.path_of("address"),
                    zerodds_config::Origin::Default,
                    format!("{v} is no IPv4 multicast address"),
                ));
            }
            out.discovery_address = v;
        }

        for (path, item) in root.items("authorized_client_keys")? {
            let zerodds_config::Node::Scalar(text, origin) = item else {
                return Err(invalid(
                    path,
                    zerodds_config::read::first_origin(item),
                    "expected a ClientKey, the four octets in hexadecimal (e.g. `0xAABBCCDD`)"
                        .to_string(),
                ));
            };
            let key = crate::daemon::access::parse_client_key(text).ok_or_else(|| {
                invalid(
                    path,
                    origin.clone(),
                    "expected a ClientKey, the four octets in hexadecimal (e.g. `0xAABBCCDD`)"
                        .to_string(),
                )
            })?;
            out.access.authorized_clients.push(key);
        }
        for (path, item) in root.items("required_client_properties")? {
            let entry = Section::of(Some(item), path)?;
            entry.only(&["name", "value"])?;
            let name = entry.string("name")?.ok_or_else(|| {
                invalid(
                    entry.path_of("name"),
                    zerodds_config::Origin::Default,
                    "required_client_properties[] entry needs `name:`".to_string(),
                )
            })?;
            let value = entry.string("value")?.ok_or_else(|| {
                invalid(
                    entry.path_of("value"),
                    zerodds_config::Origin::Default,
                    "required_client_properties[] entry needs `value:`".to_string(),
                )
            })?;
            out.access.required_properties.push((name, value));
        }
        for (path, item) in root.items("access_rules")? {
            out.access.rules.push(read_access_rule(&path, item)?);
        }
        for (path, item) in root.items("client_applications")? {
            let entry = Section::of(Some(item), path)?;
            entry.only(&["client", "application"])?;
            let (text, origin) = entry.scalar("client")?.ok_or_else(|| {
                invalid(
                    entry.path_of("client"),
                    zerodds_config::Origin::Default,
                    "client_applications[] entry needs `client:`".to_string(),
                )
            })?;
            let key = crate::daemon::access::parse_client_key(text).ok_or_else(|| {
                invalid(
                    entry.path_of("client"),
                    origin.clone(),
                    "expected a ClientKey, the four octets in hexadecimal (e.g. `0xAABBCCDD`)"
                        .to_string(),
                )
            })?;
            let application = entry.string("application")?.ok_or_else(|| {
                invalid(
                    entry.path_of("application"),
                    zerodds_config::Origin::Default,
                    "client_applications[] entry needs `application:`".to_string(),
                )
            })?;
            out.client_applications.push((key, application));
        }

        let listener_items = root.items("listeners")?;
        if !listener_items.is_empty() {
            out.listeners.clear();
            for (path, item) in listener_items {
                out.listeners.push(read_listener(&path, item)?);
            }
        }

        for (path, item) in root.items("type_files")? {
            out.type_files.push(read_type_file(&path, item)?);
        }
        if let Some(paths) = root.strings("xml_profile_files")? {
            out.xml_profile_files = paths.into_iter().map(PathBuf::from).collect();
        }
        if let Some(paths) = root.strings("qos_library_files")? {
            out.qos_library_files = paths.into_iter().map(PathBuf::from).collect();
        }
        Ok(out)
    }
}

fn parse_vendor_id(text: &str) -> Option<[u8; 2]> {
    let (major, minor) = text.split_once('.')?;
    Some([major.trim().parse().ok()?, minor.trim().parse().ok()?])
}

/// A `listeners[]` entry written as one string, `udp:<host:port>`,
/// `tcp:<host:port>` or `serial:<device>` (at 115 200 baud) — the form
/// `--listen` and `--set +listeners=…` produce, since a command-line value is
/// a scalar.
fn listener_from_shorthand(
    path: &str,
    text: &str,
    origin: &zerodds_config::Origin,
) -> Result<ListenerConfig, ConfigError> {
    let refuse = || {
        invalid(
            path.to_string(),
            origin.clone(),
            alloc::format!(
                "`{text}` is not a listener: write `udp:<host:port>`, `tcp:<host:port>` or `serial:<device>`, or a map with `kind`"
            ),
        )
    };
    let (kind, rest) = text.split_once(':').ok_or_else(refuse)?;
    if rest.is_empty() {
        return Err(refuse());
    }
    match kind.trim() {
        "udp" => Ok(ListenerConfig::Udp {
            bind: rest.trim().to_string(),
        }),
        "tcp" => Ok(ListenerConfig::Tcp {
            bind: rest.trim().to_string(),
        }),
        "serial" => Ok(ListenerConfig::Serial {
            device: rest.trim().to_string(),
            baud_rate: 115_200,
            address: 0,
            frame_check: zerodds_xrce::transport_serial::FrameCheck::Rfc1662,
        }),
        _ => Err(refuse()),
    }
}

fn read_listener(path: &str, item: &zerodds_config::Node) -> Result<ListenerConfig, ConfigError> {
    if let zerodds_config::Node::Scalar(text, origin) = item {
        return listener_from_shorthand(path, text, origin);
    }
    let entry = Section::of(Some(item), path)?;
    entry.only(&[
        "kind",
        "bind",
        "device",
        "baud_rate",
        "address",
        "frame_check",
        "cert_file",
        "key_file",
        "client_ca_file",
    ])?;
    let kind = entry
        .choice("kind", &["udp", "tcp", "serial", "tls", "dtls"])?
        .ok_or_else(|| {
            invalid(
                entry.path_of("kind"),
                zerodds_config::Origin::Default,
                "listeners[] entry needs `kind: udp|tcp|serial|tls|dtls`".to_string(),
            )
        })?;
    match kind.as_str() {
        "udp" => Ok(ListenerConfig::Udp {
            bind: entry
                .string("bind")?
                .unwrap_or_else(|| "0.0.0.0:2018".to_string()),
        }),
        "tcp" => Ok(ListenerConfig::Tcp {
            bind: entry
                .string("bind")?
                .unwrap_or_else(|| "0.0.0.0:2018".to_string()),
        }),
        "tls" | "dtls" => {
            let need = |key: &str| {
                entry.string(key)?.ok_or_else(|| {
                    invalid(
                        entry.path_of(key),
                        zerodds_config::Origin::Default,
                        alloc::format!("a {kind} listener needs `{key}:`"),
                    )
                })
            };
            let (bind, cert_file, key_file) =
                (need("bind")?, need("cert_file")?, need("key_file")?);
            let client_ca_file = entry.string("client_ca_file")?;
            Ok(if kind == "tls" {
                ListenerConfig::Tls {
                    bind,
                    cert_file,
                    key_file,
                    client_ca_file,
                }
            } else {
                ListenerConfig::Dtls {
                    bind,
                    cert_file,
                    key_file,
                    client_ca_file,
                }
            })
        }
        _ => {
            let device = entry.string("device")?.ok_or_else(|| {
                invalid(
                    entry.path_of("device"),
                    zerodds_config::Origin::Default,
                    "a serial listener needs `device: /dev/ttyUSB0`".to_string(),
                )
            })?;
            let baud_rate = entry
                .parse::<u32>("baud_rate", "a baud rate, an integer")?
                .unwrap_or(115_200);
            let address = entry
                .parse::<u8>("address", "an address on the line, 0 to 255")?
                .unwrap_or(0);
            let frame_check = match entry
                .choice("frame_check", &["rfc1662", "crc16-arc-payload"])?
                .as_deref()
            {
                Some("crc16-arc-payload") => {
                    zerodds_xrce::transport_serial::FrameCheck::Crc16ArcPayload
                }
                _ => zerodds_xrce::transport_serial::FrameCheck::Rfc1662,
            };
            Ok(ListenerConfig::Serial {
                device,
                baud_rate,
                address,
                frame_check,
            })
        }
    }
}

/// One `access_rules[]` entry (DDS-XRCE 1.0 §7.4): `effect: allow|deny`
/// and the optional `client`, `operations`, `kinds`, `domains`, `topics`
/// that narrow it.
fn read_access_rule(
    path: &str,
    item: &zerodds_config::Node,
) -> Result<crate::daemon::access::Rule, ConfigError> {
    use crate::daemon::access::{Effect, Operation, Rule, kind_from_name, parse_client_key};
    let entry = Section::of(Some(item), path)?;
    entry.only(&[
        "effect",
        "client",
        "operations",
        "kinds",
        "domains",
        "topics",
    ])?;
    let effect = match entry.choice("effect", &["allow", "deny"])?.as_deref() {
        Some("deny") => Effect::Deny,
        Some(_) => Effect::Allow,
        None => {
            return Err(invalid(
                entry.path_of("effect"),
                zerodds_config::Origin::Default,
                "an access_rules[] entry needs `effect: allow|deny`".to_string(),
            ));
        }
    };
    let client = match entry.scalar("client")? {
        Some((text, origin)) => Some(parse_client_key(text).ok_or_else(|| {
            invalid(
                entry.path_of("client"),
                origin.clone(),
                "expected a ClientKey, the four octets in hexadecimal (e.g. `0xAABBCCDD`)"
                    .to_string(),
            )
        })?),
        None => None,
    };
    let origin_of = |key: &str| {
        entry.node(key).map_or(
            zerodds_config::Origin::Default,
            zerodds_config::read::first_origin,
        )
    };
    let mut operations = Vec::new();
    for name in entry.strings("operations")?.unwrap_or_default() {
        operations.push(Operation::from_name(&name).ok_or_else(|| {
            invalid(
                entry.path_of("operations"),
                origin_of("operations"),
                alloc::format!(
                    "`{name}` is not an operation: create, update, get_info, delete, write_data, read_data"
                ),
            )
        })?);
    }
    let mut kinds = Vec::new();
    for name in entry.strings("kinds")?.unwrap_or_default() {
        kinds.push(kind_from_name(&name).ok_or_else(|| {
            invalid(
                entry.path_of("kinds"),
                origin_of("kinds"),
                alloc::format!(
                    "`{name}` is not an object kind: participant, topic, publisher, subscriber, datawriter, datareader, type, qosprofile, application"
                ),
            )
        })?);
    }
    let mut domains = Vec::new();
    for text in entry.strings("domains")?.unwrap_or_default() {
        domains.push(text.trim().parse::<u16>().map_err(|_| {
            invalid(
                entry.path_of("domains"),
                origin_of("domains"),
                alloc::format!("`{text}` is not a domain id, an integer"),
            )
        })?);
    }
    Ok(Rule {
        client,
        effect,
        operations,
        kinds,
        domains,
        topics: entry.strings("topics")?.unwrap_or_default(),
    })
}

fn read_type_file(path: &str, item: &zerodds_config::Node) -> Result<TypeFileConfig, ConfigError> {
    let entry = Section::of(Some(item), path)?;
    entry.only(&["name", "file"])?;
    let type_name = entry.string("name")?.ok_or_else(|| {
        invalid(
            entry.path_of("name"),
            zerodds_config::Origin::Default,
            "type_files[] entry needs `name: TheTypeName`".to_string(),
        )
    })?;
    let file = entry.string("file")?.ok_or_else(|| {
        invalid(
            entry.path_of("file"),
            zerodds_config::Origin::Default,
            "type_files[] entry needs `file: path/to.typeobject`".to_string(),
        )
    })?;
    Ok(TypeFileConfig {
        type_name,
        file: PathBuf::from(file),
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_one_udp_listener_on_2018() {
        let cfg = DaemonConfig::default();
        assert_eq!(
            cfg.listeners,
            vec![ListenerConfig::Udp {
                bind: "0.0.0.0:2018".to_string()
            }]
        );
    }

    #[test]
    fn from_config_none_yields_defaults() {
        let cfg = DaemonConfig::from_config(None).expect("defaults are valid");
        assert_eq!(cfg, DaemonConfig::default());
    }

    /// A serial listener takes the agent's address on the line and the
    /// frame check; Annex C is the default, anything else is refused.
    #[test]
    fn a_serial_listener_takes_its_address_and_frame_check() {
        let cfg = DaemonConfig::load_from_str(
            "listeners:\n  - kind: serial\n    device: /dev/ttyS1\n    address: 7\n    frame_check: crc16-arc-payload\n",
        )
        .expect("serial listener");
        assert_eq!(
            cfg.listeners,
            vec![ListenerConfig::Serial {
                device: "/dev/ttyS1".to_string(),
                baud_rate: 115_200,
                address: 7,
                frame_check: zerodds_xrce::transport_serial::FrameCheck::Crc16ArcPayload,
            }]
        );
        assert!(
            DaemonConfig::load_from_str(
                "listeners:\n  - kind: serial\n    device: /dev/ttyS1\n    frame_check: crc32\n"
            )
            .is_err()
        );
        assert!(
            DaemonConfig::load_from_str(
                "listeners:\n  - kind: serial\n    device: /dev/ttyS1\n    address: 300\n"
            )
            .is_err()
        );
    }

    /// DDS-XRCE 1.0 §11.2, §11.3: a DTLS and a TLS listener carry the
    /// agent's certificate and key; without them the entry is refused.
    #[test]
    fn a_dtls_and_a_tls_listener_name_their_certificate() {
        let cfg = DaemonConfig::load_from_str(
            "listeners:\n  - kind: dtls\n    bind: 0.0.0.0:2019\n    cert_file: /etc/xrce/agent.pem\n    key_file: /etc/xrce/agent.key\n  - kind: tls\n    bind: 0.0.0.0:2020\n    cert_file: /etc/xrce/agent.pem\n    key_file: /etc/xrce/agent.key\n    client_ca_file: /etc/xrce/ca.pem\n",
        )
        .expect("secure listeners");
        assert_eq!(
            cfg.listeners,
            vec![
                ListenerConfig::Dtls {
                    bind: "0.0.0.0:2019".to_string(),
                    cert_file: "/etc/xrce/agent.pem".to_string(),
                    key_file: "/etc/xrce/agent.key".to_string(),
                    client_ca_file: None,
                },
                ListenerConfig::Tls {
                    bind: "0.0.0.0:2020".to_string(),
                    cert_file: "/etc/xrce/agent.pem".to_string(),
                    key_file: "/etc/xrce/agent.key".to_string(),
                    client_ca_file: Some("/etc/xrce/ca.pem".to_string()),
                },
            ]
        );
        for missing in [
            "listeners:\n  - kind: dtls\n    bind: 0.0.0.0:2019\n    key_file: k\n",
            "listeners:\n  - kind: tls\n    bind: 0.0.0.0:2019\n    cert_file: c\n",
            "listeners:\n  - kind: tls\n    cert_file: c\n    key_file: k\n",
        ] {
            assert!(DaemonConfig::load_from_str(missing).is_err(), "{missing}");
        }
    }

    #[test]
    fn a_listener_can_be_written_as_one_string() {
        let cfg = DaemonConfig::load_from_str(
            "listeners:\n  - udp:127.0.0.1:2019\n  - tcp:0.0.0.0:2020\n  - serial:/dev/ttyUSB0\n",
        )
        .expect("shorthand listeners");
        assert_eq!(
            cfg.listeners,
            vec![
                ListenerConfig::Udp {
                    bind: "127.0.0.1:2019".to_string()
                },
                ListenerConfig::Tcp {
                    bind: "0.0.0.0:2020".to_string()
                },
                ListenerConfig::Serial {
                    device: "/dev/ttyUSB0".to_string(),
                    baud_rate: 115_200,
                    address: 0,
                    frame_check: zerodds_xrce::transport_serial::FrameCheck::Rfc1662,
                },
            ]
        );
        assert!(DaemonConfig::load_from_str("listeners:\n  - udp:\n").is_err());
        assert!(DaemonConfig::load_from_str("listeners:\n  - 127.0.0.1:2019\n").is_err());
    }

    /// DDS-XRCE 1.0 §7.4: the controller's keys come from the file in the
    /// order the octets are written, and a malformed rule is refused.
    #[test]
    fn the_access_controller_is_read_from_the_file() {
        use crate::daemon::access::{Effect, Operation};
        let cfg = DaemonConfig::load_from_str(
            "authorized_client_keys:\n  - 0xAABBCCDD\nrequired_client_properties:\n  - name: auth\n    value: token\naccess_rules:\n  - effect: allow\n    client: 0xAABBCCDD\n    operations: [write_data, read_data]\n    kinds: [datawriter]\n    domains: [0, 7]\n    topics: [\"sensor/*\"]\n",
        )
        .expect("access controller");
        assert_eq!(
            cfg.access.authorized_clients,
            vec![[0xAA, 0xBB, 0xCC, 0xDD]]
        );
        assert_eq!(
            cfg.access.required_properties,
            vec![("auth".to_string(), "token".to_string())]
        );
        let rule = &cfg.access.rules[0];
        assert_eq!(rule.effect, Effect::Allow);
        assert_eq!(rule.client, Some([0xAA, 0xBB, 0xCC, 0xDD]));
        assert_eq!(
            rule.operations,
            vec![Operation::WriteData, Operation::ReadData]
        );
        assert_eq!(rule.domains, vec![0, 7]);
        assert_eq!(rule.topics, vec!["sensor/*".to_string()]);
        for bad in [
            "authorized_client_keys:\n  - 0x1234\n",
            "access_rules:\n  - client: 0xAABBCCDD\n",
            "access_rules:\n  - effect: allow\n    operations: [write]\n",
            "access_rules:\n  - effect: allow\n    kinds: [agent]\n",
            "access_rules:\n  - effect: allow\n    domains: [x]\n",
        ] {
            assert!(DaemonConfig::load_from_str(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_create_client_for_the_same_session_can_restart_the_streams() {
        assert!(!DaemonConfig::default().restart_on_same_session);
        let cfg = DaemonConfig::load_from_str("same_session_create_client: restart\n").unwrap();
        assert!(cfg.restart_on_same_session);
        let cfg = DaemonConfig::load_from_str("same_session_create_client: keep\n").unwrap();
        assert!(!cfg.restart_on_same_session);
        assert!(DaemonConfig::load_from_str("same_session_create_client: maybe\n").is_err());
    }

    #[test]
    fn the_discovery_group_can_be_set() {
        let default = DaemonConfig::default();
        assert_eq!(
            default.discovery_address,
            std::net::Ipv4Addr::new(239, 255, 0, 2)
        );
        let cfg = DaemonConfig::load_from_str("discovery:\n  address: 239.1.2.3\n  port: 7500\n")
            .unwrap();
        assert_eq!(cfg.discovery_address, std::net::Ipv4Addr::new(239, 1, 2, 3));
        assert_eq!(cfg.discovery_port, 7500);
        for bad in [
            "discovery:\n  address: 10.0.0.1\n",
            "discovery:\n  address: nonsense\n",
        ] {
            assert!(DaemonConfig::load_from_str(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_no_client_threshold_can_be_set() {
        assert_eq!(DaemonConfig::default().no_client_threshold, 16);
        let cfg = DaemonConfig::load_from_str("no_client_threshold: 3\n").unwrap();
        assert_eq!(cfg.no_client_threshold, 3);
        assert!(DaemonConfig::load_from_str("no_client_threshold: many\n").is_err());
    }

    #[test]
    fn the_reply_to_delete_client_can_be_set() {
        assert!(!DaemonConfig::default().delete_client_reply_is_status_agent);
        let cfg = DaemonConfig::load_from_str("delete_client_reply: status_agent\n").unwrap();
        assert!(cfg.delete_client_reply_is_status_agent);
        let cfg = DaemonConfig::load_from_str("delete_client_reply: status\n").unwrap();
        assert!(!cfg.delete_client_reply_is_status_agent);
        assert!(DaemonConfig::load_from_str("delete_client_reply: info\n").is_err());
    }

    #[test]
    fn the_payload_of_status_agent_can_be_set() {
        assert!(!DaemonConfig::default().status_agent_with_result_status);
        let cfg = DaemonConfig::load_from_str("status_agent: with_result_status\n").unwrap();
        assert!(cfg.status_agent_with_result_status);
        let cfg = DaemonConfig::load_from_str("status_agent: annex_a\n").unwrap();
        assert!(!cfg.status_agent_with_result_status);
        assert!(DaemonConfig::load_from_str("status_agent: both\n").is_err());
    }

    #[test]
    fn the_unit_of_max_elapsed_time_can_be_set() {
        assert!(!DaemonConfig::default().max_elapsed_time_in_milliseconds);
        let cfg = DaemonConfig::load_from_str("max_elapsed_time_unit: milliseconds\n").unwrap();
        assert!(cfg.max_elapsed_time_in_milliseconds);
        let cfg = DaemonConfig::load_from_str("max_elapsed_time_unit: seconds\n").unwrap();
        assert!(!cfg.max_elapsed_time_in_milliseconds);
        assert!(DaemonConfig::load_from_str("max_elapsed_time_unit: minutes\n").is_err());
    }

    #[test]
    fn unsupported_qos_is_refused_unless_ignoring_is_named() {
        assert!(!DaemonConfig::default().ignore_unsupported_qos);
        let cfg = DaemonConfig::load_from_str("unsupported_qos: ignore\n").unwrap();
        assert!(cfg.ignore_unsupported_qos);
        let cfg = DaemonConfig::load_from_str("unsupported_qos: refuse\n").unwrap();
        assert!(!cfg.ignore_unsupported_qos);
        assert!(DaemonConfig::load_from_str("unsupported_qos: warn\n").is_err());
    }

    #[test]
    fn the_form_of_the_sample_info_can_be_set() {
        use zerodds_xrce::encoding::SampleInfoForm;
        assert_eq!(
            DaemonConfig::default().sample_info_form,
            SampleInfoForm::AnnexA
        );
        let cfg = DaemonConfig::load_from_str("sample_info: flat\n").unwrap();
        assert_eq!(cfg.sample_info_form, SampleInfoForm::Flat);
        let cfg = DaemonConfig::load_from_str("sample_info: annex_a\n").unwrap();
        assert_eq!(cfg.sample_info_form, SampleInfoForm::AnnexA);
        assert!(DaemonConfig::load_from_str("sample_info: union\n").is_err());
    }

    #[test]
    fn the_representation_formats_can_be_widened() {
        assert!(!DaemonConfig::default().allow_any_representation_format);
        let cfg = DaemonConfig::load_from_str("representation_formats: any\n").unwrap();
        assert!(cfg.allow_any_representation_format);
        let cfg = DaemonConfig::load_from_str("representation_formats: spec\n").unwrap();
        assert!(!cfg.allow_any_representation_format);
        assert!(DaemonConfig::load_from_str("representation_formats: some\n").is_err());
    }

    #[test]
    fn the_form_of_the_representations_can_be_set() {
        use zerodds_xrce::encoding::RepresentationTimestamps;
        assert_eq!(
            DaemonConfig::default().representation_timestamps,
            RepresentationTimestamps::Absent
        );
        let cfg = DaemonConfig::load_from_str("representation_timestamps: present\n").unwrap();
        assert_eq!(
            cfg.representation_timestamps,
            RepresentationTimestamps::Present
        );
        let cfg = DaemonConfig::load_from_str("representation_timestamps: absent\n").unwrap();
        assert_eq!(
            cfg.representation_timestamps,
            RepresentationTimestamps::Absent
        );
        assert!(DaemonConfig::load_from_str("representation_timestamps: maybe\n").is_err());
    }

    #[test]
    fn vendor_forms_of_dds_xml_are_a_named_setting() {
        assert!(!DaemonConfig::default().vendor_forms);
        let cfg = DaemonConfig::load_from_str("vendor_forms: accept\n").unwrap();
        assert!(cfg.vendor_forms);
        let cfg = DaemonConfig::load_from_str("vendor_forms: refuse\n").unwrap();
        assert!(!cfg.vendor_forms);
        assert!(DaemonConfig::load_from_str("vendor_forms: maybe\n").is_err());
    }

    #[test]
    fn the_extensibility_of_unannotated_types_can_be_set() {
        use zerodds_xrce::encoding::UnannotatedTypes;
        assert_eq!(
            DaemonConfig::default().unannotated_types,
            UnannotatedTypes::Appendable
        );
        let cfg = DaemonConfig::load_from_str("unannotated_types: final\n").unwrap();
        assert_eq!(cfg.unannotated_types, UnannotatedTypes::Final);
        let cfg = DaemonConfig::load_from_str("unannotated_types: appendable\n").unwrap();
        assert_eq!(cfg.unannotated_types, UnannotatedTypes::Appendable);
        assert!(DaemonConfig::load_from_str("unannotated_types: mutable\n").is_err());
    }

    #[test]
    fn client_applications_are_read() {
        let cfg = DaemonConfig::load_from_str(
            "client_applications:\n  - client: 0xAABBCCDD\n    application: \"Apps::App\"\n",
        )
        .unwrap();
        assert_eq!(
            cfg.client_applications,
            vec![([0xAA, 0xBB, 0xCC, 0xDD], "Apps::App".to_string())]
        );
        for bad in [
            "client_applications:\n  - application: \"A\"\n",
            "client_applications:\n  - client: 0x12\n    application: \"A\"\n",
            "client_applications:\n  - client: 0xAABBCCDD\n",
            "client_applications:\n  - client: 0xAABBCCDD\n    application: \"A\"\n    x: 1\n",
        ] {
            assert!(DaemonConfig::load_from_str(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn unknown_topic_types_are_refused_unless_forwarding_is_named() {
        assert!(!DaemonConfig::default().forward_unknown_topic_types);
        let cfg = DaemonConfig::load_from_str("unknown_topic_types: forward\n").unwrap();
        assert!(cfg.forward_unknown_topic_types);
        let cfg = DaemonConfig::load_from_str("unknown_topic_types: refuse\n").unwrap();
        assert!(!cfg.forward_unknown_topic_types);
        assert!(DaemonConfig::load_from_str("unknown_topic_types: sometimes\n").is_err());
    }

    #[test]
    fn parse_vendor_id_accepts_major_minor() {
        assert_eq!(parse_vendor_id("1.15"), Some([1, 15]));
        assert_eq!(parse_vendor_id("0.0"), Some([0, 0]));
        assert_eq!(parse_vendor_id("not-a-vendor-id"), None);
    }
}
