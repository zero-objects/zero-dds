// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-security-main` with `zerodds-security-runtime`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security-runtime`, from its non-test
//! sources:
//! - `security.rs`: the property names `PROP_IDENTITY_CA`,
//!   `PROP_IDENTITY_CERT`, `PROP_IDENTITY_KEY`, `PROP_PERMISSIONS_CA`,
//!   `PROP_GOVERNANCE`, `PROP_PERMISSIONS`, `PROP_KEY_ESTABLISHMENT` and
//!   `PROP_ACCEPT_X509V1_IDENTITY`, re-exported and written onto the
//!   `PropertyQosPolicy` of the participant, so that the client spells them
//!   as the runtime reads them.
//! - `main.rs`: `SecurityBundle::builder().logging_plugin(..).build()` for
//!   the `-logging` sink, handed to the participant factory, and the
//!   re-exports `LogLevel` and `LoggingPlugin` the sink is written against.
//!
//! What it relies on: the six file properties are the names of the
//! specification, the X.509 v1 deviation is a name outside the `dds.sec.`
//! namespace, a bundle built with a logger alone carries that logger and no
//! profile, and the re-exported `LogLevel` and `LoggingPlugin` are those of
//! `zerodds-security`.
//!
//! Oracles: DDS-Security 1.2 §10.3.1 Table 50 (`dds.sec.auth.identity_ca`,
//! `dds.sec.auth.identity_certificate`, `dds.sec.auth.private_key`),
//! §10.4.1 Table 63 (`dds.sec.access.permissions_ca`,
//! `dds.sec.access.governance`, `dds.sec.access.permissions`), §7.3.2 (the
//! `dds.sec.` prefix is reserved for the names of the specification), and
//! the rustdoc of `SecurityBundle`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use zerodds_security::error::SecurityError;
use zerodds_security::logging::{LogOptions, LoggerListener};
use zerodds_security_runtime::{
    LogLevel, LoggingPlugin, PROP_ACCEPT_X509V1_IDENTITY, PROP_GOVERNANCE, PROP_IDENTITY_CA,
    PROP_IDENTITY_CERT, PROP_IDENTITY_KEY, PROP_KEY_ESTABLISHMENT, PROP_PERMISSIONS,
    PROP_PERMISSIONS_CA, SecurityBundle,
};

#[test]
fn security_10_3_1_the_file_properties_are_the_names_of_tables_50_and_63() {
    assert_eq!(PROP_IDENTITY_CA, "dds.sec.auth.identity_ca");
    assert_eq!(PROP_IDENTITY_CERT, "dds.sec.auth.identity_certificate");
    assert_eq!(PROP_IDENTITY_KEY, "dds.sec.auth.private_key");
    assert_eq!(PROP_PERMISSIONS_CA, "dds.sec.access.permissions_ca");
    assert_eq!(PROP_GOVERNANCE, "dds.sec.access.governance");
    assert_eq!(PROP_PERMISSIONS, "dds.sec.access.permissions");
    assert!(PROP_KEY_ESTABLISHMENT.starts_with("dds.sec.auth."));
}

#[test]
fn security_7_3_2_the_x509v1_deviation_is_named_outside_the_reserved_prefix() {
    assert!(!PROP_ACCEPT_X509V1_IDENTITY.starts_with("dds.sec."));
    assert!(PROP_ACCEPT_X509V1_IDENTITY.starts_with("org.zerodds."));
    // The client writes it next to the spec properties; the names differ.
    for name in [
        PROP_IDENTITY_CA,
        PROP_IDENTITY_CERT,
        PROP_IDENTITY_KEY,
        PROP_PERMISSIONS_CA,
        PROP_GOVERNANCE,
        PROP_PERMISSIONS,
        PROP_KEY_ESTABLISHMENT,
    ] {
        assert_ne!(name, PROP_ACCEPT_X509V1_IDENTITY);
    }
}

#[test]
fn the_reexported_logging_items_are_those_of_the_security_crate() {
    use core::any::TypeId;
    assert_eq!(
        TypeId::of::<LogLevel>(),
        TypeId::of::<zerodds_security::logging::LogLevel>()
    );
    assert_eq!(
        TypeId::of::<dyn LoggingPlugin>(),
        TypeId::of::<dyn zerodds_security::logging::LoggingPlugin>()
    );
}

/// A sink that counts the events it is given.
#[derive(Default)]
struct Counting(Mutex<Vec<(LogLevel, String)>>);

impl LoggingPlugin for Counting {
    fn set_log_options(&self, _options: LogOptions) -> Result<(), SecurityError> {
        Ok(())
    }

    fn enable_logging(&self) -> Result<(), SecurityError> {
        Ok(())
    }

    fn set_listener(
        &self,
        _listener: Option<Arc<dyn LoggerListener>>,
    ) -> Result<(), SecurityError> {
        Ok(())
    }

    fn log(&self, level: LogLevel, _participant: [u8; 16], category: &str, _message: &str) {
        self.0.lock().unwrap().push((level, category.to_string()));
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Logging:DDS_LogTopic"
    }
}

/// Hands the events on to a shared `Counting`, so that the test can look at
/// them after the bundle took the plugin.
struct Forward(Arc<Counting>);

impl LoggingPlugin for Forward {
    fn set_log_options(&self, options: LogOptions) -> Result<(), SecurityError> {
        self.0.set_log_options(options)
    }

    fn enable_logging(&self) -> Result<(), SecurityError> {
        self.0.enable_logging()
    }

    fn set_listener(&self, listener: Option<Arc<dyn LoggerListener>>) -> Result<(), SecurityError> {
        self.0.set_listener(listener)
    }

    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str) {
        self.0.log(level, participant, category, message);
    }

    fn plugin_class_id(&self) -> &str {
        self.0.plugin_class_id()
    }
}

#[test]
fn a_bundle_with_a_logger_alone_carries_that_logger_and_no_profile() {
    let counting = Arc::new(Counting::default());
    let bundle = SecurityBundle::builder()
        .logging_plugin(Box::new(Forward(Arc::clone(&counting))))
        .build();
    assert!(bundle.has_logging());
    assert!(
        !bundle.has_profile(),
        "the property path builds the profile"
    );
    assert!(bundle.security_profile().is_none());
    assert!(!bundle.has_data_tagging());

    let logger = bundle.logging_plugin().expect("the logger of the builder");
    assert_eq!(logger.plugin_class_id(), "DDS:Logging:DDS_LogTopic");
    logger.log(LogLevel::Error, [1; 16], "auth.handshake.failed", "m");
    // A clone of the bundle shares the same logger.
    let copy = bundle.clone();
    copy.logging_plugin()
        .unwrap()
        .log(LogLevel::Notice, [1; 16], "auth.handshake", "m");
    assert!(copy.has_logging() && bundle.has_logging());
    assert_eq!(
        *counting.0.lock().unwrap(),
        [
            (LogLevel::Error, "auth.handshake.failed".to_string()),
            (LogLevel::Notice, "auth.handshake".to_string()),
        ]
    );
}

#[test]
fn a_bundle_built_without_a_logger_has_none() {
    let bundle = SecurityBundle::builder().build();
    assert!(!bundle.has_logging());
    assert!(bundle.logging_plugin().is_none());
    assert!(!bundle.has_profile());
}
