// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-Bridge-Trait-Surface.
//!
//! Spec dds-amqp-1.0:
//! * §7.7.2 Inbound Operation Signals — `dds:operation`-Werte
//!   (`write/register/unregister/dispose`) auf
//!   DataWriter-Method-Calls dispatchen.
//! * §7.7.3 Disposition Mapping — AMQP-Disposition auf
//!   DDS-Sample-State-Update.
//! * §11.3 Instance-Lifecycle Failures — Fehler beim Lifecycle-
//!   Handling sind spec-konform auf AMQP-Errors abzubilden.
//!
//! Das Endpoint-Crate selbst bringt keine DDS-DataWriter/Reader-
//! Implementierung mit (das ist die DCPS-Crate). Wir definieren
//! Trait-Surface, gegen die der Daemon (oder ein Test-Mock) bindet
//! — analog zu [`crate::security::AccessControlPlugin`].

use alloc::string::String;
use alloc::vec::Vec;

use crate::errors::{AmqpError, instance_unknown, register_missing_key, unknown_dds_operation};
use crate::properties::DdsOperation;

// ============================================================
// §7.7.2 — Inbound Operation Dispatcher
// ============================================================

/// Eingabe fuer einen Inbound-Operation-Dispatch-Call.
#[derive(Debug, Clone)]
pub struct InboundOperation {
    /// `dds:operation` aus den Application-Properties.
    pub operation: DdsOperation,
    /// Topic-Name, gegen den dispatched wird.
    pub topic: String,
    /// `dds:instance-handle` (16 Byte) — identifiziert die Instanz.
    /// Bei `register` aus dem Body abgeleitet, bei
    /// `unregister`/`dispose` aus dieser Property.
    pub instance_handle: Option<[u8; 16]>,
    /// Sample-Body-Bytes (XCDR2 / JSON / native).
    pub body: Vec<u8>,
}

/// Spec §11.3 — Resultat einer Lifecycle-Dispatch-Operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// Operation wurde erfolgreich an DDS-DataWriter gemeldet.
    Accepted,
    /// `unregister` / `dispose` auf unbekannter Instanz —
    /// `amqp:precondition-failed`.
    UnknownInstance,
    /// `register` ohne Key-Felder im Body —
    /// `amqp:decode-error`.
    RegisterMissingKey,
    /// Unbekannter operation-Wert (Spec §11.2 →
    /// `amqp:not-implemented`).
    UnknownOperation(String),
}

impl DispatchOutcome {
    /// Spec §11.2 + §11.3 — Outcome auf `AmqpError` abbilden.
    /// Liefert `None` wenn Outcome `Accepted` ist.
    #[must_use]
    pub fn to_amqp_error(&self, key_hex: &str) -> Option<AmqpError> {
        match self {
            Self::Accepted => None,
            Self::UnknownInstance => Some(instance_unknown("unregister/dispose", key_hex)),
            Self::RegisterMissingKey => Some(register_missing_key()),
            Self::UnknownOperation(v) => Some(unknown_dds_operation(v)),
        }
    }
}

/// Spec §7.7.2 — DDS-Bridge-Adapter, der einen Inbound-Operation
/// auf einen DDS-DataWriter-Method-Call dispatcht.
///
/// Implementer:
/// * Daemon-Variante mit echter DCPS-Bruecke.
/// * Test-Mock fuer Conformance-Tests.
pub trait DdsOperationDispatcher {
    /// Dispatch der eingelesenen Operation.
    ///
    /// # Errors / Outcome
    /// Liefert `DispatchOutcome::Accepted` bei Erfolg, sonst
    /// einen Spec-konformen Fehler-Outcome.
    fn dispatch(&self, op: &InboundOperation) -> DispatchOutcome;
}

/// No-op-Dispatcher, der alle Operationen akzeptiert.
/// Test-Default; **nicht** fuer Produktion.
#[derive(Debug, Default)]
pub struct AcceptAllDispatcher;

impl DdsOperationDispatcher for AcceptAllDispatcher {
    fn dispatch(&self, _op: &InboundOperation) -> DispatchOutcome {
        DispatchOutcome::Accepted
    }
}

/// Dispatcher, der eine Liste registrierter Instance-Handles
/// pflegt und Spec-§11.3-Fehler emittiert.
#[derive(Debug, Default)]
pub struct InstanceTrackingDispatcher {
    known: alloc::collections::BTreeSet<[u8; 16]>,
}

impl InstanceTrackingDispatcher {
    /// Frischer Tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Anzahl bekannter Instanzen (test-helper).
    #[must_use]
    pub fn known_count(&self) -> usize {
        self.known.len()
    }
}

impl DdsOperationDispatcher for InstanceTrackingDispatcher {
    fn dispatch(&self, op: &InboundOperation) -> DispatchOutcome {
        match op.operation {
            DdsOperation::Register => {
                // Spec §11.3 — register OHNE Key-Felder im Body
                // ist Decode-Error.
                if op.body.is_empty() {
                    return DispatchOutcome::RegisterMissingKey;
                }
                DispatchOutcome::Accepted
            }
            DdsOperation::Unregister | DdsOperation::Dispose => {
                // Spec §11.3 — unbekannte Instanz → precondition-
                // failed.
                if let Some(h) = op.instance_handle {
                    if !self.known.contains(&h) {
                        return DispatchOutcome::UnknownInstance;
                    }
                }
                DispatchOutcome::Accepted
            }
            DdsOperation::Write => DispatchOutcome::Accepted,
        }
    }
}

impl InstanceTrackingDispatcher {
    /// Test-helper: registriere eine Instanz.
    pub fn register_instance(&mut self, handle: [u8; 16]) {
        self.known.insert(handle);
    }
}

// ============================================================
// §7.7.3 — Disposition Mapping
// ============================================================

/// Spec §3.4 + §7.7.3 — AMQP-Disposition-State.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispositionState {
    /// Spec §3.4.2 — `accepted`.
    Accepted,
    /// Spec §3.4.3 — `rejected` (mit Error-Condition).
    Rejected,
    /// Spec §3.4.4 — `released`.
    Released,
    /// Spec §3.4.5 — `modified`.
    Modified,
}

/// Spec §7.7.3 — DDS-Side-Sample-State-Update aus AMQP-
/// Disposition-Result.
///
/// Wire-up-Pfad: [`crate::link::LinkSession::settle_with_mapper`] ruft
/// dieses Trait beim Empfangen eines AMQP-Disposition-Frames mit dem
/// dekodierten Sample-Handle und [`DispositionState`] auf.
///
/// Caller-Implementer (typisch eine DCPS-Bruecke):
/// * `accepted` → `acknowledged()` auf den DDS-DataWriter.
/// * `rejected` / `released` → `unacknowledged()`.
/// * `modified` → typisch wie `rejected` behandeln.
///
/// Fuer AMQP-only-Workflows ohne DDS-Bridge:
/// [`NoopDispositionMapper`] verwenden ODER
/// [`crate::link::LinkSession::settle`] (counter-only) aufrufen.
pub trait DispositionMapper {
    /// Wendet ein Disposition auf den DDS-side DataWriter an.
    fn apply(&self, sample_handle: [u8; 16], state: DispositionState);
}

/// Null-Object-Default fuer AMQP-only-Workflows ohne DDS-Side-Sample-
/// State-Update.
///
/// Caller mit DDS-Bridge implementieren `DispositionMapper` selbst
/// und uebergeben es an
/// [`crate::link::LinkSession::settle_with_mapper`].
#[derive(Debug, Default)]
pub struct NoopDispositionMapper;

impl DispositionMapper for NoopDispositionMapper {
    fn apply(&self, _: [u8; 16], _: DispositionState) {}
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn op(operation: DdsOperation) -> InboundOperation {
        InboundOperation {
            operation,
            topic: "T".into(),
            instance_handle: Some([0u8; 16]),
            body: alloc::vec![1, 2, 3],
        }
    }

    #[test]
    fn accept_all_returns_accepted_for_every_operation() {
        let d = AcceptAllDispatcher;
        for o in [
            DdsOperation::Write,
            DdsOperation::Register,
            DdsOperation::Unregister,
            DdsOperation::Dispose,
        ] {
            assert_eq!(d.dispatch(&op(o)), DispatchOutcome::Accepted);
        }
    }

    #[test]
    fn instance_tracking_register_with_body_accepts() {
        let d = InstanceTrackingDispatcher::new();
        let r = d.dispatch(&op(DdsOperation::Register));
        assert_eq!(r, DispatchOutcome::Accepted);
    }

    #[test]
    fn instance_tracking_register_empty_body_yields_missing_key() {
        let d = InstanceTrackingDispatcher::new();
        let mut o = op(DdsOperation::Register);
        o.body = alloc::vec![];
        assert_eq!(d.dispatch(&o), DispatchOutcome::RegisterMissingKey);
    }

    #[test]
    fn instance_tracking_unregister_unknown_yields_unknown_instance() {
        let d = InstanceTrackingDispatcher::new();
        let r = d.dispatch(&op(DdsOperation::Unregister));
        assert_eq!(r, DispatchOutcome::UnknownInstance);
    }

    #[test]
    fn instance_tracking_dispose_unknown_yields_unknown_instance() {
        let d = InstanceTrackingDispatcher::new();
        let r = d.dispatch(&op(DdsOperation::Dispose));
        assert_eq!(r, DispatchOutcome::UnknownInstance);
    }

    #[test]
    fn instance_tracking_unregister_known_accepts() {
        let mut d = InstanceTrackingDispatcher::new();
        d.register_instance([1u8; 16]);
        let mut o = op(DdsOperation::Unregister);
        o.instance_handle = Some([1u8; 16]);
        assert_eq!(d.dispatch(&o), DispatchOutcome::Accepted);
    }

    #[test]
    fn dispatch_outcome_to_amqp_error_maps_correctly() {
        // Accepted → kein Error.
        assert!(DispatchOutcome::Accepted.to_amqp_error("k").is_none());
        // UnknownInstance → precondition-failed.
        let e = DispatchOutcome::UnknownInstance
            .to_amqp_error("key-7")
            .unwrap();
        assert_eq!(
            e.condition,
            crate::errors::AmqpErrorCondition::PreconditionFailed
        );
        // RegisterMissingKey → decode-error.
        let e = DispatchOutcome::RegisterMissingKey
            .to_amqp_error("k")
            .unwrap();
        assert_eq!(e.condition, crate::errors::AmqpErrorCondition::DecodeError);
        // UnknownOperation → not-implemented.
        let e = DispatchOutcome::UnknownOperation("teleport".into())
            .to_amqp_error("k")
            .unwrap();
        assert_eq!(
            e.condition,
            crate::errors::AmqpErrorCondition::NotImplemented
        );
    }

    #[test]
    fn noop_disposition_mapper_does_nothing() {
        let m = NoopDispositionMapper;
        // Test ist Pflicht-Smoke: Trait-Aufruf darf nicht panicken.
        m.apply([0u8; 16], DispositionState::Accepted);
        m.apply([0u8; 16], DispositionState::Rejected);
        m.apply([0u8; 16], DispositionState::Released);
        m.apply([0u8; 16], DispositionState::Modified);
    }
}
