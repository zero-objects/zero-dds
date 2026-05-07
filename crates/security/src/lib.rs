// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security`. Safety classification: **SAFE** (die
//! Security-Plugins werden gegen Produktions-Vertrauensgrenzen
//! ausgefuehrt; der SPI-Layer selbst ist trust-neutral).
//!
//! DDS-Security 1.1 (formal/2018-04-01) Plugin-SPI: definiert die
//! abstrakten Plugin-Traits + Datentypen + Generic-Message-Topics;
//! Produktions-Implementationen leben in Schwester-Crates.
//!
//! ## Schichten-Position
//!
//! Layer 4 — Core Services (SPI-Crate). Pure-Rust + `alloc`, **keine**
//! ZeroDDS-Crate-Deps.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! | Spec                  | Trait / Modul                                       | Konkrete Impl |
//! |-----------------------|-----------------------------------------------------|---------------|
//! | §8.3 Authentication   | [`AuthenticationPlugin`] in [`authentication`]      | `zerodds-security-pki` (X.509 + RSA-PSS + ECDSA + OCSP/CRL) |
//! | §8.4 Access Control   | [`AccessControlPlugin`] in [`access_control`]       | `zerodds-security-permissions` (Governance + Permissions-XML) |
//! | §8.5 Cryptographic    | [`CryptographicPlugin`] in [`crypto`]               | `zerodds-security-crypto` (AES-GCM 128/256 + HMAC-SHA256 + Receiver-Specific-MACs) |
//! | §8.6 Logging          | [`LoggingPlugin`] in [`logging`]                    | `zerodds-security-logging` |
//! | §8.7 Data Tagging     | [`DataTaggingPlugin`] in [`data_tagging`]           | `zerodds-security-runtime` (Built-in DataTagging) |
//!
//! Plus Querschnitt:
//! - [`token`] — `IdentityToken`, `PermissionsToken`, `CryptoToken`, `DataHolder`, `BinaryProperty`.
//! - [`generic_message`] — `ParticipantGenericMessage`, `MessageIdentity` + Topic-Konstanten fuer DCPSParticipantStatelessMessage / DCPSParticipantVolatileMessageSecure.
//! - [`properties`] — `Property` / `PropertyList` fuer Plugin-Konfiguration.
//! - [`security_topic_qos`] — Built-in-Security-Topic-QoS-Profile.
//! - [`error`] — `SecurityError`.
//! - [`mock`] (Feature `std`) — Test-Mock-Plugins, niemals produktiv.
//!
//! ## Architektur
//!
//! Das SPI ist Trait-basiert + `Box<dyn Plugin>`-erasable, damit
//! verschiedene Backends (rustls vs. ring vs. mbedtls) ohne Crate-
//! Wiring austauschbar sind. Jeder Plugin-Trait ist in sich geschlossen
//! — keine Cross-References — damit Erweiterungen in einem Plugin nicht
//! andere brechen.
//!
//! ## API-Stability-Pledge
//!
//! Dieses Interface ist **API-frozen** ab `1.0.0-rc.1`. Breaking
//! Changes erfordern ein v2.0-Major-Bump. Semver-Patch + Minor duerfen
//! nur neue Methoden mit Default-Body oder non-breaking Enum-Varianten
//! hinzufuegen.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

// zerodds-lint: allow no_dyn_in_safe
// Plugin-SPI benötigt `Box<dyn Plugin>` für austauschbare Backends
// (rustls/ring/mbedtls). Dies ist architektur-bedingt und keine Speicher-
// Sicherheits-Schwäche.

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
        // Smoketest: jeder Plugin-Trait ist object-safe (`dyn Plugin`
        // konstruierbar). Faellt an Compile-Zeit wenn jemand versehentlich
        // `Self: Sized` oder generische Methoden einfuegt.
        fn _assert_object_safe<T: ?Sized>() {}
        _assert_object_safe::<dyn super::AuthenticationPlugin>();
        _assert_object_safe::<dyn super::AccessControlPlugin>();
        _assert_object_safe::<dyn super::CryptographicPlugin>();
        _assert_object_safe::<dyn super::LoggingPlugin>();
        _assert_object_safe::<dyn super::DataTaggingPlugin>();
    }
}
