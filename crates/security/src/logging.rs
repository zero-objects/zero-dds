// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Logging plugin SPI (OMG DDS-Security 1.1 §8.6).
//!
//! Separate plugin slot for security events — important for audits,
//! pen tests, forensics. Separated from general application logging
//! so that security-critical events do not accidentally get lost under
//! the debug flag.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (The plugin SPI needs `Box<dyn LoggingPlugin>`.)

extern crate alloc;

use alloc::boxed::Box;

/// Severity of a security event (spec §8.6.3 table 36).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LogLevel {
    /// Security emergency (auth handshake failed, key-exchange break).
    Emergency = 0,
    /// Alert.
    Alert = 1,
    /// Critical.
    Critical = 2,
    /// Error.
    Error = 3,
    /// Warning.
    Warning = 4,
    /// Notice.
    Notice = 5,
    /// Informational.
    Informational = 6,
    /// Debug.
    Debug = 7,
}

/// Logging plugin (spec §8.6.2.1).
pub trait LoggingPlugin: Send + Sync {
    /// Log a security event.
    ///
    /// Spec §8.6.2.1.1 `log`. `participant` identifies the affected
    /// participant (GUID bytes, 16 octets). `category` is a
    /// plugin-specific string ("auth.handshake.failed" etc.).
    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str);

    /// Plugin class id (e.g. "DDS:Logging:DDS_LogTopic" for the
    /// spec-intended LogTopic dispatch).
    fn plugin_class_id(&self) -> &str;
}

/// Factory alias.
pub type LoggingPluginBox = Box<dyn LoggingPlugin>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_levels_order_correctly() {
        assert!(LogLevel::Emergency < LogLevel::Warning);
        assert!(LogLevel::Debug > LogLevel::Error);
    }
}
