// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps` with `zerodds-transport-uds`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-transport-uds`, with the feature
//! `same-host-uds`, in `runtime.rs` (`select_user_transport`,
//! `transport.user_transport = UDS`): `UdsConfig { base_dir, max_datagram,
//! recv_timeout: Some(1 s), send_timeout: DEFAULT_SEND_TIMEOUT, socket_mode,
//! base_dir_mode }` with the values of `RuntimeConfig::uds`
//! (`transport.uds.*`), `UdsTransport::bind(local_id, config)` (mode
//! `datagram`, Unix) or `UdsStreamTransport::bind(local_id, config)` (mode
//! `stream`, the only one on Windows) with `local_id` = the 12 octets of
//! the GuidPrefix followed by the domain id as a big-endian `u32`, and the
//! transport behind `Arc<dyn Transport + Send + Sync>`. It relies on a peer that reads that
//! id from the announced UDS locator resolving it to the socket path this
//! participant bound, under the same base directory, and on the defaults of
//! `transport.uds.*` being the transport's.
//!
//! Oracles: ZeroDDS UDS Transport 1.0 §2.1 (`<base_dir>/<lowercase
//! hex32>.sock`, default `base_dir` `/tmp/zerodds/uds`), §2.2 (default
//! directory mode `0700`), §3.3 (default `max_datagram` 65 536), §4.3 (a
//! send waits at most 100 ms for room by default), §5.1 (no socket mode
//! of its own by default) and §3.5 (the stream transport binds the same
//! path),
//! `docs/spec-coverage/zerodds-uds-transport-1.0.md`;
//! DDSI-RTPS 2.5 §9.3.1.5 for the prefix layout.
//!
//! What the runtime makes of a refused bind: the start fails with
//! `DdsError::TransportError { label: "user unicast bind (UDS)" }`, and the
//! transport's error reaches the observability sink as one event
//! `transport.uds.bind_failed` with the directory, the mode and the error
//! (§3.5, §6.2). Two tests start a participant for it, in a leased domain;
//! its UDS bind fails before a UDS socket is bound. On Windows one of them
//! shows that the mode `datagram` fails the start (§6.2); that platform
//! runs this target in the job `platform-windows-dcps-uds`
//! (`ci/platform_jobs.py`) with the feature `same-host-uds`. The other
//! tests bind no socket and create no directory.
#![cfg(any(unix, windows))]
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use zerodds_dcps::runtime::{RuntimeConfig, UdsMode, UdsUserConfig, UserTransportKind};
use zerodds_dcps::{DdsError, DomainParticipantFactory, DomainParticipantQos};
use zerodds_foundation::observability::{Event, Level, VecSink};
use zerodds_transport::Transport;
#[cfg(unix)]
use zerodds_transport_uds::UdsTransport;
use zerodds_transport_uds::{DEFAULT_BASE_DIR, UdsConfig, UdsStreamTransport, socket_path};

/// The id the runtime binds: prefix (vendor 0x01F0 first) and domain 7.
fn runtime_local_id() -> [u8; 16] {
    let prefix = [
        0x01, 0xF0, 0xAB, 0xCD, 0xEF, 0x01, 0, 0, 0, 0x0A, 0xFF, 0x10,
    ];
    let mut id = [0u8; 16];
    id[..12].copy_from_slice(&prefix);
    id[12..].copy_from_slice(&7_u32.to_be_bytes());
    id
}

#[test]
fn the_announced_id_resolves_to_lowercase_hex_under_the_base_dir() {
    let path = socket_path(Path::new(DEFAULT_BASE_DIR), runtime_local_id());
    // 01f0 abcd ef01 0000 000a ff10 | 0000 0007, the domain big-endian.
    assert_eq!(
        path,
        PathBuf::from("/tmp/zerodds/uds/01f0abcdef010000000aff1000000007.sock")
    );
    // Every id names its own path: two domains on one prefix do not share.
    let mut other = runtime_local_id();
    other[15] = 8;
    assert_ne!(socket_path(Path::new(DEFAULT_BASE_DIR), other), path);
}

#[test]
fn the_runtimes_config_keeps_the_default_directory_and_size() {
    assert_eq!(DEFAULT_BASE_DIR, "/tmp/zerodds/uds");
    let config = UdsConfig {
        recv_timeout: Some(Duration::from_secs(1)),
        ..UdsConfig::default()
    };
    // Both peers resolve under the same directory; the 65 536-octet bound
    // is what a UDS leg reports as its message size.
    assert_eq!(config.base_dir, PathBuf::from("/tmp/zerodds/uds"));
    assert_eq!(config.max_datagram, 65_536);
    assert_eq!(config.recv_timeout, Some(Duration::from_secs(1)));
    assert_eq!(UdsConfig::default().recv_timeout, None);
    // A send to a peer that has stopped reading holds the runtime's sending
    // thread at most this long (§4.3).
    assert_eq!(
        zerodds_transport_uds::DEFAULT_SEND_TIMEOUT,
        Duration::from_millis(100)
    );
    assert_eq!(
        config.send_timeout,
        zerodds_transport_uds::DEFAULT_SEND_TIMEOUT
    );
}

/// zerodds-uds-transport-1.0 §5.1: the runtime's `transport.uds.*`
/// defaults are the transport's own, field by field.
#[test]
fn uds_5_1_the_runtimes_uds_settings_default_to_the_transports_defaults() {
    let runtime = zerodds_dcps::runtime::UdsUserConfig::default();
    let transport = UdsConfig::default();
    assert_eq!(runtime.base_dir, transport.base_dir);
    assert_eq!(runtime.socket_mode, transport.socket_mode);
    assert_eq!(runtime.base_dir_mode, transport.base_dir_mode);
    assert_eq!(runtime.max_datagram, transport.max_datagram);
}

/// zerodds-uds-transport-1.0 §3.5, §6.2: the runtime's default mode is the
/// one transport the platform has for its defaults: datagrams on Unix, the
/// stream on Windows.
#[test]
fn uds_6_2_the_runtimes_default_mode_is_the_platforms() {
    use zerodds_dcps::runtime::{DEFAULT_UDS_MODE, UdsMode, UdsUserConfig};
    assert_eq!(UdsUserConfig::default().mode, DEFAULT_UDS_MODE);
    if cfg!(windows) {
        assert_eq!(DEFAULT_UDS_MODE, UdsMode::Stream);
    } else {
        assert_eq!(DEFAULT_UDS_MODE, UdsMode::Datagram);
    }
}

#[test]
fn the_transport_goes_behind_the_shared_transport_trait() {
    fn behind_dyn<T: Transport + Send + Sync + 'static>() {}
    #[cfg(unix)]
    behind_dyn::<UdsTransport>();
    behind_dyn::<UdsStreamTransport>();
}

/// Starts a participant whose UDS user transport binds in `mode` under
/// `base_dir`, which must fail, and returns the error of the start and the
/// events `transport.uds.bind_failed` of the start.
fn refused_start(mode: UdsMode, base_dir: &Path) -> (DdsError, Vec<Event>) {
    let lease = zerodds_test_domains::lease_domain();
    let sink = Arc::new(VecSink::new());
    let mut cfg = RuntimeConfig {
        user_transport: Some(UserTransportKind::Uds),
        user_multicast: false,
        same_host_shm: false,
        uds: UdsUserConfig {
            base_dir: base_dir.to_path_buf(),
            mode,
            ..UdsUserConfig::default()
        },
        ..RuntimeConfig::default()
    };
    cfg.observability = sink.clone();
    let started = DomainParticipantFactory::instance().create_participant_with_config(
        lease.id(),
        DomainParticipantQos::default(),
        cfg,
    );
    let error = match started {
        Ok(_) => panic!("a participant started with the UDS bind refused ({mode:?})"),
        Err(error) => error,
    };
    let reported = sink
        .snapshot()
        .into_iter()
        .filter(|e| e.name == "transport.uds.bind_failed")
        .collect();
    (error, reported)
}

/// The value of attribute `key` of the one event in `reported`.
fn attr(reported: &[Event], key: &str) -> String {
    assert_eq!(reported.len(), 1, "{reported:?}");
    assert_eq!(reported[0].level, Level::Error);
    reported[0]
        .attrs
        .iter()
        .find(|a| a.key == key)
        .map(|a| a.value.clone())
        .unwrap_or_else(|| panic!("no attribute {key}: {reported:?}"))
}

/// zerodds-uds-transport-1.0 §3.5, §6.2: a stream bind the transport
/// refuses (`base_dir` is a file) fails the start with the transport error
/// of the UDS bind and one event `transport.uds.bind_failed` that names the
/// directory, the mode `stream` and the transport's error. On every
/// platform: on Windows it is the default mode.
#[test]
fn uds_3_5_a_refused_stream_bind_is_reported_with_its_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let base_dir = tmp.path().join("not-a-directory");
    std::fs::write(&base_dir, b"a file").unwrap();
    let (error, reported) = refused_start(UdsMode::Stream, &base_dir);
    assert_eq!(
        error,
        DdsError::TransportError {
            label: "user unicast bind (UDS)"
        }
    );
    assert_eq!(attr(&reported, "mode"), "stream");
    assert_eq!(attr(&reported, "base_dir"), base_dir.display().to_string());
    assert_eq!(
        attr(&reported, "error"),
        "uds base_dir exists but is not a directory"
    );
}

/// zerodds-uds-transport-1.0 §6.2: Windows has no `SOCK_DGRAM` for
/// `AF_UNIX`; the mode `datagram` fails the start with the transport error
/// of the UDS bind and one event `transport.uds.bind_failed` with the mode
/// `datagram` and the reason. Nothing is created in `base_dir`.
#[cfg(windows)]
#[test]
fn uds_6_2_the_mode_datagram_fails_the_start_on_windows() {
    let tmp = tempfile::tempdir().unwrap();
    let base_dir = tmp.path().join("uds");
    let (error, reported) = refused_start(UdsMode::Datagram, &base_dir);
    assert_eq!(
        error,
        DdsError::TransportError {
            label: "user unicast bind (UDS)"
        }
    );
    assert_eq!(attr(&reported, "mode"), "datagram");
    assert_eq!(
        attr(&reported, "error"),
        "uds mode datagram: Windows has no SOCK_DGRAM for AF_UNIX; transport.uds.mode stream"
    );
    assert!(!base_dir.exists(), "{base_dir:?}");
}
