// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `RpcQos` — QoS profile resolution for DDS-RPC (Spec §7.11).
//!
//! The DDS-RPC profile (OMG `formal/16-12-04` §7.11) requires for the
//! default foundation configuration of a requester or replier:
//!
//! * **Reliability=Reliable** (Spec §7.11 Tab.7.61) — both writer and
//!   reader. Rationale: a lost reply is semantically
//!   indistinguishable from a never-executed request.
//! * **History=KeepLast(N)** with spec default `N=10` for the reply reader,
//!   `N=10` for the request writer. Users can override the value via a
//!   profile.
//! * **ResourceLimits**: `max_samples=512`, `max_instances=1`,
//!   `max_samples_per_instance=512` — matches the recommendations from
//!   Spec §7.11.2 for single-instance endpoints.
//! * **Lifespan/Deadline/Latency-Budget**: the spec leaves them as
//!   `INFINITE`/`UNSET` — user-overridable.
//!
//! The module provides two builder defaults:
//!
//! * [`RpcQos::default_basic`] — foundation standard.
//! * [`RpcQos::default_enhanced`] — like basic, but reliable with a larger
//!   history buffer (`N=64`) for multi-pending requests.
//!
//! Plus a [`RpcQos::from_xml_profile`] method that resolves a profile under
//! `library::profile` via the XML loader (`zerodds-xml::DdsXml`)
//! and materializes it onto `RpcQos`. Policies not specified in the XML
//! stay at the spec default of the matching foundation mode.

extern crate alloc;

use alloc::string::ToString;

use zerodds_dcps::qos::{DataReaderQos, DataWriterQos};
use zerodds_qos::{
    DeadlineQosPolicy, DurabilityKind, DurabilityQosPolicy, Duration, HistoryKind,
    HistoryQosPolicy, LifespanQosPolicy, ReliabilityKind, ReliabilityQosPolicy,
    ResourceLimitsQosPolicy,
};
use zerodds_xml::{DdsXml, EntityQos, QosLibrary, QosProfile};

use crate::error::{RpcError, RpcResult};

/// Spec-default history depth for the foundation mode (§7.11.2).
pub const DEFAULT_BASIC_HISTORY_DEPTH: i32 = 10;

/// Spec-default history depth for the enhanced mode.
pub const DEFAULT_ENHANCED_HISTORY_DEPTH: i32 = 64;

/// Spec-default resource limits for single-instance RPC endpoints.
pub const DEFAULT_RESOURCE_LIMITS: ResourceLimitsQosPolicy = ResourceLimitsQosPolicy {
    max_samples: 512,
    max_instances: 1,
    max_samples_per_instance: 512,
};

// ---------------------------------------------------------------------
// RpcQos
// ---------------------------------------------------------------------

/// Effective QoS configuration of a requester/replier (Spec §7.11).
///
/// Fields are public — users can patch them directly after the builder.
/// Validation happens when materializing in
/// [`Self::request_writer_qos`] etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcQos {
    /// Reliability for both endpoints. Spec §7.11: Reliable.
    pub reliability: ReliabilityQosPolicy,
    /// Durability for both endpoints. Spec §7.11: Volatile (no
    /// history replay to late-joining peers).
    pub durability: DurabilityQosPolicy,
    /// History for the request writer / request reader.
    pub request_history: HistoryQosPolicy,
    /// History for the reply writer / reply reader.
    pub reply_history: HistoryQosPolicy,
    /// Resource limits — the same for writer and reader.
    pub resource_limits: ResourceLimitsQosPolicy,
    /// Lifespan policy. Default INFINITE.
    pub lifespan: LifespanQosPolicy,
    /// Deadline policy. Default INFINITE.
    pub deadline: DeadlineQosPolicy,
    /// Default timeout for `Requester::send_request_blocking`. Default
    /// 5 seconds.
    pub request_timeout: core::time::Duration,
}

impl RpcQos {
    /// Spec foundation default (Spec §7.11). Reliable + KeepLast(10) +
    /// Volatile + 5 s timeout.
    #[must_use]
    pub fn default_basic() -> Self {
        Self {
            reliability: ReliabilityQosPolicy {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: Duration::from_millis(100),
            },
            durability: DurabilityQosPolicy {
                kind: DurabilityKind::Volatile,
            },
            request_history: HistoryQosPolicy {
                kind: HistoryKind::KeepLast,
                depth: DEFAULT_BASIC_HISTORY_DEPTH,
            },
            reply_history: HistoryQosPolicy {
                kind: HistoryKind::KeepLast,
                depth: DEFAULT_BASIC_HISTORY_DEPTH,
            },
            resource_limits: DEFAULT_RESOURCE_LIMITS,
            lifespan: LifespanQosPolicy::default(),
            deadline: DeadlineQosPolicy::default(),
            request_timeout: core::time::Duration::from_secs(5),
        }
    }

    /// Enhanced mode (Spec §7.11.3). Like basic, but deeper history
    /// (`KeepLast(64)`) for multi-pending requests.
    #[must_use]
    pub fn default_enhanced() -> Self {
        let mut q = Self::default_basic();
        q.request_history.depth = DEFAULT_ENHANCED_HISTORY_DEPTH;
        q.reply_history.depth = DEFAULT_ENHANCED_HISTORY_DEPTH;
        q
    }

    /// Materializes the writer QoS for the **request topic** (sender:
    /// requester, receiver: replier).
    #[must_use]
    pub fn request_writer_qos(&self) -> DataWriterQos {
        DataWriterQos {
            reliability: self.reliability,
            durability: self.durability,
            history: self.request_history,
            resource_limits: self.resource_limits,
            lifespan: self.lifespan,
            deadline: self.deadline,
            ..DataWriterQos::default()
        }
    }

    /// Materializes the reader QoS for the **request topic** (receiver:
    /// replier).
    #[must_use]
    pub fn request_reader_qos(&self) -> DataReaderQos {
        DataReaderQos {
            reliability: self.reliability,
            durability: self.durability,
            history: self.request_history,
            resource_limits: self.resource_limits,
            deadline: self.deadline,
            ..DataReaderQos::default()
        }
    }

    /// Materializes the writer QoS for the **reply topic** (sender:
    /// replier).
    #[must_use]
    pub fn reply_writer_qos(&self) -> DataWriterQos {
        DataWriterQos {
            reliability: self.reliability,
            durability: self.durability,
            history: self.reply_history,
            resource_limits: self.resource_limits,
            lifespan: self.lifespan,
            deadline: self.deadline,
            ..DataWriterQos::default()
        }
    }

    /// Materializes the reader QoS for the **reply topic** (receiver:
    /// requester).
    #[must_use]
    pub fn reply_reader_qos(&self) -> DataReaderQos {
        DataReaderQos {
            reliability: self.reliability,
            durability: self.durability,
            history: self.reply_history,
            resource_limits: self.resource_limits,
            deadline: self.deadline,
            ..DataReaderQos::default()
        }
    }

    /// Resolves a QoS profile from a [`DdsXml`] and merges
    /// its policies over [`Self::default_basic`].
    ///
    /// `path` format: `library::profile` — e.g. `"RpcLib::Calculator"`.
    /// Profiles can set policies via either the `<datawriter_qos>` /
    /// `<datareader_qos>` containers — both are
    /// merged (the reader container overrides the writer for policies that
    /// are not pure writer-only policies).
    ///
    /// Policies not set in the profile stay at
    /// [`Self::default_basic`].
    ///
    /// # Errors
    /// * `RpcError::QosProfileNotFound` if `library` or `profile`
    ///   cannot be resolved or `path` does not have the
    ///   `library::profile` format.
    pub fn from_xml_profile(loader: &DdsXml, path: &str) -> RpcResult<Self> {
        let (lib_name, prof_name) = split_qos_path(path)?;
        let lib = loader
            .qos_libraries
            .iter()
            .find(|l: &&QosLibrary| l.name == lib_name)
            .ok_or_else(|| RpcError::QosProfileNotFound(path.to_string()))?;
        let prof = lib
            .profile(prof_name)
            .ok_or_else(|| RpcError::QosProfileNotFound(path.to_string()))?;
        Ok(Self::from_profile_default(Self::default_basic(), prof))
    }

    /// Like [`Self::from_xml_profile`], but with an explicit default
    /// (e.g. `default_enhanced`).
    #[must_use]
    pub fn from_profile_default(mut base: Self, prof: &QosProfile) -> Self {
        if let Some(eq) = prof.datawriter_qos.as_ref() {
            apply_entity_qos_to_writer_side(&mut base, eq);
        }
        if let Some(eq) = prof.datareader_qos.as_ref() {
            apply_entity_qos_to_reader_side(&mut base, eq);
        }
        base
    }
}

impl Default for RpcQos {
    fn default() -> Self {
        Self::default_basic()
    }
}

// ---------------------------------------------------------------------
// Profile lowering
// ---------------------------------------------------------------------

fn apply_entity_qos_to_writer_side(q: &mut RpcQos, eq: &EntityQos) {
    if let Some(p) = eq.reliability {
        q.reliability = p;
    }
    if let Some(p) = eq.durability {
        q.durability = p;
    }
    if let Some(p) = eq.history {
        // Writes onto the request writer and reply writer.
        q.request_history = p;
        q.reply_history = p;
    }
    if let Some(p) = eq.resource_limits {
        q.resource_limits = p;
    }
    if let Some(p) = eq.lifespan {
        q.lifespan = p;
    }
    if let Some(p) = eq.deadline {
        q.deadline = p;
    }
}

fn apply_entity_qos_to_reader_side(q: &mut RpcQos, eq: &EntityQos) {
    // The reader container overrides only reader-relevant policies.
    if let Some(p) = eq.reliability {
        q.reliability = p;
    }
    if let Some(p) = eq.durability {
        q.durability = p;
    }
    if let Some(p) = eq.history {
        q.request_history = p;
        q.reply_history = p;
    }
    if let Some(p) = eq.resource_limits {
        q.resource_limits = p;
    }
    if let Some(p) = eq.deadline {
        q.deadline = p;
    }
    // `lifespan` is writer-only (Spec §2.2.3.16) → ignore.
}

fn split_qos_path(path: &str) -> RpcResult<(&str, &str)> {
    let (lib, prof) = path
        .split_once("::")
        .ok_or_else(|| RpcError::QosProfileNotFound(path.to_string()))?;
    if lib.is_empty() || prof.is_empty() {
        return Err(RpcError::QosProfileNotFound(path.to_string()));
    }
    Ok((lib, prof))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use zerodds_xml::parse_dds_xml;

    #[test]
    fn default_basic_matches_spec_711() {
        let q = RpcQos::default_basic();
        assert_eq!(q.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(q.durability.kind, DurabilityKind::Volatile);
        assert_eq!(q.request_history.kind, HistoryKind::KeepLast);
        assert_eq!(q.request_history.depth, DEFAULT_BASIC_HISTORY_DEPTH);
        assert_eq!(q.reply_history.depth, DEFAULT_BASIC_HISTORY_DEPTH);
        assert_eq!(q.resource_limits.max_samples, 512);
        assert_eq!(q.resource_limits.max_instances, 1);
        assert_eq!(q.resource_limits.max_samples_per_instance, 512);
        assert_eq!(q.request_timeout, core::time::Duration::from_secs(5));
    }

    #[test]
    fn default_enhanced_uses_deeper_history() {
        let q = RpcQos::default_enhanced();
        assert_eq!(q.request_history.depth, DEFAULT_ENHANCED_HISTORY_DEPTH);
        assert_eq!(q.reply_history.depth, DEFAULT_ENHANCED_HISTORY_DEPTH);
        // Remaining policies as in basic.
        assert_eq!(q.reliability.kind, ReliabilityKind::Reliable);
    }

    #[test]
    fn default_is_basic() {
        assert_eq!(RpcQos::default(), RpcQos::default_basic());
    }

    #[test]
    fn writer_qos_carries_reliability_and_history() {
        let q = RpcQos::default_basic();
        let wq = q.request_writer_qos();
        assert_eq!(wq.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(wq.history.depth, DEFAULT_BASIC_HISTORY_DEPTH);
        assert_eq!(wq.resource_limits.max_samples, 512);
    }

    #[test]
    fn reader_qos_carries_reliability() {
        let q = RpcQos::default_basic();
        let rq = q.reply_reader_qos();
        assert_eq!(rq.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(rq.history.depth, DEFAULT_BASIC_HISTORY_DEPTH);
    }

    #[test]
    fn split_qos_path_accepts_double_colon() {
        let (l, p) = split_qos_path("Lib::Prof").unwrap();
        assert_eq!(l, "Lib");
        assert_eq!(p, "Prof");
    }

    #[test]
    fn split_qos_path_rejects_missing_separator() {
        assert!(matches!(
            split_qos_path("nope"),
            Err(RpcError::QosProfileNotFound(_))
        ));
    }

    #[test]
    fn split_qos_path_rejects_empty_parts() {
        assert!(matches!(
            split_qos_path("::Prof"),
            Err(RpcError::QosProfileNotFound(_))
        ));
        assert!(matches!(
            split_qos_path("Lib::"),
            Err(RpcError::QosProfileNotFound(_))
        ));
    }

    #[test]
    fn from_xml_profile_overrides_history_depth() {
        let xml = r#"<dds>
            <qos_library name="RpcLib">
                <qos_profile name="Calculator">
                    <datawriter_qos>
                        <history>
                            <kind>KEEP_LAST_HISTORY_QOS</kind>
                            <depth>32</depth>
                        </history>
                    </datawriter_qos>
                </qos_profile>
            </qos_library>
        </dds>"#;
        let loader = parse_dds_xml(xml).unwrap();
        let q = RpcQos::from_xml_profile(&loader, "RpcLib::Calculator").unwrap();
        assert_eq!(q.request_history.depth, 32);
        assert_eq!(q.reply_history.depth, 32);
    }

    #[test]
    fn from_xml_profile_unknown_library_errors() {
        let xml = r#"<dds><qos_library name="A"><qos_profile name="B"/></qos_library></dds>"#;
        let loader = parse_dds_xml(xml).unwrap();
        let err = RpcQos::from_xml_profile(&loader, "Missing::B").unwrap_err();
        assert!(matches!(err, RpcError::QosProfileNotFound(_)));
    }

    #[test]
    fn from_xml_profile_unknown_profile_errors() {
        let xml = r#"<dds><qos_library name="A"><qos_profile name="X"/></qos_library></dds>"#;
        let loader = parse_dds_xml(xml).unwrap();
        let err = RpcQos::from_xml_profile(&loader, "A::Missing").unwrap_err();
        assert!(matches!(err, RpcError::QosProfileNotFound(_)));
    }

    #[test]
    fn from_xml_profile_malformed_path_errors() {
        let loader = DdsXml::default();
        let err = RpcQos::from_xml_profile(&loader, "no-colon").unwrap_err();
        assert!(matches!(err, RpcError::QosProfileNotFound(_)));
    }
}
