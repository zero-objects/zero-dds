// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Process wiring for `zerodds-xrce-agentd`: builds the
//! [`SessionEngine`]/[`RealDdsPort`], starts every configured listener
//! plus the discovery responder, and answers SIGHUP (reload what can be
//! reloaded)/SIGTERM (clean shutdown)/systemd `sd_notify`.
//!
//! ## SIGHUP reload scope
//!
//! Reloadable without a restart: `log_level`, `limits.*`,
//! `session_timeout_secs`, `discovery.enabled` (the responder thread is
//! stopped/started accordingly). **Not** reloadable — a changed
//! `listeners:`/`type_files:`/`xml_profile_files:` entry is
//! logged and ignored until restart, because live-rebinding a listening
//! socket or swapping the `TypeObject` set under in-flight sessions would
//! risk silently orphaning them; the daemon says so on stderr rather than
//! attempting it.

#![allow(clippy::print_stderr)]

use std::net::{TcpListener, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use zerodds_dcps::runtime::RuntimeConfig;

use crate::daemon::config::{DaemonConfig, ListenerConfig, TypeFileConfig};
use crate::daemon::dcps_bridge::RealDdsPort;
use crate::daemon::session::{SessionEngine, SessionEngineConfig};
use crate::daemon::{discovery, transport};

/// Daemon top-level error.
#[derive(Debug)]
pub enum ServerError {
    /// A setting that cannot be served (exit 1).
    Config(String),
    /// A listener could not bind (exit 2).
    Bind(String),
    /// DDS init failed (exit 3).
    Dds(String),
}

impl core::fmt::Display for ServerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Config(m) => write!(f, "config: {m}"),
            Self::Bind(m) => write!(f, "bind: {m}"),
            Self::Dds(m) => write!(f, "dds: {m}"),
        }
    }
}

impl std::error::Error for ServerError {}

/// A running daemon.
pub struct DaemonHandle {
    stop: Arc<AtomicBool>,
    reload: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    engine: Arc<Mutex<SessionEngine<RealDdsPort>>>,
    reload_fn: Option<Box<dyn FnMut() -> Result<DaemonConfig, String> + Send>>,
    /// The first UDP listener's real bound address, read back from the
    /// socket right after `bind` — lets a caller that configured
    /// `0.0.0.0:0`/`127.0.0.1:0` (an OS-assigned ephemeral port, e.g. so
    /// two test runs never collide on a fixed port) find out which port
    /// it actually got. `None` if no UDP listener is configured.
    udp_local_addr: Option<std::net::SocketAddr>,
}

impl DaemonHandle {
    /// `true` once SIGTERM/SIGINT has been observed.
    #[must_use]
    pub fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    /// The first UDP listener's real bound address — see the field doc.
    #[must_use]
    pub fn udp_local_addr(&self) -> Option<std::net::SocketAddr> {
        self.udp_local_addr
    }

    /// The shutdown flag, to hand to [`install_signal_watcher`] so
    /// SIGTERM/SIGINT sets the same flag [`Self::stop_requested`] reads.
    #[must_use]
    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.stop)
    }

    /// The reload flag, to hand to [`install_signal_watcher`] so SIGHUP
    /// sets the same flag [`Self::reload_if_requested`] consumes.
    #[must_use]
    pub fn reload_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.reload)
    }

    /// Applies a SIGHUP reload if one is pending (see the module doc for
    /// what is and is not reloadable).
    pub fn reload_if_requested(&mut self) {
        if !self.reload.swap(false, Ordering::Relaxed) {
            return;
        }
        let Some(reload_fn) = self.reload_fn.as_mut() else {
            return;
        };
        match reload_fn() {
            Ok(cfg) => {
                if let Ok(mut e) = self.engine.lock() {
                    apply_reloadable(&mut e, &cfg);
                }
                eprintln!(
                    "[zerodds-xrce-agentd] reloaded (log_level/limits/session_timeout/discovery.enabled)"
                );
            }
            Err(m) => eprintln!(
                "[zerodds-xrce-agentd] reload failed, keeping the running configuration: {m}"
            ),
        }
    }

    /// Stops every listener thread and joins them.
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn apply_reloadable(engine: &mut SessionEngine<RealDdsPort>, cfg: &DaemonConfig) {
    engine.set_reloadable_limits(
        cfg.max_clients,
        cfg.max_objects_per_client,
        std::time::Duration::from_secs(cfg.session_timeout_secs),
    );
}

/// Reads every `type_files:` entry: an XCDR2 `TypeObject` that must be
/// COMPLETE and name the type it is configured under — a file that
/// describes a different type than its entry says would put the wrong
/// codec behind a topic.
///
/// # Errors
/// [`ServerError::Config`] naming the entry.
pub fn load_type_files(
    files: &[TypeFileConfig],
) -> Result<std::collections::BTreeMap<String, zerodds_types::CompleteTypeObject>, ServerError> {
    let mut out = std::collections::BTreeMap::new();
    for entry in files {
        let fail = |what: String| {
            ServerError::Config(format!(
                "type_files: `{}` ({}): {what}",
                entry.type_name,
                entry.file.display()
            ))
        };
        let bytes = std::fs::read(&entry.file).map_err(|e| fail(e.to_string()))?;
        let named = crate::daemon::entity_repr::types(
            &zerodds_xrce::object_repr::ObjectVariant::InBinary(bytes),
            &crate::daemon::profiles::AgentProfiles::default(),
            &std::collections::BTreeMap::new(),
        )
        .map_err(|e| fail(e.message().to_string()))?;
        let [(name, root)] = <[_; 1]>::try_from(named)
            .map_err(|_| fail("not exactly one TypeObject".to_string()))?;
        if name != entry.type_name {
            return Err(fail(format!("the TypeObject describes `{name}`")));
        }
        if out.insert(name, root).is_some() {
            return Err(fail("the type name is configured twice".to_string()));
        }
    }
    Ok(out)
}

/// A function that reloads the layered config (files re-read, `--set`
/// flags re-applied) — the same closure `zerodds-coap-bridged` builds
/// around `Configured::from_command_line`.
pub type ConfigLoader = Box<dyn FnMut() -> Result<DaemonConfig, String> + Send>;

/// Builds and starts the daemon: one `RealDdsPort`/`SessionEngine`, every
/// configured listener, and the discovery responder if enabled.
///
/// # Errors
/// [`ServerError::Config`] if a `type_files:`, `xml_profile_files:` or
/// `qos_library_files:` entry does not load;
/// [`ServerError::Bind`] if a listener's address is already in use;
/// [`ServerError::Dds`] if the initial `DcpsRuntime` for the default
/// domain fails to start.
pub fn start(
    cfg: DaemonConfig,
    runtime_config: RuntimeConfig,
    reload: Option<ConfigLoader>,
) -> Result<DaemonHandle, ServerError> {
    // The way the types of Annex A without @extensibility are written is a
    // setting of the process (zerodds_xrce::encoding::set_unannotated_types).
    zerodds_xrce::encoding::set_unannotated_types(cfg.unannotated_types);
    // So is the form of CLIENT_/AGENT_Representation (with or without the
    // timestamp member, §7.8.2.1).
    zerodds_xrce::encoding::set_representation_timestamps(cfg.representation_timestamps);
    // And the form of the SampleInfo (the union of Annex A or the flat struct).
    zerodds_xrce::encoding::set_sample_info_form(cfg.sample_info_form);
    let mut engine_config = SessionEngineConfig {
        agent_vendor_id: cfg.vendor_id,
        max_clients: cfg.max_clients,
        max_objects_per_client: cfg.max_objects_per_client,
        max_pending_samples: cfg.max_pending_samples,
        session_timeout: std::time::Duration::from_secs(cfg.session_timeout_secs),
        listening_locators: Vec::new(),
        known_types: std::collections::BTreeMap::new(),
        profiles: crate::daemon::profiles::AgentProfiles::default(),
        read_without_delivery_control: cfg.read_without_delivery_control,
        forward_unknown_topic_types: cfg.forward_unknown_topic_types,
        ignore_unsupported_qos: cfg.ignore_unsupported_qos,
        status_agent_with_result_status: cfg.status_agent_with_result_status,
        delete_client_reply_is_status_agent: cfg.delete_client_reply_is_status_agent,
        max_elapsed_time_in_milliseconds: cfg.max_elapsed_time_in_milliseconds,
        restart_on_same_session: cfg.restart_on_same_session,
        refuse_update_of_missing_object: cfg.refuse_update_of_missing_object,
        allow_any_representation_format: cfg.allow_any_representation_format,
        timestamp_interval: (cfg.timestamp_interval_secs > 0)
            .then(|| std::time::Duration::from_secs(cfg.timestamp_interval_secs)),
        client_applications: cfg.client_applications,
        access: cfg.access,
    };
    let profiles = crate::daemon::profiles::AgentProfiles::load_with(
        &cfg.xml_profile_files,
        &cfg.qos_library_files,
        cfg.vendor_forms,
    )
    .map_err(|e| ServerError::Config(format!("xml_profile_files/qos_library_files: {e}")))?;
    for note in profiles.vendor_notes() {
        eprintln!("[zerodds-xrce-agentd] vendor form read as given: {note}");
    }
    for unused in profiles.unused() {
        eprintln!("[zerodds-xrce-agentd] profile element {unused} is not used by this agent");
    }
    // Types of the profile files and of type_files: together; one name,
    // one definition.
    let mut known_types = load_type_files(&cfg.type_files)?;
    for (name, def) in profiles.types() {
        if known_types.insert(name.clone(), def.clone()).is_some() {
            return Err(ServerError::Config(format!(
                "type `{name}` is defined by type_files: and by xml_profile_files:"
            )));
        }
    }
    engine_config.known_types = known_types;
    engine_config.profiles = profiles;
    engine_config.listening_locators = listener_locators(&cfg.listeners);

    let dds = RealDdsPort::new(runtime_config);
    let engine = Arc::new(Mutex::new(SessionEngine::new(dds, engine_config)));

    let stop = Arc::new(AtomicBool::new(false));
    let mut threads = Vec::new();
    let peers = transport::new_peer_table_with(cfg.no_client_threshold);
    let mut udp_local_addr = None;

    for listener in &cfg.listeners {
        match listener {
            ListenerConfig::Udp { bind } => {
                let socket = UdpSocket::bind(bind)
                    .map_err(|e| ServerError::Bind(format!("udp {bind}: {e}")))?;
                if udp_local_addr.is_none() {
                    udp_local_addr = socket.local_addr().ok();
                }
                let engine = Arc::clone(&engine);
                let peers = Arc::clone(&peers);
                let stop = Arc::clone(&stop);
                threads.push(thread::spawn(move || {
                    transport::run_udp_listener(socket, engine, peers, stop)
                }));
            }
            ListenerConfig::Tcp { bind } => {
                let listener = TcpListener::bind(bind)
                    .map_err(|e| ServerError::Bind(format!("tcp {bind}: {e}")))?;
                let engine = Arc::clone(&engine);
                let peers = Arc::clone(&peers);
                let stop = Arc::clone(&stop);
                threads.push(thread::spawn(move || {
                    transport::run_tcp_listener(listener, engine, peers, stop)
                }));
            }
            ListenerConfig::Tls {
                bind,
                cert_file,
                key_file,
                client_ca_file,
            } => {
                #[cfg(feature = "tls")]
                {
                    let addr: std::net::SocketAddr = bind
                        .parse()
                        .map_err(|e| ServerError::Config(format!("tls {bind}: {e}")))?;
                    let server =
                        zerodds_xrce::transport_tls::RustlsTlsServer::bind_with_certificate(
                            addr,
                            cert_file,
                            key_file,
                            client_ca_file.as_deref(),
                        )
                        .map_err(|e| ServerError::Bind(format!("tls {bind}: {e}")))?;
                    let engine = Arc::clone(&engine);
                    let peers = Arc::clone(&peers);
                    let stop = Arc::clone(&stop);
                    threads.push(thread::spawn(move || {
                        transport::secure::run_tls_listener(server, engine, peers, stop)
                    }));
                }
                #[cfg(not(feature = "tls"))]
                {
                    let _ = (cert_file, key_file, client_ca_file);
                    return Err(ServerError::Config(format!(
                        "listeners[]: kind: tls ({bind}) needs this build's `tls` feature"
                    )));
                }
            }
            ListenerConfig::Dtls {
                bind,
                cert_file,
                key_file,
                client_ca_file,
            } => {
                #[cfg(feature = "dtls")]
                {
                    let addr: std::net::SocketAddr = bind
                        .parse()
                        .map_err(|e| ServerError::Config(format!("dtls {bind}: {e}")))?;
                    let listener = zerodds_xrce::transport_dtls::WebrtcDtlsListener::bind(
                        addr,
                        &zerodds_xrce::transport_dtls::DtlsServerCredentials {
                            cert_file: cert_file.clone(),
                            key_file: key_file.clone(),
                            client_ca_file: client_ca_file.clone(),
                        },
                    )
                    .map_err(|e| ServerError::Bind(format!("dtls {bind}: {e}")))?;
                    let engine = Arc::clone(&engine);
                    let peers = Arc::clone(&peers);
                    let stop = Arc::clone(&stop);
                    threads.push(thread::spawn(move || {
                        transport::secure::run_dtls_listener(listener, engine, peers, stop)
                    }));
                }
                #[cfg(not(feature = "dtls"))]
                {
                    let _ = (cert_file, key_file, client_ca_file);
                    return Err(ServerError::Config(format!(
                        "listeners[]: kind: dtls ({bind}) needs this build's `dtls` feature"
                    )));
                }
            }
            ListenerConfig::Serial {
                device,
                baud_rate,
                address,
                frame_check,
            } => {
                #[cfg(feature = "serial")]
                {
                    let engine = Arc::clone(&engine);
                    let peers = Arc::clone(&peers);
                    let stop = Arc::clone(&stop);
                    let device = device.clone();
                    let (baud_rate, address, frame_check) = (*baud_rate, *address, *frame_check);
                    threads.push(thread::spawn(move || {
                        if let Err(e) = transport::serial::run_serial_listener(
                            &device,
                            baud_rate,
                            address,
                            frame_check,
                            engine,
                            peers,
                            stop,
                        ) {
                            eprintln!("[zerodds-xrce-agentd] serial {device}: {e}");
                        }
                    }));
                }
                #[cfg(not(feature = "serial"))]
                {
                    let _ = (address, frame_check);
                    return Err(ServerError::Config(format!(
                        "listeners[]: kind: serial (device {device} @ {baud_rate}) needs this build's `serial` feature"
                    )));
                }
            }
        }
    }

    // One clock for all listeners: HEARTBEAT, the samples of a running
    // READ_DATA and the FRAGMENTs go to each client over its transport.
    {
        let engine = Arc::clone(&engine);
        let peers = Arc::clone(&peers);
        let stop = Arc::clone(&stop);
        threads.push(thread::spawn(move || {
            transport::run_ticker(engine, peers, stop)
        }));
    }

    if cfg.discovery_enabled {
        match discovery::bind_discovery_socket(cfg.discovery_address, cfg.discovery_port) {
            Ok(socket) => {
                let locators = listener_locators(&cfg.listeners);
                let vendor_id = cfg.vendor_id;
                let stop = Arc::clone(&stop);
                threads.push(thread::spawn(move || {
                    discovery::run_discovery_responder(socket, locators, vendor_id, stop)
                }));
            }
            Err(e) => {
                // Discovery is a convenience, not a hard requirement (a
                // client can always be pointed at a listener directly) —
                // logged, the daemon still starts.
                eprintln!(
                    "[zerodds-xrce-agentd] discovery: could not bind port {}: {e}",
                    cfg.discovery_port
                );
            }
        }
    }

    Ok(DaemonHandle {
        stop,
        reload: Arc::new(AtomicBool::new(false)),
        threads,
        engine,
        reload_fn: reload,
        udp_local_addr,
    })
}

/// The locators `GET_INFO(OBJECTID_AGENT)` reports for the agent's listeners
/// (DDS-XRCE 1.0 §7.8.2.2 `address_seq`): a UDP or TCP listener on IPv4 as
/// `ADDRESS_FORMAT_MEDIUM`, on IPv6 as `ADDRESS_FORMAT_LARGE` (§11.2.1,
/// §11.3.1). A listener that is protected (DTLS, TLS: §8.3.2.4, §11.2, §11.3)
/// or has no IP address (a serial line) has no value of its own in
/// `TransportLocatorFormat`; it is reported as `ADDRESS_FORMAT_STRING`, a URL
/// that names the protection (`dtls://127.0.0.1:2018`, `tls://…`,
/// `serial:///dev/ttyUSB0?address=0`), so that a client does not take the plain
/// locator of the same host for it and connect without the protection.
fn listener_locators(
    listeners: &[ListenerConfig],
) -> Vec<zerodds_xrce::transport_locator::TransportLocator> {
    use zerodds_xrce::transport_locator::{
        TransportLocator, TransportLocatorLarge, TransportLocatorMedium,
    };
    let mut out = Vec::new();
    for l in listeners {
        match l {
            ListenerConfig::Udp { bind } | ListenerConfig::Tcp { bind } => {
                let Ok(addr) = bind.parse::<std::net::SocketAddr>() else {
                    continue;
                };
                out.push(match addr.ip() {
                    std::net::IpAddr::V4(v4) => TransportLocator::Medium(TransportLocatorMedium {
                        address: v4.octets(),
                        port: addr.port(),
                    }),
                    std::net::IpAddr::V6(v6) => TransportLocator::Large(TransportLocatorLarge {
                        address: v6.octets(),
                        port: u32::from(addr.port()),
                    }),
                });
            }
            ListenerConfig::Dtls { bind, .. } => {
                if bind.parse::<std::net::SocketAddr>().is_ok() {
                    out.push(TransportLocator::String(format!("dtls://{bind}")));
                }
            }
            ListenerConfig::Tls { bind, .. } => {
                if bind.parse::<std::net::SocketAddr>().is_ok() {
                    out.push(TransportLocator::String(format!("tls://{bind}")));
                }
            }
            ListenerConfig::Serial {
                device, address, ..
            } => {
                out.push(TransportLocator::String(format!(
                    "serial://{device}?address={address}"
                )));
            }
        }
    }
    out
}

/// SIGTERM/SIGINT/SIGHUP watcher (mirrors `crates/coap-bridge`'s).
#[cfg(unix)]
pub fn install_signal_watcher(
    shutdown_flag: Arc<AtomicBool>,
    reload_flag: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;
    let mut signals = Signals::new([SIGTERM, SIGINT, SIGHUP])?;
    thread::Builder::new()
        .name("zerodds-xrce-signals".into())
        .spawn(move || {
            for sig in signals.forever() {
                match sig {
                    SIGTERM | SIGINT => {
                        shutdown_flag.store(true, Ordering::SeqCst);
                        break;
                    }
                    SIGHUP => reload_flag.store(true, Ordering::SeqCst),
                    _ => {}
                }
            }
        })
}

/// As [`install_signal_watcher`], on a platform with no POSIX signals.
#[cfg(not(unix))]
pub fn install_signal_watcher(
    _shutdown_flag: Arc<AtomicBool>,
    _reload_flag: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    Ok(thread::spawn(|| {}))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// A `type_files:` entry whose TypeObject describes another type than
    /// the entry names is refused at start.
    #[test]
    fn a_type_file_must_describe_the_type_it_is_configured_for() {
        let ast = std::sync::Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "@final struct HelloWorld { unsigned long index; };",
                "server-type-files.idl",
            )
            .unwrap(),
        );
        let ty = zerodds_types::dynamic::DynamicType::find(&ast, "HelloWorld").unwrap();
        let bytes = zerodds_types::TypeObject::Complete(
            zerodds_types::from_ast::complete_type_object(&ast, ty.id()).unwrap(),
        )
        .to_bytes_le()
        .unwrap();
        let dir = std::env::temp_dir().join(format!("xrce-type-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("helloworld.typeobject");
        std::fs::write(&file, &bytes).unwrap();
        let entry = |name: &str| TypeFileConfig {
            type_name: name.to_string(),
            file: file.clone(),
        };
        let loaded = load_type_files(&[entry("HelloWorld")]).unwrap();
        assert!(loaded.contains_key("HelloWorld"));
        assert!(matches!(
            load_type_files(&[entry("Other")]),
            Err(ServerError::Config(_))
        ));
        assert!(matches!(
            load_type_files(&[TypeFileConfig {
                type_name: "HelloWorld".into(),
                file: dir.join("missing"),
            }]),
            Err(ServerError::Config(_))
        ));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// DDS-XRCE 1.0 §11.2.1, §11.3.1: UDP v4 and TCP v4 are
    /// ADDRESS_FORMAT_MEDIUM with the port number, IPv6 is ADDRESS_FORMAT_LARGE;
    /// a serial line has no locator.
    #[test]
    fn listener_locators_maps_udp_tcp_and_ipv6() {
        use zerodds_xrce::transport_locator::{
            TransportLocator, TransportLocatorLarge, TransportLocatorMedium,
        };
        let listeners = vec![
            ListenerConfig::Udp {
                bind: "127.0.0.1:2018".to_string(),
            },
            ListenerConfig::Tcp {
                bind: "10.0.0.7:2019".to_string(),
            },
            ListenerConfig::Udp {
                bind: "[::1]:2020".to_string(),
            },
        ];
        assert_eq!(
            listener_locators(&listeners),
            vec![
                TransportLocator::Medium(TransportLocatorMedium {
                    address: [127, 0, 0, 1],
                    port: 2018,
                }),
                TransportLocator::Medium(TransportLocatorMedium {
                    address: [10, 0, 0, 7],
                    port: 2019,
                }),
                TransportLocator::Large(TransportLocatorLarge {
                    address: std::net::Ipv6Addr::LOCALHOST.octets(),
                    port: 2020,
                }),
            ]
        );
    }

    /// A secured listener and a serial line have no `TransportLocatorFormat` of
    /// their own: they are reported as `ADDRESS_FORMAT_STRING` with a URL that
    /// names the protection, never as the plain locator.
    #[test]
    fn listener_locators_names_the_secured_and_the_serial_listeners() {
        let listeners = vec![
            ListenerConfig::Dtls {
                bind: "127.0.0.1:2018".to_string(),
                cert_file: "c".to_string(),
                key_file: "k".to_string(),
                client_ca_file: None,
            },
            ListenerConfig::Tls {
                bind: "127.0.0.1:2019".to_string(),
                cert_file: "c".to_string(),
                key_file: "k".to_string(),
                client_ca_file: None,
            },
        ];
        let mut listeners = listeners;
        listeners.push(ListenerConfig::Serial {
            device: "/dev/ttyUSB0".to_string(),
            baud_rate: 115_200,
            address: 0,
            frame_check: zerodds_xrce::transport_serial::FrameCheck::Rfc1662,
        });
        assert_eq!(
            listener_locators(&listeners),
            vec![
                zerodds_xrce::transport_locator::TransportLocator::String(
                    "dtls://127.0.0.1:2018".into()
                ),
                zerodds_xrce::transport_locator::TransportLocator::String(
                    "tls://127.0.0.1:2019".into()
                ),
                zerodds_xrce::transport_locator::TransportLocator::String(
                    "serial:///dev/ttyUSB0?address=0".into()
                ),
            ]
        );
    }

    #[test]
    fn listener_locators_skips_unparseable_bind() {
        let listeners = vec![ListenerConfig::Udp {
            bind: "not-an-address".to_string(),
        }];
        assert!(listener_locators(&listeners).is_empty());
    }
}
