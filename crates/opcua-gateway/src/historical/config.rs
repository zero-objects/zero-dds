// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Historical variable configuration — Spec §9.3.4.4 variable
//! attribute settings.
//!
//! Spec-normative:
//! * `Historizing` attribute = `true`.
//! * `AccessLevel` must contain the `HistoryRead` bit.
//! * Optional: `HasHistoricalConfiguration` reference to an
//!   HA configuration node that must be consistent for all variables
//!   of the same topic.

use alloc::string::String;

/// `HistoryRead` bit from the AccessLevel bitfield (OPCUA-03 §5.6.1
/// Tab 8 — `HistoryRead = 0x04`).
pub const HISTORY_READ_BIT: u8 = 0x04;

/// `CurrentRead` bit (`0x01`) — set in parallel with HistoryRead,
/// so that standard read and HistoryRead both work.
pub const CURRENT_READ_BIT: u8 = 0x01;

/// AccessLevel wrapper for the OPC-UA `AccessLevel` attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AccessLevel(pub u8);

impl AccessLevel {
    /// Default AccessLevel for a historical variable: CurrentRead +
    /// HistoryRead. Mandatory per Spec §9.3.4.4.
    #[must_use]
    pub const fn historical_default() -> Self {
        Self(CURRENT_READ_BIT | HISTORY_READ_BIT)
    }

    /// `true` if the HistoryRead bit is set (Spec §9.3.4.4).
    #[must_use]
    pub const fn allows_history_read(self) -> bool {
        (self.0 & HISTORY_READ_BIT) != 0
    }

    /// `true` if the CurrentRead bit is set.
    #[must_use]
    pub const fn allows_current_read(self) -> bool {
        (self.0 & CURRENT_READ_BIT) != 0
    }
}

/// Symbolic reference to an HA configuration node (Spec
/// §9.3.4.4 optional).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HistoricalConfigRef {
    /// BrowseName of the HA configuration node (e.g. "HA Configuration").
    pub browse_name: String,
}

/// Per-variable configuration for historical data reading. Enriched by
/// the caller from the walker NodeSpec (Spec §9.2): the
/// walker module provides the variable; this module provides the
/// `Historizing`/`AccessLevel`/`HasHistoricalConfiguration`
/// decorations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalNodeConfig {
    /// Spec §9.3.4.4: `Historizing = true`.
    pub historizing: bool,
    /// Spec §9.3.4.4: AccessLevel with HistoryRead bit.
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
    /// Spec conformance: returns `Ok(())` if `Historizing == true`
    /// AND HistoryRead is set in the AccessLevel.
    ///
    /// # Errors
    /// `HistoricalConfigError` with the missing spec aspect.
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

/// Spec conformance error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoricalConfigError {
    /// `Historizing == false` — Spec §9.3.4.4 requires `true`.
    HistorizingNotSet,
    /// `AccessLevel` does not contain the `HistoryRead` bit.
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
