// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! `transport.same_host_shm` (env `ZERODDS_SAME_HOST_SHM`): on by default
//! where the `same-host-shm` feature is built in, off where it is not, and
//! a configuration file turns it off. What the runtime does with it is
//! `same_host_e2e::e2e_same_host_shm_off_takes_udp`.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use zerodds_dcps::runtime::{ConfigSource, RuntimeConfig};

#[test]
fn same_host_shm_is_a_setting_and_defaults_to_the_feature() {
    assert_eq!(
        RuntimeConfig::default().same_host_shm,
        cfg!(feature = "same-host-shm")
    );
    let dir = std::env::temp_dir().join(format!("zerodds-same-host-shm-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("workdir");
    let base = dir.join("zerodds.yaml");
    std::fs::write(&base, "transport:\n  same_host_shm: false\n").expect("base file");
    let files = zerodds_config::Loader::new()
        .base(&base)
        .load()
        .expect("the file loads");
    let config =
        RuntimeConfig::from_source(&ConfigSource::with_files(files)).expect("typed file value");
    assert!(!config.same_host_shm);
    let _ = std::fs::remove_dir_all(&dir);
}
