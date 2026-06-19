// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Configuration of the monitor subsystem (spec §6, §9).

use std::sync::OnceLock;

/// When is `PID_VENDOR_TRACE_CONTEXT` embedded into outgoing samples?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceContextEmission {
    /// On every sample, whether sampled or not.
    Always,
    /// Only when the current span context carries `sampled=true`
    /// (default — respects the OTel sampler decision).
    Sampled,
    /// Never.
    Never,
}

impl Default for TraceContextEmission {
    fn default() -> Self {
        Self::Sampled
    }
}

/// Stable short hash (FNV-1a, 64-bit → 16 hex chars) for the
/// `Hashed` policy variants. Deterministic and stable across processes
/// (no random seed as with `DefaultHasher`).
fn short_hash(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Policy for the `topic` label of the DCPS metrics (spec §9, §2.3) — bounds the
/// Prometheus label cardinality of end-user topic names. Dyn-free (enum instead
/// of `Box<dyn Fn>`), `Copy`, deterministic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopicLabelPolicy {
    /// Topic name unchanged (default — 1.0 behavior).
    Full,
    /// Truncate the topic label to at most `n` characters.
    Truncate(usize),
    /// Stable short hash instead of plaintext.
    Hashed,
    /// Omit the `topic` label entirely (maximum cardinality protection).
    Drop,
}

impl Default for TopicLabelPolicy {
    fn default() -> Self {
        Self::Full
    }
}

impl TopicLabelPolicy {
    /// Applies the policy to a topic name. `None` = omit the label
    /// (`Drop`); `Some(v)` = the (possibly transformed) label value.
    pub fn apply(&self, topic: &str) -> Option<String> {
        match self {
            Self::Full => Some(topic.to_string()),
            Self::Truncate(n) => Some(topic.chars().take(*n).collect()),
            Self::Hashed => Some(short_hash(topic)),
            Self::Drop => None,
        }
    }
}

/// Policy for GUID-bearing span attributes (spec §9, §5) — controls the
/// exposure of endpoint GUIDs on export. Dyn-free, `Copy`, deterministic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuidLabelPolicy {
    /// GUID as hex (default — sensible on trusted networks).
    Full,
    /// Stable short hash of the GUID.
    Hashed,
    /// Omit the GUID attribute.
    Omit,
}

impl Default for GuidLabelPolicy {
    fn default() -> Self {
        Self::Full
    }
}

impl GuidLabelPolicy {
    /// Applies the policy to a GUID (hex string). `None` = omit the attribute
    /// (`Omit`); `Some(v)` = the (possibly hashed) value.
    pub fn apply(&self, guid_hex: &str) -> Option<String> {
        match self {
            Self::Full => Some(guid_hex.to_string()),
            Self::Hashed => Some(short_hash(guid_hex)),
            Self::Omit => None,
        }
    }
}

/// Lifecycle configuration (spec §6.2).
#[derive(Clone, Copy, Debug)]
pub struct MonitorConfig {
    /// Trace-Context-Emit-Modus.
    pub emit_trace_context: TraceContextEmission,
    /// Receiver-Side: PID 0x0D00 entgegennehmen?
    pub accept_trace_context: bool,
    /// Metric registry enabled?
    pub enable_metrics: bool,
    /// Cardinality policy for `topic` labels (spec §9).
    pub topic_label_policy: TopicLabelPolicy,
    /// Redaction policy for GUID span attributes (spec §9).
    pub guid_label_policy: GuidLabelPolicy,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            emit_trace_context: TraceContextEmission::default(),
            accept_trace_context: true,
            enable_metrics: true,
            topic_label_policy: TopicLabelPolicy::default(),
            guid_label_policy: GuidLabelPolicy::default(),
        }
    }
}

static CONFIG: OnceLock<MonitorConfig> = OnceLock::new();

/// Sets the process-wide monitor configuration. Set-once (like the
/// default registry); a second call returns the config as `Err`.
pub fn set_config(cfg: MonitorConfig) -> Result<(), MonitorConfig> {
    CONFIG.set(cfg)
}

/// The active configuration (set via [`set_config`], otherwise `Default`).
pub fn active_config() -> MonitorConfig {
    CONFIG.get().copied().unwrap_or_default()
}

/// Applies the active [`TopicLabelPolicy`] to a topic name. The
/// canonical application point for DCPS metric topic labels (spec §9, §2.3).
pub fn topic_label(topic: &str) -> Option<String> {
    active_config().topic_label_policy.apply(topic)
}

/// Applies the active [`GuidLabelPolicy`] to a GUID (hex). The canonical
/// application point for GUID span attributes (spec §9, §5).
pub fn guid_label(guid_hex: &str) -> Option<String> {
    active_config().guid_label_policy.apply(guid_hex)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let c = MonitorConfig::default();
        assert_eq!(c.emit_trace_context, TraceContextEmission::Sampled);
        assert!(c.accept_trace_context);
        assert!(c.enable_metrics);
        assert_eq!(c.topic_label_policy, TopicLabelPolicy::Full);
        assert_eq!(c.guid_label_policy, GuidLabelPolicy::Full);
    }

    #[test]
    fn topic_policy_full_is_identity() {
        assert_eq!(
            TopicLabelPolicy::Full.apply("Vehicle.Track"),
            Some("Vehicle.Track".to_string())
        );
    }

    #[test]
    fn topic_policy_truncate_caps_length() {
        assert_eq!(
            TopicLabelPolicy::Truncate(7).apply("VehicleTracking.TrackUpdate"),
            Some("Vehicle".to_string())
        );
    }

    #[test]
    fn topic_policy_hashed_is_stable_and_hex() {
        let a = TopicLabelPolicy::Hashed
            .apply("Telemetry.Heartbeat")
            .unwrap();
        let b = TopicLabelPolicy::Hashed
            .apply("Telemetry.Heartbeat")
            .unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(
            a,
            TopicLabelPolicy::Hashed.apply("Telemetry.Other").unwrap()
        );
    }

    #[test]
    fn topic_policy_drop_omits_label() {
        assert_eq!(TopicLabelPolicy::Drop.apply("anything"), None);
    }

    #[test]
    fn guid_policy_variants() {
        let g = "0102030405060708090a0b0c0d0e0f10";
        assert_eq!(GuidLabelPolicy::Full.apply(g), Some(g.to_string()));
        assert_eq!(GuidLabelPolicy::Omit.apply(g), None);
        let h = GuidLabelPolicy::Hashed.apply(g).unwrap();
        assert_eq!(h.len(), 16);
        assert_ne!(h, g);
    }

    #[test]
    fn active_config_defaults_when_unset_helpers_apply() {
        // Without set_config the default (Full) applies → identity.
        assert_eq!(topic_label("X.Y"), Some("X.Y".to_string()));
        assert_eq!(
            guid_label("00112233445566778899aabbccddeeff"),
            Some("00112233445566778899aabbccddeeff".to_string())
        );
    }
}
