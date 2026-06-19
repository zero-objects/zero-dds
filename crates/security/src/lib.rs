// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security`. Safety classification: **SAFE** (the
//! security plugins run against production trust boundaries; the SPI
//! layer itself is trust-neutral).
//!
//! DDS-Security 1.1 (formal/2018-04-01) plugin SPI: defines the
//! abstract plugin traits + data types + generic-message topics;
//! production implementations live in sister crates.
//!
//! ## Layer position
//!
//! Layer 4 — Core Services (SPI crate). Pure Rust + `alloc`, **no**
//! ZeroDDS crate deps.
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! | Spec                  | Trait / module                                      | Concrete impl |
//! |-----------------------|-----------------------------------------------------|---------------|
//! | §8.3 Authentication   | [`AuthenticationPlugin`] in [`authentication`]      | `zerodds-security-pki` (X.509 + RSA-PSS + ECDSA + OCSP/CRL) |
//! | §8.4 Access Control   | [`AccessControlPlugin`] in [`access_control`]       | `zerodds-security-permissions` (Governance + Permissions XML) |
//! | §8.5 Cryptographic    | [`CryptographicPlugin`] in [`crypto`]               | `zerodds-security-crypto` (AES-GCM 128/256 + HMAC-SHA256 + receiver-specific MACs) |
//! | §8.6 Logging          | [`LoggingPlugin`] in [`logging`]                    | `zerodds-security-logging` |
//! | §8.7 Data Tagging     | [`DataTaggingPlugin`] in [`data_tagging`]           | `zerodds-security-runtime` (built-in DataTagging) |
//!
//! Plus cross-cutting:
//! - [`token`] — `IdentityToken`, `PermissionsToken`, `CryptoToken`, `DataHolder`, `BinaryProperty`.
//! - [`generic_message`] — `ParticipantGenericMessage`, `MessageIdentity` + topic constants for DCPSParticipantStatelessMessage / DCPSParticipantVolatileMessageSecure.
//! - [`properties`] — `Property` / `PropertyList` for plugin configuration.
//! - [`security_topic_qos`] — built-in security-topic QoS profiles.
//! - [`error`] — `SecurityError`.
//! - [`mock`] (feature `std`) — test mock plugins, never in production.
//!
//! ## Architecture
//!
//! The SPI is trait-based + `Box<dyn Plugin>`-erasable, so that
//! different backends (rustls vs. ring vs. mbedtls) are interchangeable
//! without crate wiring. Each plugin trait is self-contained
//! — no cross-references — so that extensions in one plugin do not
//! break others.
//!
//! ## API stability pledge
//!
//! This interface is **API-frozen** as of `1.0.0-rc.1`. Breaking
//! changes require a v2.0 major bump. Semver patch + minor may
//! only add new methods with a default body or non-breaking enum
//! variants.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

// zerodds-lint: allow no_dyn_in_safe
// The plugin SPI needs `Box<dyn Plugin>` for interchangeable backends
// (rustls/ring/mbedtls). This is architectural and not a memory-safety
// weakness.

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod access_control;
pub mod authentication;
pub mod crypto;
pub mod data_tagging;
pub mod error;
pub mod generic_message;
pub mod logging;
pub mod properties;
pub mod security_topic_qos;
pub mod token;

#[cfg(feature = "std")]
pub mod mock;

pub use access_control::AccessControlPlugin;
pub use authentication::AuthenticationPlugin;
pub use crypto::CryptographicPlugin;
pub use data_tagging::DataTaggingPlugin;
pub use error::SecurityError;
pub use generic_message::{
    MessageIdentity, ParticipantGenericMessage, TOPIC_STATELESS_MESSAGE,
    TOPIC_VOLATILE_MESSAGE_SECURE, TYPE_NAME_GENERIC_MESSAGE,
};
pub use logging::{LogLevel, LoggingPlugin};
pub use properties::{Property, PropertyList};
pub use token::{
    BinaryProperty, CryptoToken, DataHolder, IdentityStatusToken, IdentityToken, PermissionsToken,
    WireProperty,
};

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    #[test]
    fn plugin_trait_objects_are_object_safe() {
        // Smoke test: every plugin trait is object-safe (`dyn Plugin`
        // constructible). Fails at compile time if someone accidentally
        // adds `Self: Sized` or generic methods.
        fn _assert_object_safe<T: ?Sized>() {}
        _assert_object_safe::<dyn super::AuthenticationPlugin>();
        _assert_object_safe::<dyn super::AccessControlPlugin>();
        _assert_object_safe::<dyn super::CryptographicPlugin>();
        _assert_object_safe::<dyn super::LoggingPlugin>();
        _assert_object_safe::<dyn super::DataTaggingPlugin>();
    }
}
