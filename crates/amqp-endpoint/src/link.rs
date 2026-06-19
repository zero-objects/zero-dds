// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Sender/receiver link acceptance + settlement tracking.
//!
//! Spec source: DDS-AMQP-1.0 §7.4 settlement-mode mapping +
//! `amqp-1.0-transport` §2.6 link lifecycle.

use alloc::string::String;

use crate::dds_bridge::{DispositionMapper, DispositionState};

/// Spec §2.6 — link role from the AMQP endpoint's perspective.
///
/// On a sender link the endpoint sends transfers to the peer
/// (DDS->AMQP consumer). On a receiver link it receives them
/// (AMQP producer->DDS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkRole {
    /// Endpoint sends transfers (DDS sample -> AMQP consumer).
    Sender,
    /// Endpoint receives transfers (AMQP producer -> DDS sample).
    Receiver,
}

/// Spec §2.6.4 / dds-amqp-1.0-beta1 §7.4 — settlement mode of the link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementMode {
    /// `BEST_EFFORT` (DDS) ↔ pre-settled (AMQP).
    Settled,
    /// `RELIABLE` (DDS) ↔ unsettled with disposition acknowledgment.
    Unsettled,
}

/// AMQP `terminus.durable` value (Spec §3.5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminusDurability {
    /// `none` (0) — no durability-state retention.
    None,
    /// `configuration` (1) — durability only for terminus config.
    Configuration,
    /// `unsettled-state` (2) — broker-level message durability.
    UnsettledState,
}

impl TerminusDurability {
    /// Parse the AMQP wire value (Spec §3.5.3).
    #[must_use]
    pub const fn from_wire(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::None),
            1 => Some(Self::Configuration),
            2 => Some(Self::UnsettledState),
            _ => None,
        }
    }
}

/// Spec §7.4.2 — result of a DURABILITY pre-attach check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachDurabilityCheck {
    /// `terminus.durable` is acceptable — the attach may proceed.
    Accept,
    /// `terminus.durable = unsettled-state` (2) requires broker
    /// functionality that this spec leaves out of scope → the attach
    /// SHALL be rejected with `amqp:not-implemented`
    /// (Spec §7.4.2 + Annex C C.1.x).
    RejectNotImplemented,
}

/// Spec §7.4.2 + §11.2 — check whether an attach with the given
/// `terminus.durable` value may be accepted.
///
/// `None`/`Configuration` → accepted.
/// `UnsettledState` → SHALL be rejected with
/// `amqp:not-implemented` (broker-level message durability is
/// out of scope for this spec).
#[must_use]
pub const fn check_attach_durability(durable: TerminusDurability) -> AttachDurabilityCheck {
    match durable {
        TerminusDurability::None | TerminusDurability::Configuration => {
            AttachDurabilityCheck::Accept
        }
        TerminusDurability::UnsettledState => AttachDurabilityCheck::RejectNotImplemented,
    }
}

/// Active link sub-state of the endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSession {
    /// Unique link name (Spec §2.6.1).
    pub name: String,
    /// Handle (Spec §2.6.5) — unique within the session.
    pub handle: u32,
    /// Role of the endpoint.
    pub role: LinkRole,
    /// Settlement mode.
    pub settlement: SettlementMode,
    /// Number of disposition acknowledgments still outstanding
    /// (settlement tracking, only for `Unsettled`).
    pub pending_settlements: u32,
    /// Number of transfers sent so far (for flow-credit
    /// authority).
    pub delivered: u64,
    /// Current flow credit (Spec §2.6.7).
    pub credit: u32,
}

/// Error when sending a transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliverError {
    /// No flow credit available.
    NoCredit,
}

impl LinkSession {
    /// Creates a new link sub-state.
    #[must_use]
    pub fn new(name: String, handle: u32, role: LinkRole, settlement: SettlementMode) -> Self {
        Self {
            name,
            handle,
            role,
            settlement,
            pending_settlements: 0,
            delivered: 0,
            credit: 0,
        }
    }

    /// Server side raises the flow credit (a receiver link gets a
    /// `flow` performative on the wire). For sender links the credit
    /// is client-controlled; we only store it for telemetry.
    pub fn grant_credit(&mut self, additional: u32) {
        self.credit = self.credit.saturating_add(additional);
    }

    /// When sending a transfer: consume credit, increment delivered,
    /// and create a pending_settlements entry if applicable.
    ///
    /// # Errors
    /// `NoCredit` if no credit is available.
    pub fn deliver(&mut self) -> Result<(), DeliverError> {
        if self.credit == 0 {
            return Err(DeliverError::NoCredit);
        }
        self.credit -= 1;
        self.delivered = self.delivered.saturating_add(1);
        if self.settlement == SettlementMode::Unsettled {
            self.pending_settlements = self.pending_settlements.saturating_add(1);
        }
        Ok(())
    }

    /// When receiving a disposition acknowledgment: decrement the
    /// pending count.
    ///
    /// This variant performs **no** DDS-side sample-state update;
    /// it suits AMQP-only workflows without a DDS bridge. With a
    /// DDS bridge: use [`Self::settle_with_mapper`], which
    /// additionally calls [`DispositionMapper::apply`] (Spec §7.7.3).
    pub fn settle(&mut self) {
        if self.pending_settlements > 0 {
            self.pending_settlements -= 1;
        }
    }

    /// Spec §7.7.3 — when receiving a disposition acknowledgment:
    /// decrement the pending counter AND call `mapper.apply(...)` with
    /// the decoded `sample_handle` and [`DispositionState`].
    ///
    /// This is the spec-compliant wire-up path for DDS-AMQP endpoints
    /// with a DDS bridge: the caller supplies its
    /// [`DispositionMapper`] implementor (typically a DCPS bridge
    /// that calls `acknowledged()`/`unacknowledged()` on the DDS-side
    /// DataWriter).
    pub fn settle_with_mapper<M: DispositionMapper>(
        &mut self,
        mapper: &M,
        sample_handle: [u8; 16],
        state: DispositionState,
    ) {
        mapper.apply(sample_handle, state);
        if self.pending_settlements > 0 {
            self.pending_settlements -= 1;
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    fn link(role: LinkRole, mode: SettlementMode) -> LinkSession {
        LinkSession::new("L1".to_string(), 0, role, mode)
    }

    #[test]
    fn new_link_starts_with_zero_credit_and_zero_delivered() {
        let l = link(LinkRole::Sender, SettlementMode::Unsettled);
        assert_eq!(l.credit, 0);
        assert_eq!(l.delivered, 0);
        assert_eq!(l.pending_settlements, 0);
    }

    #[test]
    fn grant_credit_accumulates() {
        let mut l = link(LinkRole::Sender, SettlementMode::Settled);
        l.grant_credit(10);
        l.grant_credit(5);
        assert_eq!(l.credit, 15);
    }

    #[test]
    fn deliver_consumes_credit_and_increments_delivered() {
        let mut l = link(LinkRole::Sender, SettlementMode::Settled);
        l.grant_credit(2);
        assert!(l.deliver().is_ok());
        assert_eq!(l.credit, 1);
        assert_eq!(l.delivered, 1);
        assert_eq!(l.pending_settlements, 0); // settled mode
    }

    #[test]
    fn deliver_without_credit_yields_error() {
        let mut l = link(LinkRole::Sender, SettlementMode::Settled);
        assert!(l.deliver().is_err());
    }

    #[test]
    fn unsettled_deliver_increments_pending() {
        let mut l = link(LinkRole::Sender, SettlementMode::Unsettled);
        l.grant_credit(3);
        l.deliver().expect("ok");
        l.deliver().expect("ok");
        assert_eq!(l.pending_settlements, 2);
    }

    #[test]
    fn settle_decrements_pending() {
        let mut l = link(LinkRole::Sender, SettlementMode::Unsettled);
        l.grant_credit(3);
        l.deliver().expect("ok");
        l.deliver().expect("ok");
        l.settle();
        assert_eq!(l.pending_settlements, 1);
    }

    #[test]
    fn settle_at_zero_does_not_underflow() {
        let mut l = link(LinkRole::Sender, SettlementMode::Settled);
        l.settle();
        assert_eq!(l.pending_settlements, 0);
    }

    /// Spec §7.7.3 — disposition-mapper wire-up: `settle_with_mapper`
    /// MUST call the caller's mapper with the correct sample handle and
    /// disposition state, AND decrement the pending counter.
    #[test]
    fn settle_with_mapper_calls_apply_and_decrements_pending() {
        use core::cell::RefCell;

        struct RecordingMapper {
            calls: RefCell<alloc::vec::Vec<([u8; 16], DispositionState)>>,
        }

        impl DispositionMapper for RecordingMapper {
            fn apply(&self, sample_handle: [u8; 16], state: DispositionState) {
                self.calls.borrow_mut().push((sample_handle, state));
            }
        }

        let mapper = RecordingMapper {
            calls: RefCell::new(alloc::vec::Vec::new()),
        };

        let mut l = link(LinkRole::Sender, SettlementMode::Unsettled);
        l.grant_credit(3);
        l.deliver().expect("ok");
        l.deliver().expect("ok");
        assert_eq!(l.pending_settlements, 2);

        let h1 = [0x11u8; 16];
        let h2 = [0x22u8; 16];
        l.settle_with_mapper(&mapper, h1, DispositionState::Accepted);
        l.settle_with_mapper(&mapper, h2, DispositionState::Rejected);

        // Counter decremented both times.
        assert_eq!(l.pending_settlements, 0);
        // Mapper saw exactly the two calls in order.
        let calls = mapper.calls.borrow();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], (h1, DispositionState::Accepted));
        assert_eq!(calls[1], (h2, DispositionState::Rejected));
    }

    #[test]
    fn settle_with_mapper_underflow_safe_at_zero() {
        // If pending_settlements is already 0 (e.g. a duplicate
        // disposition frame or settled mode), the counter must not
        // underflow — the mapper is still called, because the caller
        // update is mandated by Spec §7.7.3.
        use core::cell::Cell;

        struct CountingMapper {
            count: Cell<u32>,
        }

        impl DispositionMapper for CountingMapper {
            fn apply(&self, _: [u8; 16], _: DispositionState) {
                self.count.set(self.count.get() + 1);
            }
        }

        let mapper = CountingMapper {
            count: Cell::new(0),
        };
        let mut l = link(LinkRole::Sender, SettlementMode::Settled);
        l.settle_with_mapper(&mapper, [0u8; 16], DispositionState::Accepted);
        assert_eq!(l.pending_settlements, 0);
        assert_eq!(mapper.count.get(), 1);
    }

    #[test]
    fn terminus_durability_from_wire() {
        assert_eq!(
            TerminusDurability::from_wire(0),
            Some(TerminusDurability::None)
        );
        assert_eq!(
            TerminusDurability::from_wire(1),
            Some(TerminusDurability::Configuration)
        );
        assert_eq!(
            TerminusDurability::from_wire(2),
            Some(TerminusDurability::UnsettledState)
        );
        assert_eq!(TerminusDurability::from_wire(3), None);
    }

    #[test]
    fn attach_durability_none_accepted() {
        assert_eq!(
            check_attach_durability(TerminusDurability::None),
            AttachDurabilityCheck::Accept
        );
    }

    #[test]
    fn attach_durability_configuration_accepted() {
        assert_eq!(
            check_attach_durability(TerminusDurability::Configuration),
            AttachDurabilityCheck::Accept
        );
    }

    #[test]
    fn attach_durability_unsettled_state_rejected_not_implemented() {
        // Spec §7.4.2: durability=unsettled-state (broker-level)
        // is out of scope and SHALL be rejected with
        // amqp:not-implemented.
        assert_eq!(
            check_attach_durability(TerminusDurability::UnsettledState),
            AttachDurabilityCheck::RejectNotImplemented
        );
    }
}
