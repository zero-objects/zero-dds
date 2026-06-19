// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CosTransactions ↔ JTA-Mapping — OMG Transaction Service §10 +
//! JEE JTA 1.3 §3.2.
//!
//! `CosTransactions::Status` (Spec 1.4 §2.4.5):
//! ```idl
//!   enum Status {
//!     StatusActive, StatusMarkedRollback, StatusPrepared,
//!     StatusCommitted, StatusRolledBack, StatusUnknown,
//!     StatusNoTransaction, StatusPreparing, StatusCommitting,
//!     StatusRollingBack
//!   };
//! ```
//!
//! `javax.transaction.Status` (JTA 1.3 §3.5) has the same 10 values
//! with integer codes 0-9. We map them 1:1.

use core::fmt;

/// CosTransactions::Status — OMG TX-Service 1.4 §2.4.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TxStatus {
    /// `StatusActive`.
    Active = 0,
    /// `StatusMarkedRollback`.
    MarkedRollback = 1,
    /// `StatusPrepared`.
    Prepared = 2,
    /// `StatusCommitted`.
    Committed = 3,
    /// `StatusRolledBack`.
    RolledBack = 4,
    /// `StatusUnknown`.
    Unknown = 5,
    /// `StatusNoTransaction`.
    NoTransaction = 6,
    /// `StatusPreparing`.
    Preparing = 7,
    /// `StatusCommitting`.
    Committing = 8,
    /// `StatusRollingBack`.
    RollingBack = 9,
}

/// `javax.transaction.Status` — JTA 1.3 §3.5 (Integer-Codes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum JtaStatus {
    /// `STATUS_ACTIVE = 0`.
    Active = 0,
    /// `STATUS_MARKED_ROLLBACK = 1`.
    MarkedRollback = 1,
    /// `STATUS_PREPARED = 2`.
    Prepared = 2,
    /// `STATUS_COMMITTED = 3`.
    Committed = 3,
    /// `STATUS_ROLLEDBACK = 4`.
    RolledBack = 4,
    /// `STATUS_UNKNOWN = 5`.
    Unknown = 5,
    /// `STATUS_NO_TRANSACTION = 6`.
    NoTransaction = 6,
    /// `STATUS_PREPARING = 7`.
    Preparing = 7,
    /// `STATUS_COMMITTING = 8`.
    Committing = 8,
    /// `STATUS_ROLLING_BACK = 9`.
    RollingBack = 9,
}

impl fmt::Display for TxStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Active => "StatusActive",
            Self::MarkedRollback => "StatusMarkedRollback",
            Self::Prepared => "StatusPrepared",
            Self::Committed => "StatusCommitted",
            Self::RolledBack => "StatusRolledBack",
            Self::Unknown => "StatusUnknown",
            Self::NoTransaction => "StatusNoTransaction",
            Self::Preparing => "StatusPreparing",
            Self::Committing => "StatusCommitting",
            Self::RollingBack => "StatusRollingBack",
        })
    }
}

/// Mapping CosTransactions → JTA. Both enums are 1:1.
#[must_use]
pub fn jta_status_from_cos(s: TxStatus) -> JtaStatus {
    match s {
        TxStatus::Active => JtaStatus::Active,
        TxStatus::MarkedRollback => JtaStatus::MarkedRollback,
        TxStatus::Prepared => JtaStatus::Prepared,
        TxStatus::Committed => JtaStatus::Committed,
        TxStatus::RolledBack => JtaStatus::RolledBack,
        TxStatus::Unknown => JtaStatus::Unknown,
        TxStatus::NoTransaction => JtaStatus::NoTransaction,
        TxStatus::Preparing => JtaStatus::Preparing,
        TxStatus::Committing => JtaStatus::Committing,
        TxStatus::RollingBack => JtaStatus::RollingBack,
    }
}

/// Mapping JTA → CosTransactions. Both enums are 1:1.
#[must_use]
pub fn jta_status_to_cos(s: JtaStatus) -> TxStatus {
    match s {
        JtaStatus::Active => TxStatus::Active,
        JtaStatus::MarkedRollback => TxStatus::MarkedRollback,
        JtaStatus::Prepared => TxStatus::Prepared,
        JtaStatus::Committed => TxStatus::Committed,
        JtaStatus::RolledBack => TxStatus::RolledBack,
        JtaStatus::Unknown => TxStatus::Unknown,
        JtaStatus::NoTransaction => TxStatus::NoTransaction,
        JtaStatus::Preparing => TxStatus::Preparing,
        JtaStatus::Committing => TxStatus::Committing,
        JtaStatus::RollingBack => TxStatus::RollingBack,
    }
}

/// Bridge — connects a CosTransactions::Current with a
/// JTA UserTransaction. Trait definition; the caller layer provides
/// the JNI/JVM bridge.
pub trait TxBridge {
    /// Current status of the bridge transaction.
    fn status(&self) -> TxStatus;

    /// Begin a new transaction (CosTransactions: `Current::begin()`,
    /// JTA: `UserTransaction.begin()`).
    ///
    /// # Errors
    /// Static string if the begin fails.
    fn begin(&mut self) -> Result<(), &'static str>;

    /// Commit (CosTransactions: `Current::commit()`, JTA:
    /// `UserTransaction.commit()`).
    ///
    /// # Errors
    /// Static string on heuristic/rollback.
    fn commit(&mut self) -> Result<(), &'static str>;

    /// Rollback.
    ///
    /// # Errors
    /// Static string if the rollback fails.
    fn rollback(&mut self) -> Result<(), &'static str>;

    /// Marks the transaction as rollback-only (CosTransactions:
    /// `Current::rollback_only()`, JTA: `setRollbackOnly()`).
    ///
    /// # Errors
    /// Static string if no transaction is active.
    fn set_rollback_only(&mut self) -> Result<(), &'static str>;
}

/// In-memory bridge for tests + default behavior.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InMemoryTxBridge {
    state: Option<TxStatus>,
}

impl InMemoryTxBridge {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl TxBridge for InMemoryTxBridge {
    fn status(&self) -> TxStatus {
        self.state.unwrap_or(TxStatus::NoTransaction)
    }

    fn begin(&mut self) -> Result<(), &'static str> {
        if self.state.is_some() {
            return Err("transaction already active");
        }
        self.state = Some(TxStatus::Active);
        Ok(())
    }

    fn commit(&mut self) -> Result<(), &'static str> {
        match self.state {
            Some(TxStatus::Active) => {
                self.state = Some(TxStatus::Committed);
                Ok(())
            }
            Some(TxStatus::MarkedRollback) => {
                self.state = Some(TxStatus::RolledBack);
                Err("commit forced rollback (marked rollback)")
            }
            _ => Err("no active transaction to commit"),
        }
    }

    fn rollback(&mut self) -> Result<(), &'static str> {
        match self.state {
            Some(TxStatus::Active | TxStatus::MarkedRollback) => {
                self.state = Some(TxStatus::RolledBack);
                Ok(())
            }
            _ => Err("no active transaction to rollback"),
        }
    }

    fn set_rollback_only(&mut self) -> Result<(), &'static str> {
        match self.state {
            Some(TxStatus::Active) => {
                self.state = Some(TxStatus::MarkedRollback);
                Ok(())
            }
            _ => Err("no active transaction"),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn cos_to_jta_round_trip_for_all_states() {
        for cos in [
            TxStatus::Active,
            TxStatus::MarkedRollback,
            TxStatus::Prepared,
            TxStatus::Committed,
            TxStatus::RolledBack,
            TxStatus::Unknown,
            TxStatus::NoTransaction,
            TxStatus::Preparing,
            TxStatus::Committing,
            TxStatus::RollingBack,
        ] {
            let jta = jta_status_from_cos(cos);
            let back = jta_status_to_cos(jta);
            assert_eq!(cos, back);
            assert_eq!(cos as u8, jta as i32 as u8);
        }
    }

    #[test]
    fn fresh_bridge_has_no_transaction() {
        let b = InMemoryTxBridge::new();
        assert_eq!(b.status(), TxStatus::NoTransaction);
    }

    #[test]
    fn begin_then_commit_succeeds() {
        let mut b = InMemoryTxBridge::new();
        b.begin().unwrap();
        assert_eq!(b.status(), TxStatus::Active);
        b.commit().unwrap();
        assert_eq!(b.status(), TxStatus::Committed);
    }

    #[test]
    fn double_begin_fails() {
        let mut b = InMemoryTxBridge::new();
        b.begin().unwrap();
        assert!(b.begin().is_err());
    }

    #[test]
    fn rollback_only_then_commit_fails_with_rollback() {
        let mut b = InMemoryTxBridge::new();
        b.begin().unwrap();
        b.set_rollback_only().unwrap();
        assert_eq!(b.status(), TxStatus::MarkedRollback);
        assert!(b.commit().is_err());
        assert_eq!(b.status(), TxStatus::RolledBack);
    }

    #[test]
    fn rollback_active_succeeds() {
        let mut b = InMemoryTxBridge::new();
        b.begin().unwrap();
        b.rollback().unwrap();
        assert_eq!(b.status(), TxStatus::RolledBack);
    }

    #[test]
    fn set_rollback_only_without_tx_fails() {
        let mut b = InMemoryTxBridge::new();
        assert!(b.set_rollback_only().is_err());
    }

    #[test]
    fn display_for_status_returns_spec_name() {
        assert_eq!(format!("{}", TxStatus::Active), "StatusActive");
        assert_eq!(format!("{}", TxStatus::RolledBack), "StatusRolledBack");
    }
}
