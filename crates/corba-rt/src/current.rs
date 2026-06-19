// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `RTCORBA::Current` (RT-CORBA §5.5) — read/write access to the CORBA
//! priority of the current execution context.

use crate::priority::Priority;

/// `RTCORBA::Current` (§5.5): holds the CORBA priority of the current context.
/// Setting it changes (in a real ORB) the native thread priority via the
/// active [`PriorityMapping`](crate::priority::PriorityMapping).
#[derive(Debug, Clone, Copy)]
pub struct RtCurrent {
    priority: Priority,
}

impl RtCurrent {
    /// New `Current` with an initial priority.
    #[must_use]
    pub fn new(priority: Priority) -> Self {
        Self { priority }
    }

    /// `get_priority` (§5.5) — the current CORBA priority.
    #[must_use]
    pub fn get_priority(&self) -> Priority {
        self.priority
    }

    /// `set_priority` (§5.5) — sets the CORBA priority of the context.
    pub fn set_priority(&mut self, priority: Priority) {
        self.priority = priority;
    }
}

impl Default for RtCurrent {
    fn default() -> Self {
        // Default priority 0 is always valid.
        Self {
            priority: Priority::new(0).unwrap_or_else(|| Priority::clamped(0)),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn get_set_priority() {
        let mut cur = RtCurrent::new(Priority::new(10).unwrap());
        assert_eq!(cur.get_priority().value(), 10);
        cur.set_priority(Priority::new(50).unwrap());
        assert_eq!(cur.get_priority().value(), 50);
    }

    #[test]
    fn default_is_zero() {
        assert_eq!(RtCurrent::default().get_priority().value(), 0);
    }
}
