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
    /// `KEEP_LAST` — only the last N samples per instance.
    KeepLast,
    /// `KEEP_ALL` — all samples until resource limits force otherwise.
    KeepAll,
}

/// Validator for the DataReader HISTORY QoS against the §9.3.4.4 requirements.
#[derive(Debug, Clone, Copy)]
pub struct HistoryQosValidator {
    /// Minimum HISTORY DEPTH that the gateway configurator requires as
    /// "sufficient for the desired time span"
    /// (the spec leaves the number to the implementer; default 1024).
    pub minimum_depth: u32,
}

impl Default for HistoryQosValidator {
    fn default() -> Self {
        Self {
            minimum_depth: 1024,
        }
    }
}

/// Spec violation — what about the HISTORY QoS is not §9.3.4.4-conformant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryQosViolation {
    /// `KEEP_LAST` with `depth < minimum_depth`.
    KeepLastDepthTooSmall {
        /// Configured depth.
        depth: u32,
        /// Minimum value required by the validator.
        minimum: u32,
    },
}

impl HistoryQosValidator {
    /// Validates the HISTORY QoS configuration of a DataReader against
    /// Spec §9.3.4.4.
    ///
    /// # Errors
    /// `HistoryQosViolation::KeepLastDepthTooSmall` if `kind ==
    /// KeepLast` and `depth < minimum_depth`. `KeepAll` is always
    /// valid (the spec leaves resource limits to the implementer).
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
