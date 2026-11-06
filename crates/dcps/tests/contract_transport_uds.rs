// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps` with `zerodds-transport-uds`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-transport-uds`, with the feature
//! `same-host-uds`, in `runtime.rs` (`select_user_transport`,
//! `transport.user_transport = UDS`): `UdsConfig { recv_timeout:
//! Some(1 s), ..UdsConfig::default() }`, `UdsTransport::bind(local_id,
//! config)` with `local_id` = the 12 octets of the GuidPrefix followed by
//! the domain id as a big-endian `u32`, and the transport behind
//! `Arc<dyn Transport + Send + Sync>`. It relies on a peer that reads that
//! id from the announced UDS locator resolving it to the socket path this
//! participant bound, under the same default base directory.
//!
//! Oracles: ZeroDDS UDS Transport 1.0 §2.1 (`<base_dir>/<lowercase
//! hex32>.sock`, default `base_dir` `/tmp/zerodds/uds`) and §3.3 (default
//! `max_datagram` 65 536), `docs/spec-coverage/zerodds-uds-transport-1.0.md`;
//! DDSI-RTPS 2.5 §9.3.1.5 for the prefix layout. No socket is bound and no
//! directory is created.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use zerodds_transport::Transport;
use zerodds_transport_uds::{DEFAULT_BASE_DIR, UdsConfig, UdsTransport, socket_path};

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
}

#[test]
fn the_transport_goes_behind_the_shared_transport_trait() {
    fn behind_dyn<T: Transport + Send + Sync + 'static>() {}
    behind_dyn::<UdsTransport>();
}
