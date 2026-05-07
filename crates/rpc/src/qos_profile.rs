// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `RpcQos` — QoS-Profile-Resolution fuer DDS-RPC (Spec §7.11).
//!
//! Das DDS-RPC-Profile (OMG `formal/16-12-04` §7.11) verlangt fuer die
//! Default-Foundation-Konfiguration eines Requesters bzw. Repliers:
//!
//! * **Reliability=Reliable** (Spec §7.11 Tab.7.61) — sowohl Writer als
//!   auch Reader. Begruendung: ein verlorenes Reply ist semantisch
//!   nicht von einem nie ausgefuehrten Request unterscheidbar.
//! * **History=KeepLast(N)** mit Spec-Default `N=10` fuer Reply-Reader,
//!   `N=10` fuer Request-Writer. User koennen den Wert per Profile
//!   ueberschreiben.
//! * **ResourceLimits**: `max_samples=512`, `max_instances=1`,
//!   `max_samples_per_instance=512` — entspricht den Empfehlungen aus
//!   Spec §7.11.2 fuer Single-Instance-Endpoints.
//! * **Lifespan/Deadline/Latency-Budget**: Spec laesst sie als
//!   `INFINITE`/`UNSET` — User-overridable.
//!
//! Das Modul liefert zwei Builder-Defaults:
//!
//! * [`RpcQos::default_basic`] — Foundation-Standard.
//! * [`RpcQos::default_enhanced`] — wie Basic, aber Reliable mit groesserem
//!   History-Buffer (`N=64`) fuer Multi-Pending-Requests.
//!
//! Plus eine [`RpcQos::from_xml_profile`]-Methode, die ueber den
//! XML-Loader (`zerodds-xml::DdsXml`) ein Profile unter `library::profile`
//! aufloest und auf `RpcQos` materialisiert. Nicht im XML angegebene
//! Policies bleiben auf den Spec-Default des passenden Foundation-Modus.

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

/// Spec-Default-History-Depth fuer den Foundation-Modus (§7.11.2).
pub const DEFAULT_BASIC_HISTORY_DEPTH: i32 = 10;

/// Spec-Default-History-Depth fuer den Enhanced-Modus.
pub const DEFAULT_ENHANCED_HISTORY_DEPTH: i32 = 64;

/// Spec-Default-Resource-Limits fuer Single-Instance-RPC-Endpoints.
pub const DEFAULT_RESOURCE_LIMITS: ResourceLimitsQosPolicy = ResourceLimitsQosPolicy {
    max_samples: 512,
    max_instances: 1,
    max_samples_per_instance: 512,
};

// ---------------------------------------------------------------------
// RpcQos
// ---------------------------------------------------------------------

/// Effektive QoS-Konfiguration eines Requesters/Repliers (Spec §7.11).
///
/// Felder sind public — User koennen sie nach dem Builder direkt
/// patchen. Validation findet beim Materialisieren in
/// [`Self::request_writer_qos`] etc. statt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcQos {
    /// Reliability fuer beide Endpoints. Spec §7.11: Reliable.
    pub reliability: ReliabilityQosPolicy,
    /// Durability fuer beide Endpoints. Spec §7.11: Volatile (kein
    /// History-Replay an spaet-joinende Peers).
    pub durability: DurabilityQosPolicy,
    /// History fuer Request-Writer / Request-Reader.
    pub request_history: HistoryQosPolicy,
    /// History fuer Reply-Writer / Reply-Reader.
    pub reply_history: HistoryQosPolicy,
    /// Resource-Limits — fuer Writer und Reader gleich.
    pub resource_limits: ResourceLimitsQosPolicy,
    /// Lifespan-Policy. Default INFINITE.
    pub lifespan: LifespanQosPolicy,
    /// Deadline-Policy. Default INFINITE.
    pub deadline: DeadlineQosPolicy,
    /// Default-Timeout fuer `Requester::send_request_blocking`. Default
    /// 5 Sekunden.
    pub request_timeout: core::time::Duration,
}

impl RpcQos {
    /// Spec-Foundation-Default (Spec §7.11). Reliable + KeepLast(10) +
    /// Volatile + 5-s-Timeout.
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

    /// Enhanced-Modus (Spec §7.11.3). Wie Basic, aber tiefere History
    /// (`KeepLast(64)`) fuer Multi-Pending-Requests.
    #[must_use]
    pub fn default_enhanced() -> Self {
        let mut q = Self::default_basic();
        q.request_history.depth = DEFAULT_ENHANCED_HISTORY_DEPTH;
        q.reply_history.depth = DEFAULT_ENHANCED_HISTORY_DEPTH;
        q
    }

    /// Materialisiert die Writer-QoS fuer den **Request-Topic** (Sender:
    /// Requester, Empfaenger: Replier).
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

    /// Materialisiert die Reader-QoS fuer den **Request-Topic** (Empfaenger:
    /// Replier).
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

    /// Materialisiert die Writer-QoS fuer den **Reply-Topic** (Sender:
    /// Replier).
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

    /// Materialisiert die Reader-QoS fuer den **Reply-Topic** (Empfaenger:
    /// Requester).
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

    /// Loest ein QoS-Profile aus einem [`DdsXml`] auf und merged
    /// dessen Policies ueber [`Self::default_basic`].
    ///
    /// `path` Format: `library::profile` — z.B. `"RpcLib::Calculator"`.
    /// Profile koennen entweder ueber `<datawriter_qos>` /
    /// `<datareader_qos>`-Container Policies setzen — beide werden
    /// gemergt (Reader-Container ueberschreibt Writer in Policies, die
    /// nicht reine Writer-only-Policies sind).
    ///
    /// Nicht-im-Profile-gesetzte Policies bleiben auf
    /// [`Self::default_basic`].
    ///
    /// # Errors
    /// * `RpcError::QosProfileNotFound` wenn `library` oder `profile`
    ///   nicht aufgeloest werden kann oder `path` nicht das
    ///   `library::profile`-Format hat.
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

    /// Wie [`Self::from_xml_profile`], aber mit explizitem Default
    /// (z.B. `default_enhanced`).
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
// Profile-Lowering
// ---------------------------------------------------------------------

fn apply_entity_qos_to_writer_side(q: &mut RpcQos, eq: &EntityQos) {
    if let Some(p) = eq.reliability {
        q.reliability = p;
    }
    if let Some(p) = eq.durability {
        q.durability = p;
    }
    if let Some(p) = eq.history {
        // Schreibt sich auf Request-Writer und Reply-Writer.
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
    // Reader-Container ueberschreibt nur Reader-relevante Policies.
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
    // `lifespan` ist Writer-only (Spec §2.2.3.16) → ignorieren.
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
        // Restliche Policies wie Basic.
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
