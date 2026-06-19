// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! POAManager — Spec §11.3.2.
//!
//! Four states + transitions:
//!
//! ```text
//!         create -> HOLDING
//!
//!  activate / hold_requests / discard_requests :
//!    *      -> ACTIVE
//!    *      -> HOLDING
//!    *      -> DISCARDING
//!
//!  deactivate :
//!    *      -> INACTIVE  (terminal)
//! ```
//!
//! INACTIVE is terminal — all further operations raise
//! `BadInvocationOrder` (Spec §11.3.2.1.5, normative).

use core::sync::atomic::{AtomicU8, Ordering};

use crate::error::{PoaError, PoaResult};

/// POAManager states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PoaManagerState {
    /// `HOLDING` — requests are queued, no dispatch.
    Holding = 0,
    /// `ACTIVE` — requests are dispatched.
    Active = 1,
    /// `DISCARDING` — requests are rejected with a `TRANSIENT` system
    /// exception (e.g. for load shedding).
    Discarding = 2,
    /// `INACTIVE` — the manager and all associated POAs are dead. Spec
    /// §11.3.2.1.5: "this is the final state of the POAManager".
    Inactive = 3,
}

impl PoaManagerState {
    fn from_u8(v: u8) -> PoaManagerState {
        match v {
            0 => Self::Holding,
            1 => Self::Active,
            2 => Self::Discarding,
            _ => Self::Inactive,
        }
    }
}

/// POAManager.
///
/// `Send + Sync` via `AtomicU8`. Spec §11.3.2: a POAManager is
/// shared by multiple POAs, so it must be thread-safe without a
/// mutex on the hot path.
#[derive(Debug)]
pub struct PoaManager {
    state: AtomicU8,
}

impl PoaManager {
    /// Constructs a new manager in the HOLDING state (Spec
    /// §11.3.2.1, normative: "the POAManager is in the holding state
    /// when it is created").
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(PoaManagerState::Holding as u8),
        }
    }

    /// Returns the current state.
    #[must_use]
    pub fn state(&self) -> PoaManagerState {
        PoaManagerState::from_u8(self.state.load(Ordering::Acquire))
    }

    /// `activate` — Spec §11.3.2.1.1.
    ///
    /// # Errors
    /// `BadInvocationOrder` if the manager is already INACTIVE.
    pub fn activate(&self) -> PoaResult<()> {
        self.transition(PoaManagerState::Active)
    }

    /// `hold_requests` — Spec §11.3.2.1.2.
    ///
    /// # Errors
    /// `BadInvocationOrder` if INACTIVE.
    pub fn hold_requests(&self) -> PoaResult<()> {
        self.transition(PoaManagerState::Holding)
    }

    /// `discard_requests` — Spec §11.3.2.1.3.
    ///
    /// # Errors
    /// `BadInvocationOrder` if INACTIVE.
    pub fn discard_requests(&self) -> PoaResult<()> {
        self.transition(PoaManagerState::Discarding)
    }

    /// `deactivate` — Spec §11.3.2.1.4. Terminal.
    pub fn deactivate(&self) {
        self.state
            .store(PoaManagerState::Inactive as u8, Ordering::Release);
    }

    fn transition(&self, target: PoaManagerState) -> PoaResult<()> {
        // Compare-and-swap loop: the transition is allowed from any
        // non-Inactive state; Inactive is terminal.
        loop {
            let current = self.state.load(Ordering::Acquire);
            if current == PoaManagerState::Inactive as u8 {
                return Err(PoaError::BadInvocationOrder(
                    "POAManager is INACTIVE; further transitions are not allowed".into(),
                ));
            }
            if self
                .state
                .compare_exchange(current, target as u8, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(());
            }
        }
    }
}

impl Default for PoaManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_in_holding() {
        let m = PoaManager::new();
        assert_eq!(m.state(), PoaManagerState::Holding);
    }

    #[test]
    fn full_state_cycle() {
        let m = PoaManager::new();
        m.activate().unwrap();
        assert_eq!(m.state(), PoaManagerState::Active);
        m.discard_requests().unwrap();
        assert_eq!(m.state(), PoaManagerState::Discarding);
        m.hold_requests().unwrap();
        assert_eq!(m.state(), PoaManagerState::Holding);
        m.activate().unwrap();
        assert_eq!(m.state(), PoaManagerState::Active);
        m.deactivate();
        assert_eq!(m.state(), PoaManagerState::Inactive);
    }

    #[test]
    fn inactive_is_terminal() {
        let m = PoaManager::new();
        m.deactivate();
        assert!(matches!(m.activate(), Err(PoaError::BadInvocationOrder(_))));
        assert!(matches!(
            m.hold_requests(),
            Err(PoaError::BadInvocationOrder(_))
        ));
        assert!(matches!(
            m.discard_requests(),
            Err(PoaError::BadInvocationOrder(_))
        ));
    }

    #[test]
    fn deactivate_idempotent() {
        let m = PoaManager::new();
        m.deactivate();
        m.deactivate(); // no-op, still INACTIVE.
        assert_eq!(m.state(), PoaManagerState::Inactive);
    }
}
