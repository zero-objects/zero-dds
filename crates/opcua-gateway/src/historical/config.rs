// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Historical-Variable-Configuration — Spec §9.3.4.4 Variable-
//! Attribute-Setzungen.
//!
//! Spec normativ:
//! * `Historizing` Attribut = `true`.
//! * `AccessLevel` muss das `HistoryRead`-Bit enthalten.
//! * Optional: `HasHistoricalConfiguration`-Reference auf eine
//!   HA-Configuration-Node, die fuer alle Variables des selben
//!   Topics konsistent sein muss.

use alloc::string::String;

/// `HistoryRead`-Bit aus dem AccessLevel-Bitfield (OPCUA-03 §5.6.1
/// Tab 8 — `HistoryRead = 0x04`).
pub const HISTORY_READ_BIT: u8 = 0x04;

/// `CurrentRead`-Bit (`0x01`) — wird parallel mit HistoryRead gesetzt,
/// damit Standard-Read und HistoryRead beide funktionieren.
pub const CURRENT_READ_BIT: u8 = 0x01;

/// AccessLevel-Wrapper fuer das OPC-UA `AccessLevel`-Attribut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AccessLevel(pub u8);

impl AccessLevel {
    /// Default-AccessLevel fuer ein Historical-Variable: CurrentRead +
    /// HistoryRead. Spec §9.3.4.4 zwingend.
    #[must_use]
    pub const fn historical_default() -> Self {
        Self(CURRENT_READ_BIT | HISTORY_READ_BIT)
    }

    /// `true` wenn das HistoryRead-Bit gesetzt ist (Spec §9.3.4.4).
    #[must_use]
    pub const fn allows_history_read(self) -> bool {
        (self.0 & HISTORY_READ_BIT) != 0
    }

    /// `true` wenn das CurrentRead-Bit gesetzt ist.
    #[must_use]
    pub const fn allows_current_read(self) -> bool {
        (self.0 & CURRENT_READ_BIT) != 0
    }
}

/// Symbolische Reference zu einer HA-Configuration-Node (Spec
/// §9.3.4.4 optional).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HistoricalConfigRef {
    /// BrowseName der HA-Configuration-Node (z.B. "HA Configuration").
    pub browse_name: String,
}

/// Pro-Variable-Konfiguration fuer Historical Data Reading. Wird vom
/// Caller aus dem Walker-NodeSpec (Spec §9.2) angereichert: das
/// Walker-Modul liefert die Variable; dieses Modul liefert die
/// `Historizing`/`AccessLevel`/`HasHistoricalConfiguration`-
/// Decorations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalNodeConfig {
    /// Spec §9.3.4.4: `Historizing = true`.
    pub historizing: bool,
    /// Spec §9.3.4.4: AccessLevel mit HistoryRead-Bit.
    pub access_level: AccessLevel,
    /// Spec §9.3.4.4 optional: HasHistoricalConfiguration-Reference.
    pub historical_config: Option<HistoricalConfigRef>,
}

impl Default for HistoricalNodeConfig {
    fn default() -> Self {
        Self {
            historizing: true,
            access_level: AccessLevel::historical_default(),
            historical_config: None,
        }
    }
}

impl HistoricalNodeConfig {
    /// Spec-Konformitaet: liefert `Ok(())` wenn `Historizing == true`
    /// UND HistoryRead im AccessLevel gesetzt ist.
    ///
    /// # Errors
    /// `HistoricalConfigError` mit dem fehlenden Spec-Aspekt.
    pub fn validate(&self) -> Result<(), HistoricalConfigError> {
        if !self.historizing {
            return Err(HistoricalConfigError::HistorizingNotSet);
        }
        if !self.access_level.allows_history_read() {
            return Err(HistoricalConfigError::HistoryReadBitMissing);
        }
        Ok(())
    }
}

/// Spec-Konformitaets-Fehler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoricalConfigError {
    /// `Historizing == false` — Spec §9.3.4.4 verlangt `true`.
    HistorizingNotSet,
    /// `AccessLevel` enthaelt das `HistoryRead`-Bit nicht.
    HistoryReadBitMissing,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn historical_default_has_history_and_current_read_bits() {
        let al = AccessLevel::historical_default();
        assert!(al.allows_history_read());
        assert!(al.allows_current_read());
    }

    #[test]
    fn default_config_validates() {
        let cfg = HistoricalNodeConfig::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn historizing_false_fails_validation() {
        let cfg = HistoricalNodeConfig {
            historizing: false,
            ..HistoricalNodeConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(HistoricalConfigError::HistorizingNotSet)
        );
    }

    #[test]
    fn missing_history_read_bit_fails_validation() {
        let cfg = HistoricalNodeConfig {
            access_level: AccessLevel(CURRENT_READ_BIT), // no HistoryRead
            ..HistoricalNodeConfig::default()
        };
        assert_eq!(
            cfg.validate(),
            Err(HistoricalConfigError::HistoryReadBitMissing)
        );
    }

    #[test]
    fn history_read_bit_value_matches_spec() {
        // Spec OPCUA-03 §5.6.1: HistoryRead = bit 2 (= 0x04).
        assert_eq!(HISTORY_READ_BIT, 0x04);
    }
}
