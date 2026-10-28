// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A build without DDS-Security cannot load `security.enclave_dir`; a
//! configuration that names one fails the participant create instead of
//! starting it plaintext.

#![cfg(not(feature = "security"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use zerodds_dcps::factory::DomainParticipantFactory;
use zerodds_dcps::qos::DomainParticipantQos;
use zerodds_dcps::runtime::RuntimeConfig;

#[test]
fn an_enclave_without_the_security_feature_is_refused() {
    let cfg = RuntimeConfig {
        security_enclave_dir: Some("/nonexistent/enclave".into()),
        ..RuntimeConfig::default()
    };
    let err = DomainParticipantFactory::instance()
        .create_participant_with_config(
            zerodds_test_domains::unique_domain(),
            DomainParticipantQos::default(),
            cfg,
        )
        .expect_err("refused");
    assert!(err.to_string().contains("no DDS-Security"), "{err}");
}
