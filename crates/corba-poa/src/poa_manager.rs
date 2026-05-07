// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! POAManager — Spec §11.3.2.
//!
//! Vier States + Transitions:
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
//! INACTIVE ist terminal — alle weiteren Operations werfen
//! `BadInvocationOrder` (Spec §11.3.2.1.5 normativ).

use core::sync::atomic::{AtomicU8, Ordering};

use crate::error::{PoaError, PoaResult};

/// POAManager-States.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PoaManagerState {
    /// `HOLDING` — Requests werden gequeued, kein Dispatch.
    Holding = 0,
    /// `ACTIVE` — Requests werden dispatched.
    Active = 1,
    /// `DISCARDING` — Requests werden mit `TRANSIENT` System-Exception
    /// abgelehnt (z.B. fuer Load-Shedding).
    Discarding = 2,
    /// `INACTIVE` — Manager + alle assoziierten POAs sind tot. Spec
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
/// `Send + Sync` per `AtomicU8`. Spec §11.3.2: ein POAManager wird
/// von mehreren POAs geteilt, also muss er thread-safe sein ohne
/// Mutex auf dem Hot-Path.
#[derive(Debug)]
pub struct PoaManager {
    state: AtomicU8,
}

impl PoaManager {
    /// Konstruiert einen neuen Manager im HOLDING-State (Spec
    /// §11.3.2.1 normativ: "the POAManager is in the holding state
    /// when it is created").
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(PoaManagerState::Holding as u8),
        }
    }

    /// Liefert den aktuellen State.
    #[must_use]
    pub fn state(&self) -> PoaManagerState {
        PoaManagerState::from_u8(self.state.load(Ordering::Acquire))
    }

    /// `activate` — Spec §11.3.2.1.1.
    ///
    /// # Errors
    /// `BadInvocationOrder` wenn der Manager bereits INACTIVE ist.
    pub fn activate(&self) -> PoaResult<()> {
        self.transition(PoaManagerState::Active)
    }

    /// `hold_requests` — Spec §11.3.2.1.2.
    ///
    /// # Errors
    /// `BadInvocationOrder` wenn INACTIVE.
    pub fn hold_requests(&self) -> PoaResult<()> {
        self.transition(PoaManagerState::Holding)
    }

    /// `discard_requests` — Spec §11.3.2.1.3.
    ///
    /// # Errors
    /// `BadInvocationOrder` wenn INACTIVE.
    pub fn discard_requests(&self) -> PoaResult<()> {
        self.transition(PoaManagerState::Discarding)
    }

    /// `deactivate` — Spec §11.3.2.1.4. Terminal.
    pub fn deactivate(&self) {
        self.state
            .store(PoaManagerState::Inactive as u8, Ordering::Release);
    }

    fn transition(&self, target: PoaManagerState) -> PoaResult<()> {
        // Compare-and-swap-Schleife: aus jedem nicht-Inactive-State
        // ist die Transition erlaubt; Inactive ist terminal.
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
