// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! HISTORY-QoS-Validation — Spec §9.3.4.4.
//!
//! Spec normativ: "the DataReader embedded in the Gateway to handle
//! subscription to the DDS Topic shall be configured to support
//! historical access. In particular, their HISTORY QoS Policy shall
//! be configured either as KEEP_ALL or KEEP_LAST with a HISTORY_DEPTH
//! big enough to store the desired time span of samples."

/// HISTORY-QoS-Kind aus DDS PSM (Spec OMG DDS 1.4 §2.2.3.18).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryQosKind {
    /// `KEEP_LAST` — nur die letzten N Samples pro Instance.
    KeepLast,
    /// `KEEP_ALL` — alle Samples bis Resource-Limits zwingen.
    KeepAll,
}

/// Validator fuer DataReader-HISTORY-QoS gegen §9.3.4.4-Anforderungen.
#[derive(Debug, Clone, Copy)]
pub struct HistoryQosValidator {
    /// Mindest-HISTORY-DEPTH, die der Gateway-Configurator als
    /// "ausreichend fuer den gewuenschten Time-Span" verlangt
    /// (Spec laesst die Zahl dem Implementer; default 1024).
    pub minimum_depth: u32,
}

impl Default for HistoryQosValidator {
    fn default() -> Self {
        Self {
            minimum_depth: 1024,
        }
    }
}

/// Spec-Verletzung — Was an der HISTORY-QoS nicht §9.3.4.4-konform ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryQosViolation {
    /// `KEEP_LAST` mit `depth < minimum_depth`.
    KeepLastDepthTooSmall {
        /// Konfigurierte depth.
        depth: u32,
        /// Vom Validator geforderter Mindestwert.
        minimum: u32,
    },
}

impl HistoryQosValidator {
    /// Validiert die HISTORY-QoS-Konfiguration eines DataReaders gegen
    /// Spec §9.3.4.4.
    ///
    /// # Errors
    /// `HistoryQosViolation::KeepLastDepthTooSmall` wenn `kind ==
    /// KeepLast` und `depth < minimum_depth`. `KeepAll` ist immer
    /// gueltig (Spec laesst Resource-Limits dem Implementer).
    pub fn validate(&self, kind: HistoryQosKind, depth: u32) -> Result<(), HistoryQosViolation> {
        match kind {
            HistoryQosKind::KeepAll => Ok(()),
            HistoryQosKind::KeepLast => {
                if depth < self.minimum_depth {
                    Err(HistoryQosViolation::KeepLastDepthTooSmall {
                        depth,
                        minimum: self.minimum_depth,
                    })
                } else {
                    Ok(())
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn keep_all_always_valid() {
        let v = HistoryQosValidator::default();
        assert!(v.validate(HistoryQosKind::KeepAll, 0).is_ok());
        assert!(v.validate(HistoryQosKind::KeepAll, 1).is_ok());
    }

    #[test]
    fn keep_last_with_sufficient_depth_valid() {
        let v = HistoryQosValidator { minimum_depth: 100 };
        assert!(v.validate(HistoryQosKind::KeepLast, 100).is_ok());
        assert!(v.validate(HistoryQosKind::KeepLast, 1000).is_ok());
    }

    #[test]
    fn keep_last_below_minimum_violates() {
        let v = HistoryQosValidator { minimum_depth: 100 };
        assert_eq!(
            v.validate(HistoryQosKind::KeepLast, 1),
            Err(HistoryQosViolation::KeepLastDepthTooSmall {
                depth: 1,
                minimum: 100,
            })
        );
    }
}
