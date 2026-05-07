// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DdsBridgeComponent — bidirektionale CCM↔DDS-Bridge.
//!
//! Spec-Refs:
//! * CCM 4.0 §6.4 (EventSource/EventSink Ports)
//! * DDS 1.4 §2.2.2 (DCPS Topic/Publisher/Subscriber)
//!
//! Diese Component mappt:
//!
//! * CCM-EventSink → DDS-DataReader (publishing in DDS-Topic)
//! * CCM-EventSource → DDS-DataWriter (forwarding from DDS-Topic)
//!
//! Die konkreten DCPS-Calls macht die ausgewaehlte DDS-Runtime
//! (`zerodds-dcps`); diese Component liefert nur das Mapping-Modell und
//! die Lifecycle-Hooks.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_corba_ccm::cif::{CifError, ComponentExecutor};
use zerodds_corba_ccm::context::ComponentContext;

/// Mapping-Direction: Component-Port-zu-DDS oder DDS-zu-Component-Port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingDirection {
    /// CCM-EventSink → DDS-DataReader (Subscriber).
    SinkSubscribesTopic,
    /// CCM-EventSource → DDS-DataWriter (Publisher).
    SourcePublishesTopic,
}

/// Eine konkrete Bridge-Mapping-Definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicMapping {
    /// CCM-Port-Name (Source oder Sink).
    pub port_name: String,
    /// DDS-Topic-Name.
    pub topic_name: String,
    /// Type-Name (vollqualifiziert).
    pub type_name: String,
    /// Mapping-Direction.
    pub direction: MappingDirection,
}

/// Bridge-Component-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    /// Mapping mit gleichem Port-Name existiert bereits.
    DuplicatePort(String),
    /// Component nicht aktiviert.
    NotActive,
    /// Generic CCM-Error.
    Cif(CifError),
}

impl core::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DuplicatePort(s) => write!(f, "duplicate port `{s}`"),
            Self::NotActive => f.write_str("component not active"),
            Self::Cif(e) => write!(f, "cif: {e:?}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BridgeError {}

/// DDS-Bridge-Component — production-ready CCM-Component.
#[derive(Default)]
pub struct DdsBridgeComponent {
    mappings: BTreeMap<String, TopicMapping>,
    activated: bool,
    ctx: Option<Box<dyn ComponentContext>>,
}

impl core::fmt::Debug for DdsBridgeComponent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DdsBridgeComponent")
            .field("mappings", &self.mappings)
            .field("activated", &self.activated)
            .field("has_context", &self.ctx.is_some())
            .finish()
    }
}

impl DdsBridgeComponent {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Fuegt ein Topic-Mapping hinzu.
    ///
    /// # Errors
    /// `DuplicatePort` wenn ein Mapping mit demselben Port-Name
    /// bereits registriert ist.
    pub fn add_mapping(&mut self, m: TopicMapping) -> Result<(), BridgeError> {
        if self.mappings.contains_key(&m.port_name) {
            return Err(BridgeError::DuplicatePort(m.port_name));
        }
        self.mappings.insert(m.port_name.clone(), m);
        Ok(())
    }

    /// Liste aller Mappings.
    #[must_use]
    pub fn mappings(&self) -> Vec<&TopicMapping> {
        self.mappings.values().collect()
    }

    /// Anzahl Subscriber-Mappings.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.mappings
            .values()
            .filter(|m| matches!(m.direction, MappingDirection::SinkSubscribesTopic))
            .count()
    }

    /// Anzahl Publisher-Mappings.
    #[must_use]
    pub fn publisher_count(&self) -> usize {
        self.mappings
            .values()
            .filter(|m| matches!(m.direction, MappingDirection::SourcePublishesTopic))
            .count()
    }

    /// Liefert `true` wenn der Container `ccm_activate` aufgerufen
    /// hat.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.activated
    }
}

impl ComponentExecutor for DdsBridgeComponent {
    fn set_context(&mut self, context: Box<dyn ComponentContext>) {
        self.ctx = Some(context);
    }

    fn ccm_activate(&mut self) -> Result<(), CifError> {
        self.activated = true;
        Ok(())
    }

    fn ccm_passivate(&mut self) -> Result<(), CifError> {
        self.activated = false;
        Ok(())
    }

    fn ccm_remove(&mut self) -> Result<(), CifError> {
        self.activated = false;
        self.mappings.clear();
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn sub_mapping(name: &str) -> TopicMapping {
        TopicMapping {
            port_name: name.into(),
            topic_name: alloc::format!("topic_{name}"),
            type_name: "demo::Trade".into(),
            direction: MappingDirection::SinkSubscribesTopic,
        }
    }

    fn pub_mapping(name: &str) -> TopicMapping {
        TopicMapping {
            port_name: name.into(),
            topic_name: alloc::format!("topic_{name}"),
            type_name: "demo::Quote".into(),
            direction: MappingDirection::SourcePublishesTopic,
        }
    }

    struct AnonContext;
    impl ComponentContext for AnonContext {
        fn get_caller_principal(&self) -> Option<Vec<u8>> {
            None
        }
    }

    #[test]
    fn fresh_component_is_inactive() {
        let c = DdsBridgeComponent::new();
        assert!(!c.is_active());
        assert_eq!(c.mappings().len(), 0);
    }

    #[test]
    fn activate_makes_active() {
        let mut c = DdsBridgeComponent::new();
        c.set_context(Box::new(AnonContext));
        c.ccm_activate().unwrap();
        assert!(c.is_active());
    }

    #[test]
    fn passivate_clears_active_flag() {
        let mut c = DdsBridgeComponent::new();
        c.ccm_activate().unwrap();
        c.ccm_passivate().unwrap();
        assert!(!c.is_active());
    }

    #[test]
    fn add_mapping_round_trip() {
        let mut c = DdsBridgeComponent::new();
        c.add_mapping(sub_mapping("a")).unwrap();
        c.add_mapping(pub_mapping("b")).unwrap();
        assert_eq!(c.mappings().len(), 2);
        assert_eq!(c.subscriber_count(), 1);
        assert_eq!(c.publisher_count(), 1);
    }

    #[test]
    fn duplicate_port_rejected() {
        let mut c = DdsBridgeComponent::new();
        c.add_mapping(sub_mapping("p")).unwrap();
        let err = c.add_mapping(pub_mapping("p")).unwrap_err();
        assert!(matches!(err, BridgeError::DuplicatePort(_)));
    }

    #[test]
    fn remove_clears_mappings_and_state() {
        let mut c = DdsBridgeComponent::new();
        c.add_mapping(sub_mapping("a")).unwrap();
        c.ccm_activate().unwrap();
        c.ccm_remove().unwrap();
        assert!(!c.is_active());
        assert_eq!(c.mappings().len(), 0);
    }

    #[test]
    fn mapping_direction_round_trip() {
        let s = sub_mapping("s");
        let p = pub_mapping("p");
        assert_eq!(s.direction, MappingDirection::SinkSubscribesTopic);
        assert_eq!(p.direction, MappingDirection::SourcePublishesTopic);
        assert_ne!(s.direction, p.direction);
    }
}
