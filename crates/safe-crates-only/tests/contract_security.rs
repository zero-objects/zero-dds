// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `safe-crates-only` with `zerodds-security`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security` (`src/lib.rs`, `Cargo.toml`):
//! it re-exports the crate as `safe_crates_only::zerodds_security`, depends
//! on it with `default-features = false` and, with its `safety` feature,
//! forwards `zerodds-security/safety` and `zerodds-security/alloc` for the
//! no_std safe-profile build (the target needs `safety`, see `Cargo.toml`).
//! No item is called; what the crate relies on is that the re-export is the
//! security crate and that its plugin SPI and token data model are there
//! without `std`.
//!
//! Oracles: DDS-Security 1.2 §7.5.3 and §7.5.4 (the topic names
//! `DCPSParticipantStatelessMessage` and
//! `DCPSParticipantVolatileMessageSecure`, both of type
//! `ParticipantGenericMessage`), and the `message_class_id` values
//! `dds.sec.auth_request` and `dds.sec.auth` of the stateless handshake.

use safe_crates_only::zerodds_security as reexport;

#[test]
fn the_reexport_is_the_security_crate() {
    use core::any::TypeId;
    assert_eq!(
        TypeId::of::<reexport::token::DataHolder>(),
        TypeId::of::<zerodds_security::token::DataHolder>()
    );
    assert_eq!(
        TypeId::of::<reexport::generic_message::ParticipantGenericMessage>(),
        TypeId::of::<zerodds_security::generic_message::ParticipantGenericMessage>()
    );
    assert_eq!(
        TypeId::of::<reexport::properties::PropertyList>(),
        TypeId::of::<zerodds_security::properties::PropertyList>()
    );
    assert_eq!(
        TypeId::of::<reexport::error::SecurityError>(),
        TypeId::of::<zerodds_security::error::SecurityError>()
    );
}

#[test]
fn security_7_5_3_the_no_std_surface_names_the_builtin_message_topics() {
    use reexport::generic_message::{
        TOPIC_STATELESS_MESSAGE, TOPIC_VOLATILE_MESSAGE_SECURE, TYPE_NAME_GENERIC_MESSAGE, class_id,
    };
    assert_eq!(TOPIC_STATELESS_MESSAGE, "DCPSParticipantStatelessMessage");
    assert_eq!(
        TOPIC_VOLATILE_MESSAGE_SECURE,
        "DCPSParticipantVolatileMessageSecure"
    );
    assert_eq!(TYPE_NAME_GENERIC_MESSAGE, "ParticipantGenericMessage");
    assert_eq!(class_id::AUTH_REQUEST, "dds.sec.auth_request");
    assert_eq!(class_id::AUTH, "dds.sec.auth");
    // The token data model builds without `std`.
    let token = reexport::token::DataHolder::new("DDS:Auth:PKI-DH:1.2")
        .with_property("dds.cert.sn", "CN=a");
    assert_eq!(token.property("dds.cert.sn"), Some("CN=a"));
}
