// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DdsBridgeComponent — bidirectional CCM↔DDS bridge.
//!
//! Spec refs:
//! * CCM 4.0 §6.4 (EventSource/EventSink Ports)
//! * DDS 1.4 §2.2.2 (DCPS Topic/Publisher/Subscriber)
//!
//! This component maps:
//!
//! * CCM EventSink → DDS DataReader (publishing into a DDS topic)
//! * CCM EventSource → DDS DataWriter (forwarding from a DDS topic)
//!
//! The concrete DCPS calls are made by the selected DDS runtime
//! (`zerodds-dcps`); this component only provides the mapping model and
//! the lifecycle hooks.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_corba_ccm::cif::{CifError, ComponentExecutor};
use zerodds_corba_ccm::context::ComponentContext;

/// Mapping direction: component-port-to-DDS or DDS-to-component-port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingDirection {
    /// CCM EventSink → DDS DataReader (subscriber).
    SinkSubscribesTopic,
    /// CCM EventSource → DDS DataWriter (publisher).
    SourcePublishesTopic,
}

/// A concrete bridge mapping definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicMapping {
    /// CCM port name (source or sink).
    pub port_name: String,
    /// DDS topic name.
    pub topic_name: String,
    /// Type name (fully qualified).
    pub type_name: String,
    /// Mapping direction.
    pub direction: MappingDirection,
}

/// Bridge component error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    /// A mapping with the same port name already exists.
    DuplicatePort(String),
    /// Component not activated.
    NotActive,
    /// Generic CCM error.
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

/// DDS bridge component — production-ready CCM component.
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
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a topic mapping.
    ///
    /// # Errors
    /// `DuplicatePort` if a mapping with the same port name is
    /// already registered.
    pub fn add_mapping(&mut self, m: TopicMapping) -> Result<(), BridgeError> {
        if self.mappings.contains_key(&m.port_name) {
            return Err(BridgeError::DuplicatePort(m.port_name));
        }
        self.mappings.insert(m.port_name.clone(), m);
        Ok(())
    }

    /// List of all mappings.
    #[must_use]
    pub fn mappings(&self) -> Vec<&TopicMapping> {
        self.mappings.values().collect()
    }

    /// Number of subscriber mappings.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.mappings
            .values()
            .filter(|m| matches!(m.direction, MappingDirection::SinkSubscribesTopic))
            .count()
    }

    /// Number of publisher mappings.
    #[must_use]
    pub fn publisher_count(&self) -> usize {
        self.mappings
            .values()
            .filter(|m| matches!(m.direction, MappingDirection::SourcePublishesTopic))
            .count()
    }

    /// Returns `true` if the container has called `ccm_activate`.
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
