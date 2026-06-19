// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS bridge trait surface.
//!
//! Spec dds-amqp-1.0:
//! * §7.7.2 Inbound Operation Signals — dispatch `dds:operation`
//!   values (`write/register/unregister/dispose`) onto
//!   DataWriter method calls.
//! * §7.7.3 Disposition Mapping — AMQP disposition onto a
//!   DDS sample-state update.
//! * §11.3 Instance-Lifecycle Failures — errors during lifecycle
//!   handling must be mapped to AMQP errors per spec.
//!
//! The endpoint crate itself ships no DDS DataWriter/Reader
//! implementation (that is the DCPS crate). We define the
//! trait surface that the daemon (or a test mock) binds against
//! — analogous to [`crate::security::AccessControlPlugin`].

use alloc::string::String;
use alloc::vec::Vec;

use crate::errors::{AmqpError, instance_unknown, register_missing_key, unknown_dds_operation};
use crate::properties::DdsOperation;

// ============================================================
// §7.7.2 — inbound operation dispatcher
// ============================================================

/// Input for an inbound operation dispatch call.
#[derive(Debug, Clone)]
pub struct InboundOperation {
    /// `dds:operation` from the application properties.
    pub operation: DdsOperation,
    /// Topic name to dispatch against.
    pub topic: String,
    /// `dds:instance-handle` (16 byte) — identifies the instance.
    /// For `register` derived from the body, for
    /// `unregister`/`dispose` from this property.
    pub instance_handle: Option<[u8; 16]>,
    /// Sample body bytes (XCDR2 / JSON / native).
    pub body: Vec<u8>,
}

/// Spec §11.3 — result of a lifecycle dispatch operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// Operation was successfully reported to the DDS DataWriter.
    Accepted,
    /// `unregister` / `dispose` on an unknown instance —
    /// `amqp:precondition-failed`.
    UnknownInstance,
    /// `register` without key fields in the body —
    /// `amqp:decode-error`.
    RegisterMissingKey,
    /// Unknown operation value (spec §11.2 →
    /// `amqp:not-implemented`).
    UnknownOperation(String),
}

impl DispatchOutcome {
    /// Spec §11.2 + §11.3 — map the outcome to an `AmqpError`.
    /// Returns `None` when the outcome is `Accepted`.
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

/// Spec §7.7.2 — DDS bridge adapter that dispatches an inbound
/// operation onto a DDS DataWriter method call.
///
/// Implementers:
/// * Daemon variant with a real DCPS bridge.
/// * Test mock for conformance tests.
pub trait DdsOperationDispatcher {
    /// Dispatch the parsed operation.
    ///
    /// # Errors / Outcome
    /// Returns `DispatchOutcome::Accepted` on success, otherwise
    /// a spec-conformant error outcome.
    fn dispatch(&self, op: &InboundOperation) -> DispatchOutcome;
}

/// No-op dispatcher that accepts all operations.
/// Test default; **not** for production.
#[derive(Debug, Default)]
pub struct AcceptAllDispatcher;

impl DdsOperationDispatcher for AcceptAllDispatcher {
    fn dispatch(&self, _op: &InboundOperation) -> DispatchOutcome {
        DispatchOutcome::Accepted
    }
}

/// Dispatcher that maintains a list of registered instance handles
/// and emits §11.3 spec errors.
#[derive(Debug, Default)]
pub struct InstanceTrackingDispatcher {
    known: alloc::collections::BTreeSet<[u8; 16]>,
}

impl InstanceTrackingDispatcher {
    /// Fresh tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of known instances (test helper).
    #[must_use]
    pub fn known_count(&self) -> usize {
        self.known.len()
    }
}

impl DdsOperationDispatcher for InstanceTrackingDispatcher {
    fn dispatch(&self, op: &InboundOperation) -> DispatchOutcome {
        match op.operation {
            DdsOperation::Register => {
                // Spec §11.3 — register WITHOUT key fields in the
                // body is a decode error.
                if op.body.is_empty() {
                    return DispatchOutcome::RegisterMissingKey;
                }
                DispatchOutcome::Accepted
            }
            DdsOperation::Unregister | DdsOperation::Dispose => {
                // Spec §11.3 — unknown instance → precondition-
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
    /// Test helper: register an instance.
    pub fn register_instance(&mut self, handle: [u8; 16]) {
        self.known.insert(handle);
    }
}

// ============================================================
// §7.7.3 — disposition mapping
// ============================================================

/// Spec §3.4 + §7.7.3 — AMQP disposition state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispositionState {
    /// Spec §3.4.2 — `accepted`.
    Accepted,
    /// Spec §3.4.3 — `rejected` (with an error condition).
    Rejected,
    /// Spec §3.4.4 — `released`.
    Released,
    /// Spec §3.4.5 — `modified`.
    Modified,
}

/// Spec §7.7.3 — DDS-side sample-state update from an AMQP
/// disposition result.
///
/// Wireup path: [`crate::link::LinkSession::settle_with_mapper`] calls
/// this trait when receiving an AMQP disposition frame with the
/// decoded sample handle and [`DispositionState`].
///
/// Caller implementers (typically a DCPS bridge):
/// * `accepted` → `acknowledged()` on the DDS DataWriter.
/// * `rejected` / `released` → `unacknowledged()`.
/// * `modified` → typically handled like `rejected`.
///
/// For AMQP-only workflows without a DDS bridge:
/// use [`NoopDispositionMapper`] OR call
/// [`crate::link::LinkSession::settle`] (counter-only).
pub trait DispositionMapper {
    /// Applies a disposition to the DDS-side DataWriter.
    fn apply(&self, sample_handle: [u8; 16], state: DispositionState);
}

/// Null-object default for AMQP-only workflows without a DDS-side
/// sample-state update.
///
/// Callers with a DDS bridge implement `DispositionMapper` themselves
/// and pass it to
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
        // Accepted → no error.
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
        // Mandatory smoke test: the trait call must not panic.
        m.apply([0u8; 16], DispositionState::Accepted);
        m.apply([0u8; 16], DispositionState::Rejected);
        m.apply([0u8; 16], DispositionState::Released);
        m.apply([0u8; 16], DispositionState::Modified);
    }
}
