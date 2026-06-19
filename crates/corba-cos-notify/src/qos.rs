// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CosNotification §2.5 — QoS + admin properties as a `Property` bag.
//!
//! The spec defines QoS/admin as `sequence<Property>` (name + `any` value);
//! `set_qos`/`get_qos`/`set_admin`/`get_admin` operate on it. Well-known
//! property names below (§2.5.2/§2.5.3).

use alloc::string::String;

use zerodds_cdr::CorbaAny;

use crate::event::{Property, PropertySeq};

/// `EventReliability` (§2.5.2): BestEffort(0)/Persistent(1).
pub const EVENT_RELIABILITY: &str = "EventReliability";
/// `ConnectionReliability` (§2.5.2).
pub const CONNECTION_RELIABILITY: &str = "ConnectionReliability";
/// `Priority` (§2.5.2): -32767..32767.
pub const PRIORITY: &str = "Priority";
/// `OrderPolicy` (§2.5.2): AnyOrder/FifoOrder/PriorityOrder/DeadlineOrder.
pub const ORDER_POLICY: &str = "OrderPolicy";
/// `DiscardPolicy` (§2.5.2).
pub const DISCARD_POLICY: &str = "DiscardPolicy";
/// `MaxEventsPerConsumer` (§2.5.2).
pub const MAX_EVENTS_PER_CONSUMER: &str = "MaxEventsPerConsumer";
/// `MaxQueueLength` (Admin, §2.5.3).
pub const MAX_QUEUE_LENGTH: &str = "MaxQueueLength";
/// `MaxConsumers` (Admin, §2.5.3).
pub const MAX_CONSUMERS: &str = "MaxConsumers";
/// `MaxSuppliers` (Admin, §2.5.3).
pub const MAX_SUPPLIERS: &str = "MaxSuppliers";

/// Property bag for QoS or admin properties.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QoSProperties(pub PropertySeq);

impl QoSProperties {
    /// Empty bag.
    #[must_use]
    pub fn new() -> Self {
        Self(PropertySeq::new())
    }

    /// Sets (or overwrites) a property.
    pub fn set(&mut self, name: impl Into<String>, value: CorbaAny) {
        let name = name.into();
        if let Some(p) = self.0.iter_mut().find(|p| p.name == name) {
            p.value = value;
        } else {
            self.0.push(Property::new(name, value));
        }
    }

    /// Reads a property by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&CorbaAny> {
        self.0.iter().find(|p| p.name == name).map(|p| &p.value)
    }

    /// `set_qos`/`set_admin`: adopt all properties from `props`.
    pub fn apply(&mut self, props: &PropertySeq) {
        for p in props {
            self.set(p.name.clone(), p.value.clone());
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::AnyValue;

    #[test]
    fn qos_set_get_overwrite() {
        let mut q = QoSProperties::new();
        q.set(PRIORITY, CorbaAny(AnyValue::Short(5)));
        assert_eq!(q.get(PRIORITY), Some(&CorbaAny(AnyValue::Short(5))));
        q.set(PRIORITY, CorbaAny(AnyValue::Short(9)));
        assert_eq!(q.get(PRIORITY), Some(&CorbaAny(AnyValue::Short(9))));
        assert_eq!(q.0.len(), 1, "overwrite, no duplicate");
        assert_eq!(q.get(EVENT_RELIABILITY), None);
    }
}
