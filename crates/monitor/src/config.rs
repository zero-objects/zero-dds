// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Konfiguration des Monitor-Subsystems (Spec §6).

/// Wann wird `PID_VENDOR_TRACE_CONTEXT` in ausgehende Samples
/// eingebettet?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceContextEmission {
    /// Bei jedem Sample, ob gesampelt oder nicht.
    Always,
    /// Nur wenn der aktuelle Span-Kontext `sampled=true` traegt
    /// (Default — respektiert die OTel-Sampler-Decision).
    Sampled,
    /// Niemals.
    Never,
}

impl Default for TraceContextEmission {
    fn default() -> Self {
        Self::Sampled
    }
}

/// Lifecycle-Konfiguration.
#[derive(Clone, Debug)]
pub struct MonitorConfig {
    /// Trace-Context-Emit-Modus.
    pub emit_trace_context: TraceContextEmission,
    /// Receiver-Side: PID 0x0D00 entgegennehmen?
    pub accept_trace_context: bool,
    /// Metric-Registry aktiviert?
    pub enable_metrics: bool,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            emit_trace_context: TraceContextEmission::default(),
            accept_trace_context: true,
            enable_metrics: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let c = MonitorConfig::default();
        assert_eq!(c.emit_trace_context, TraceContextEmission::Sampled);
        assert!(c.accept_trace_context);
        assert!(c.enable_metrics);
    }
}
