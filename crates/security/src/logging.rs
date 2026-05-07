// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Logging-Plugin SPI (OMG DDS-Security 1.1 §8.6).
//!
//! Separater Plugin-Slot fuer Security-Events — wichtig fuer Audits,
//! Pen-Tests, Forensik. Getrennt vom allgemeinen Application-Logging
//! damit sicherheitskritische Events nicht versehentlich im Debug-Flag
//! untergehen.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (Plugin-SPI benötigt `Box<dyn LoggingPlugin>`.)

extern crate alloc;

use alloc::boxed::Box;

/// Severity eines Security-Events (Spec §8.6.3 Tabelle 36).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LogLevel {
    /// Security-Emergency (Auth-Handshake failed, Key-Exchange-Bruch).
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

/// Logging-Plugin (Spec §8.6.2.1).
pub trait LoggingPlugin: Send + Sync {
    /// Ein Security-Event loggen.
    ///
    /// Spec §8.6.2.1.1 `log`. `participant` identifiziert den betroffenen
    /// Teilnehmer (GUID-bytes, 16 octets). `category` ist ein
    /// plugin-spezifischer String ("auth.handshake.failed" etc.).
    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str);

    /// Plugin-Class-Id (z.B. "DDS:Logging:DDS_LogTopic" fuer den
    /// Spec-vorgesehenen LogTopic-Dispatch).
    fn plugin_class_id(&self) -> &str;
}

/// Factory-Alias.
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
