// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A participant created with an explicit `RuntimeConfig` announces the
//! USER_DATA of its QoS (DDS 1.4 §2.2.3.1).
//!
//! `create_participant_with_config` and the builder with `with_config` once
//! took the config as given and announced an empty PID_USER_DATA while the
//! QoS said otherwise. Programs that load their configuration from the
//! command line go through exactly this path.
//!
//! macOS: ignored — multicast loopback is unreliable there, as for the other
//! same-host SPDP e2e tests; runs on Linux.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::time::{Duration, Instant};

use zerodds_dcps::DomainParticipantFactory;
use zerodds_dcps::qos::DomainParticipantQos;
use zerodds_dcps::runtime::RuntimeConfig;

#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn an_explicit_config_keeps_the_user_data_of_the_qos() {
    let domain = 173;
    let mut qos = DomainParticipantQos::default();
    qos.user_data.value = b"cli-config".to_vec();
    let announcer = DomainParticipantFactory::create(domain)
        .with_qos(qos)
        .with_config(RuntimeConfig::default())
        .build()
        .expect("announcer");
    let observer = DomainParticipantFactory::instance()
        .create_participant_with_config(
            domain,
            DomainParticipantQos::default(),
            RuntimeConfig::default(),
        )
        .expect("observer");

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let seen = observer.get_discovered_participants().into_iter().any(|h| {
            observer
                .get_discovered_participant_data(h)
                .is_ok_and(|d| d.user_data == b"cli-config")
        });
        if seen {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "no discovered participant announced the QoS user_data"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(announcer);
}
