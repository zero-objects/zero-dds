// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Sender-/Receiver-Link-Acceptance + Settlement-Tracking.
//!
//! Spec-Quelle: DDS-AMQP-1.0 §7.4 Settlement-Mode-Mapping +
//! `amqp-1.0-transport` §2.6 Link-Lifecycle.

use alloc::string::String;

use crate::dds_bridge::{DispositionMapper, DispositionState};

/// Spec §2.6 — Link-Role aus Sicht des AMQP-Endpoints.
///
/// Bei einem Sender-Link sendet der Endpoint Transfers an den Peer
/// (DDS->AMQP-Konsumer). Bei einem Receiver-Link empfaengt er sie
/// (AMQP-Producer->DDS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkRole {
    /// Endpoint sendet Transfers (DDS-Sample -> AMQP-Consumer).
    Sender,
    /// Endpoint empfaengt Transfers (AMQP-Producer -> DDS-Sample).
    Receiver,
}

/// Spec §2.6.4 / dds-amqp-1.0-beta1 §7.4 — Settlement-Mode des Links.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementMode {
    /// `BEST_EFFORT` (DDS) ↔ pre-settled (AMQP).
    Settled,
    /// `RELIABLE` (DDS) ↔ unsettled mit Disposition-Acknowledgment.
    Unsettled,
}

/// AMQP `terminus.durable`-Wert (Spec §3.5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminusDurability {
    /// `none` (0) — keine durability-state-retention.
    None,
    /// `configuration` (1) — durability nur fuer terminus-config.
    Configuration,
    /// `unsettled-state` (2) — broker-level message-durability.
    UnsettledState,
}

impl TerminusDurability {
    /// AMQP wire-value parsen (Spec §3.5.3).
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

/// Spec §7.4.2 — Resultat einer DURABILITY-Pre-Attach-Pruefung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachDurabilityCheck {
    /// `terminus.durable` ist akzeptabel — Attach kann fortgesetzt
    /// werden.
    Accept,
    /// `terminus.durable = unsettled-state` (2) verlangt Broker-
    /// Funktionalitaet, die diese Spec out-of-scope laesst → Attach
    /// SHALL mit `amqp:not-implemented` rejected werden
    /// (Spec §7.4.2 + Annex C C.1.x).
    RejectNotImplemented,
}

/// Spec §7.4.2 + §11.2 — pruefen, ob ein Attach mit gegebenem
/// `terminus.durable`-Wert akzeptiert werden darf.
///
/// `None`/`Configuration` → akzeptiert.
/// `UnsettledState` → SHALL rejected mit
/// `amqp:not-implemented` (broker-level message durability ist
/// out-of-scope fuer diese Spec).
#[must_use]
pub const fn check_attach_durability(durable: TerminusDurability) -> AttachDurabilityCheck {
    match durable {
        TerminusDurability::None | TerminusDurability::Configuration => {
            AttachDurabilityCheck::Accept
        }
        TerminusDurability::UnsettledState => AttachDurabilityCheck::RejectNotImplemented,
    }
}

/// Aktiver Link-Sub-State des Endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSession {
    /// Eindeutiger Link-Name (Spec §2.6.1).
    pub name: String,
    /// Handle (Spec §2.6.5) — innerhalb der Session eindeutig.
    pub handle: u32,
    /// Rolle des Endpoints.
    pub role: LinkRole,
    /// Settlement-Modus.
    pub settlement: SettlementMode,
    /// Anzahl noch ausstehender Disposition-Acknowledgments
    /// (Settlement-Tracking, nur fuer `Unsettled`).
    pub pending_settlements: u32,
    /// Anzahl bisher gesendeter Transfers (fuer Flow-Credit-
    /// Authority).
    pub delivered: u64,
    /// Aktuelles Flow-Credit (Spec §2.6.7).
    pub credit: u32,
}

/// Fehler beim Senden eines Transfers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliverError {
    /// Kein Flow-Credit verfuegbar.
    NoCredit,
}

impl LinkSession {
    /// Erzeugt einen neuen Link-Sub-State.
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

    /// Server-Side erhoeht das Flow-Credit (Receiver-Link bekommt
    /// einen `flow`-Performative auf den Wire). Bei Sender-Links ist
    /// das Credit clientseitig kontrolliert; wir speichern es nur
    /// fuer Telemetrie.
    pub fn grant_credit(&mut self, additional: u32) {
        self.credit = self.credit.saturating_add(additional);
    }

    /// Beim Senden eines Transfers: Credit konsumieren, delivered
    /// inkrementieren, ggf. pending_settlements anlegen.
    ///
    /// # Errors
    /// `NoCredit` wenn kein Credit verfuegbar.
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

    /// Beim Empfangen einer Disposition-Acknowledgment: pending zaehlen
    /// herunter.
    ///
    /// Diese Variante macht **kein** DDS-Side-Sample-State-Update;
    /// sie eignet sich fuer AMQP-only-Workflows ohne DDS-Bridge. Mit
    /// DDS-Bridge: [`Self::settle_with_mapper`] verwenden, das
    /// zusaetzlich [`DispositionMapper::apply`] (Spec §7.7.3) aufruft.
    pub fn settle(&mut self) {
        if self.pending_settlements > 0 {
            self.pending_settlements -= 1;
        }
    }

    /// Spec §7.7.3 — Beim Empfangen einer Disposition-Acknowledgment:
    /// pending-Counter dekrementieren UND `mapper.apply(...)` mit dem
    /// dekodierten `sample_handle` und [`DispositionState`] aufrufen.
    ///
    /// Dies ist der spec-konforme Wire-up-Pfad fuer DDS-AMQP-Endpoints
    /// mit DDS-Bridge: der Caller liefert seinen
    /// [`DispositionMapper`]-Implementor (typisch eine DCPS-Bruecke,
    /// die `acknowledged()`/`unacknowledged()` auf den DDS-side
    /// DataWriter ruft).
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

    /// Spec §7.7.3 — Disposition-Mapper-Wire-up: `settle_with_mapper`
    /// MUSS den Caller-Mapper mit dem korrekten Sample-Handle und
    /// Disposition-State aufrufen, UND den pending-Counter
    /// dekrementieren.
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

        // Counter dekrementiert beide Male.
        assert_eq!(l.pending_settlements, 0);
        // Mapper hat genau die zwei Calls in Reihenfolge gesehen.
        let calls = mapper.calls.borrow();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], (h1, DispositionState::Accepted));
        assert_eq!(calls[1], (h2, DispositionState::Rejected));
    }

    #[test]
    fn settle_with_mapper_underflow_safe_at_zero() {
        // Wenn pending_settlements bereits 0 ist (z.B. doppeltes
        // Disposition-Frame oder Settled-Mode), darf der Counter
        // nicht underflowen — der Mapper wird trotzdem aufgerufen,
        // weil das Caller-Update Spec-§7.7.3 mandatorisch ist.
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
        // ist out-of-scope und SHALL mit amqp:not-implemented
        // rejected werden.
        assert_eq!(
            check_attach_durability(TerminusDurability::UnsettledState),
            AttachDurabilityCheck::RejectNotImplemented
        );
    }
}
