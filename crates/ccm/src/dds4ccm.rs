// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS for Lightweight CCM 1.1 — Connector-Stub-Layer.
//!
//! Spec-Quelle: `omg-ccm/dds4ccm-1.1.pdf`. Wir implementieren die
//! Connector-Datenmodelle + Codegen-Datenstrukturen + QoS-Profile-
//! Defaults als Stub-Layer fuer Migrations-Tooling. Die echte
//! Wire-Bindung erfolgt durch den ZeroDDS-DCPS-Stack
//! (`crates/dcps/`).

use alloc::string::String;
use alloc::vec::Vec;

// ===========================================================================
// §7.2.2.1 DDS-DCPS Basic Port Interfaces (Reader/Writer/Updater)
// ===========================================================================

/// Spec §7.2.2.1 — Connector-Port-Kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasicPortKind {
    /// `CCM_DDS::Reader<T>` — DDS-DataReader-Wrapper.
    Reader,
    /// `CCM_DDS::Writer<T>` — DDS-DataWriter-Wrapper.
    Writer,
    /// `CCM_DDS::Updater<T>` — Updater-Variant (write+update Ops).
    Updater,
    /// `CCM_DDS::Getter<T>` — Pull-Style-Getter.
    Getter,
    /// `CCM_DDS::Listener<T>` — Listener-Pattern.
    Listener,
    /// `CCM_DDS::StateListener<T>` — State-Listener-Variant.
    StateListener,
}

/// Spec §7.2.2.1 — Basic-Port-Definition (typed by T).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicPort {
    /// Port-Name (z.B. `"sensor_data"`).
    pub name: String,
    /// Port-Kind.
    pub kind: BasicPortKind,
    /// Type-Parameter T (Repository-ID).
    pub type_id: String,
}

// ===========================================================================
// §7.2.2.2 DDS-DCPS Extended Ports
// ===========================================================================

/// Spec §7.2.2.2 — Extended-Port-Kind (Multi-Topic-/Filtered-Variants).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtendedPortKind {
    /// `MultiTopicReader<T>`.
    MultiTopicReader,
    /// `ContentFilteredReader<T>`.
    ContentFilteredReader,
    /// `QueryConditionReader<T>`.
    QueryConditionReader,
    /// `WaitsetReader<T>`.
    WaitsetReader,
}

/// Spec §7.2.2.2 — Extended-Port-Definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtendedPort {
    /// Port-Name.
    pub name: String,
    /// Port-Kind.
    pub kind: ExtendedPortKind,
    /// Type-Parameter T.
    pub type_id: String,
}

// ===========================================================================
// §7.3 Connectors
// ===========================================================================

/// Spec §7.3.1-§7.3.3 — Connector-Pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorPattern {
    /// `Base` — generischer DCPS-Connector.
    Base,
    /// `StateTransfer` — Pattern §7.3.2 (TransientLocal+KeepLast(1)).
    StateTransfer,
    /// `EventTransfer` — Pattern §7.3.3 (Volatile+KeepAll).
    EventTransfer,
    /// `DLRL` — DLRL-Cache-Connector (Spec §8.3).
    Dlrl,
}

/// Spec §7.3 — Connector-Definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connector {
    /// Connector-Name.
    pub name: String,
    /// Connector-Pattern.
    pub pattern: ConnectorPattern,
    /// Topic-Type (Repository-ID).
    pub type_id: String,
    /// Domain-ID.
    pub domain_id: u32,
    /// QoS-Profile-Name (Cross-Ref §7.4.3).
    pub qos_profile: Option<String>,
    /// Provided Basic-Ports.
    pub basic_ports: Vec<BasicPort>,
    /// Provided Extended-Ports.
    pub extended_ports: Vec<ExtendedPort>,
}

impl Connector {
    /// Konstruktor.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        pattern: ConnectorPattern,
        type_id: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            pattern,
            type_id: type_id.into(),
            domain_id: 0,
            qos_profile: None,
            basic_ports: Vec::new(),
            extended_ports: Vec::new(),
        }
    }

    /// Spec §7.4.3 — QoS-Profile binden.
    pub fn with_qos_profile(mut self, profile: impl Into<String>) -> Self {
        self.qos_profile = Some(profile.into());
        self
    }

    /// Spec §7.4.1 — Domain-ID setzen.
    #[must_use]
    pub fn with_domain(mut self, domain_id: u32) -> Self {
        self.domain_id = domain_id;
        self
    }

    /// Fuegt einen Basic-Port hinzu.
    pub fn add_basic_port(&mut self, p: BasicPort) {
        self.basic_ports.push(p);
    }

    /// Fuegt einen Extended-Port hinzu.
    pub fn add_extended_port(&mut self, p: ExtendedPort) {
        self.extended_ports.push(p);
    }

    /// Anzahl Ports gesamt.
    #[must_use]
    pub fn port_count(&self) -> usize {
        self.basic_ports.len() + self.extended_ports.len()
    }
}

// ===========================================================================
// §7.4.4 Threading Policy
// ===========================================================================

/// Spec §7.4.4 — Threading-Policy fuer Connector-Operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorThreadingPolicy {
    /// `THREAD_PER_CONNECTOR` — separater Thread pro Connector.
    ThreadPerConnector,
    /// `SHARED_THREAD_POOL` — globaler Worker-Pool.
    SharedThreadPool,
    /// `INVOKE_INLINE` — auf dem Caller-Thread.
    InvokeInline,
}

// ===========================================================================
// Annex E — Pattern-spezifische QoS-Profile
// ===========================================================================

/// Spec Annex E — Default-QoS-Profile pro Pattern.
pub mod qos_profiles {
    /// Spec §7.3.2 — State-Transfer-Default: TransientLocal +
    /// KeepLast(1) + Reliable.
    pub const STATE_TRANSFER_DEFAULT: &str = "DDS4CCM:StateTransfer:Default";

    /// Spec §7.3.3 — Event-Transfer-Default: Volatile + KeepAll +
    /// Reliable.
    pub const EVENT_TRANSFER_DEFAULT: &str = "DDS4CCM:EventTransfer:Default";

    /// Spec §7.3.1 — Base-Connector-Default: Volatile + KeepLast(10)
    /// + BestEffort.
    pub const BASE_DEFAULT: &str = "DDS4CCM:Base:Default";

    /// Spec §8.3 — DLRL-Connector-Default.
    pub const DLRL_DEFAULT: &str = "DDS4CCM:DLRL:Default";
}

// ===========================================================================
// §8 DLRL Ports + Connectors
// ===========================================================================

/// Spec §8.2.1.1 / §8.2.1.2 — DLRL-Port-Kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlrlPortKind {
    /// `CacheOperation` (§8.2.1.1).
    CacheOperation,
    /// `DLRLClass` / `ObjectHome` (§8.2.1.2).
    ObjectHome,
}

/// Spec §8 — DLRL-Port-Definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DlrlPort {
    /// Port-Name.
    pub name: String,
    /// Port-Kind.
    pub kind: DlrlPortKind,
    /// Type-Parameter (DLRL-Object-Type).
    pub type_id: String,
}

// ===========================================================================
// Annex A/B — IDL3+ Stub
// ===========================================================================

/// Spec Annex A/B — IDL3+-Output-Form fuer DCPS-/DLRL-Ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdlOutputForm {
    /// IDL3-Compatible (Templated-Modules ausgespiegelt).
    Idl3Compatible,
    /// IDL3+ (mit Templated-Modules).
    Idl3Plus,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn basic_port_kinds_distinct() {
        assert_ne!(BasicPortKind::Reader, BasicPortKind::Writer);
        assert_ne!(BasicPortKind::Updater, BasicPortKind::Getter);
        assert_ne!(BasicPortKind::Listener, BasicPortKind::StateListener);
    }

    #[test]
    fn basic_port_construct() {
        let p = BasicPort {
            name: "sensor".into(),
            kind: BasicPortKind::Reader,
            type_id: "IDL:demo/Sensor:1.0".into(),
        };
        assert_eq!(p.name, "sensor");
    }

    #[test]
    fn extended_port_kinds_distinct() {
        assert_ne!(
            ExtendedPortKind::MultiTopicReader,
            ExtendedPortKind::ContentFilteredReader
        );
    }

    #[test]
    fn connector_construct_default_domain_zero() {
        let c = Connector::new("c", ConnectorPattern::Base, "IDL:T:1.0");
        assert_eq!(c.domain_id, 0);
        assert!(c.qos_profile.is_none());
        assert_eq!(c.port_count(), 0);
    }

    #[test]
    fn connector_with_qos_profile() {
        let c = Connector::new("c", ConnectorPattern::StateTransfer, "IDL:T:1.0")
            .with_qos_profile(qos_profiles::STATE_TRANSFER_DEFAULT)
            .with_domain(42);
        assert_eq!(
            c.qos_profile.as_deref(),
            Some("DDS4CCM:StateTransfer:Default")
        );
        assert_eq!(c.domain_id, 42);
    }

    #[test]
    fn connector_add_basic_port_increments_count() {
        let mut c = Connector::new("c", ConnectorPattern::Base, "IDL:T:1.0");
        c.add_basic_port(BasicPort {
            name: "in".into(),
            kind: BasicPortKind::Reader,
            type_id: "IDL:T:1.0".into(),
        });
        assert_eq!(c.port_count(), 1);
    }

    #[test]
    fn connector_add_extended_port_increments_count() {
        let mut c = Connector::new("c", ConnectorPattern::Base, "IDL:T:1.0");
        c.add_extended_port(ExtendedPort {
            name: "filt".into(),
            kind: ExtendedPortKind::ContentFilteredReader,
            type_id: "IDL:T:1.0".into(),
        });
        assert_eq!(c.port_count(), 1);
    }

    #[test]
    fn connector_pattern_distinct() {
        assert_ne!(ConnectorPattern::Base, ConnectorPattern::StateTransfer);
        assert_ne!(ConnectorPattern::EventTransfer, ConnectorPattern::Dlrl);
    }

    #[test]
    fn threading_policy_distinct() {
        assert_ne!(
            ConnectorThreadingPolicy::ThreadPerConnector,
            ConnectorThreadingPolicy::SharedThreadPool
        );
        assert_ne!(
            ConnectorThreadingPolicy::SharedThreadPool,
            ConnectorThreadingPolicy::InvokeInline
        );
    }

    #[test]
    fn qos_profile_constants_match_spec_namespace() {
        assert!(qos_profiles::STATE_TRANSFER_DEFAULT.starts_with("DDS4CCM:"));
        assert!(qos_profiles::EVENT_TRANSFER_DEFAULT.starts_with("DDS4CCM:"));
        assert!(qos_profiles::BASE_DEFAULT.starts_with("DDS4CCM:"));
        assert!(qos_profiles::DLRL_DEFAULT.starts_with("DDS4CCM:"));
    }

    #[test]
    fn dlrl_port_kinds_distinct() {
        assert_ne!(DlrlPortKind::CacheOperation, DlrlPortKind::ObjectHome);
    }

    #[test]
    fn dlrl_port_construct() {
        let p = DlrlPort {
            name: "trader_cache".into(),
            kind: DlrlPortKind::CacheOperation,
            type_id: "IDL:demo/TraderCache:1.0".into(),
        };
        assert_eq!(p.name, "trader_cache");
    }

    #[test]
    fn idl_output_form_distinct() {
        assert_ne!(IdlOutputForm::Idl3Compatible, IdlOutputForm::Idl3Plus);
    }
}
