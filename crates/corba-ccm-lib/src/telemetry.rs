// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! TelemetryComponent — component lifecycle metric emitter.
//!
//! Production CCM containers need observability: every
//! lifecycle transition (`set_context`, `ccm_activate`,
//! `ccm_passivate`, `ccm_remove`) is published as an event to a DCPS
//! topic `__ZeroDDS_CcmTelemetry`. This component provides the
//! in-process sample model and lifecycle hooks; the topic publication
//! is the responsibility of the DDS runtime.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_corba_ccm::cif::{CifError, ComponentExecutor};
use zerodds_corba_ccm::context::ComponentContext;

/// Kind of a telemetry event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryKind {
    /// `set_context` called.
    SetContext,
    /// `ccm_activate` called.
    Activate,
    /// `ccm_passivate` called.
    Passivate,
    /// `ccm_remove` called.
    Remove,
    /// User-defined event (caller emits via `record_custom`).
    Custom,
}

/// Telemetry event sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryEvent {
    /// Ascending sequence number (starts at 1).
    pub sequence: u64,
    /// Kind.
    pub kind: TelemetryKind,
    /// Free-form label (e.g. component name or custom tag).
    pub label: String,
}

/// TelemetryComponent — production-ready CCM component.
pub struct TelemetryComponent {
    name: String,
    events: Vec<TelemetryEvent>,
    seq: u64,
    activated: bool,
    ctx: Option<Box<dyn ComponentContext>>,
}

impl Default for TelemetryComponent {
    fn default() -> Self {
        Self::new("anonymous")
    }
}

impl core::fmt::Debug for TelemetryComponent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TelemetryComponent")
            .field("name", &self.name)
            .field("event_count", &self.events.len())
            .field("activated", &self.activated)
            .finish_non_exhaustive()
    }
}

impl TelemetryComponent {
    /// Constructor with component name (used as label prefix).
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            events: Vec::new(),
            seq: 0,
            activated: false,
            ctx: None,
        }
    }

    fn record(&mut self, kind: TelemetryKind, label: String) {
        self.seq += 1;
        self.events.push(TelemetryEvent {
            sequence: self.seq,
            kind,
            label,
        });
    }

    /// Records a custom event. The caller layer (e.g.
    /// component logic) calls this to emit app-specific telemetry.
    pub fn record_custom(&mut self, label: String) {
        self.record(TelemetryKind::Custom, label);
    }

    /// List of all events emitted so far (in order).
    #[must_use]
    pub fn events(&self) -> &[TelemetryEvent] {
        &self.events
    }

    /// Number of events of a given kind.
    #[must_use]
    pub fn count_of(&self, kind: TelemetryKind) -> usize {
        self.events.iter().filter(|e| e.kind == kind).count()
    }

    /// Last event (None if empty).
    #[must_use]
    pub fn last_event(&self) -> Option<&TelemetryEvent> {
        self.events.last()
    }

    /// Drain — empties the event queue and returns it. Useful
    /// when the DDS runtime reads out the buffer.
    pub fn drain(&mut self) -> Vec<TelemetryEvent> {
        core::mem::take(&mut self.events)
    }

    /// Returns `true` if activated.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.activated
    }

    /// Component name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl ComponentExecutor for TelemetryComponent {
    fn set_context(&mut self, context: Box<dyn ComponentContext>) {
        self.ctx = Some(context);
        let label = self.name.clone();
        self.record(TelemetryKind::SetContext, label);
    }

    fn ccm_activate(&mut self) -> Result<(), CifError> {
        self.activated = true;
        let label = self.name.clone();
        self.record(TelemetryKind::Activate, label);
        Ok(())
    }

    fn ccm_passivate(&mut self) -> Result<(), CifError> {
        self.activated = false;
        let label = self.name.clone();
        self.record(TelemetryKind::Passivate, label);
        Ok(())
    }

    fn ccm_remove(&mut self) -> Result<(), CifError> {
        self.activated = false;
        let label = self.name.clone();
        self.record(TelemetryKind::Remove, label);
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    struct AnonContext;
    impl ComponentContext for AnonContext {
        fn get_caller_principal(&self) -> Option<Vec<u8>> {
            None
        }
    }

    #[test]
    fn fresh_component_has_no_events() {
        let t = TelemetryComponent::new("comp");
        assert!(t.events().is_empty());
    }

    #[test]
    fn full_lifecycle_records_four_events() {
        let mut t = TelemetryComponent::new("comp");
        t.set_context(Box::new(AnonContext));
        t.ccm_activate().unwrap();
        t.ccm_passivate().unwrap();
        t.ccm_remove().unwrap();
        assert_eq!(t.events().len(), 4);
        assert_eq!(t.count_of(TelemetryKind::SetContext), 1);
        assert_eq!(t.count_of(TelemetryKind::Activate), 1);
        assert_eq!(t.count_of(TelemetryKind::Passivate), 1);
        assert_eq!(t.count_of(TelemetryKind::Remove), 1);
    }

    #[test]
    fn sequence_numbers_strictly_increase() {
        let mut t = TelemetryComponent::new("c");
        t.set_context(Box::new(AnonContext));
        t.ccm_activate().unwrap();
        t.ccm_passivate().unwrap();
        let seqs: alloc::vec::Vec<u64> = t.events().iter().map(|e| e.sequence).collect();
        assert_eq!(seqs, alloc::vec![1, 2, 3]);
    }

    #[test]
    fn record_custom_adds_event() {
        let mut t = TelemetryComponent::new("c");
        t.record_custom("user_logged_in".into());
        assert_eq!(t.events().len(), 1);
        assert_eq!(t.last_event().unwrap().kind, TelemetryKind::Custom);
        assert_eq!(t.last_event().unwrap().label, "user_logged_in");
    }

    #[test]
    fn drain_empties_buffer() {
        let mut t = TelemetryComponent::new("c");
        t.ccm_activate().unwrap();
        t.ccm_passivate().unwrap();
        let drained = t.drain();
        assert_eq!(drained.len(), 2);
        assert!(t.events().is_empty());
    }

    #[test]
    fn label_uses_component_name() {
        let mut t = TelemetryComponent::new("MyComp");
        t.ccm_activate().unwrap();
        assert_eq!(t.last_event().unwrap().label, "MyComp");
    }

    #[test]
    fn telemetry_kinds_are_distinct() {
        for (a, b) in [
            (TelemetryKind::SetContext, TelemetryKind::Activate),
            (TelemetryKind::Activate, TelemetryKind::Passivate),
            (TelemetryKind::Passivate, TelemetryKind::Remove),
            (TelemetryKind::Remove, TelemetryKind::Custom),
        ] {
            assert_ne!(a, b);
        }
    }

    #[test]
    fn activate_then_passivate_toggles_active_flag() {
        let mut t = TelemetryComponent::new("c");
        t.ccm_activate().unwrap();
        assert!(t.is_active());
        t.ccm_passivate().unwrap();
        assert!(!t.is_active());
    }
}
