// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CosNotification §2.2 — `StructuredEvent` + `EventType` + `Property`.
//!
//! ```text
//! struct EventType { string domain_name; string type_name; };
//! struct Property  { string name; any value; };
//! typedef sequence<Property> PropertySeq;
//! struct FixedEventHeader { EventType event_type; string event_name; };
//! struct EventHeader { FixedEventHeader fixed_header; PropertySeq variable_header; };
//! struct StructuredEvent {
//!     EventHeader  header;
//!     PropertySeq  filterable_data;
//!     any          remainder_of_body;
//! };
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::CorbaAny;

/// `EventType` (§2.2): domain + type name. `"*"` is a wildcard for filter/subscription.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EventType {
    /// `domain_name`.
    pub domain_name: String,
    /// `type_name`.
    pub type_name: String,
}

impl EventType {
    /// Constructor.
    #[must_use]
    pub fn new(domain: impl Into<String>, type_name: impl Into<String>) -> Self {
        Self {
            domain_name: domain.into(),
            type_name: type_name.into(),
        }
    }

    /// `true` if `pattern` (with `"*"` wildcard per field) matches `self`.
    #[must_use]
    pub fn matches(&self, pattern: &EventType) -> bool {
        let dom = pattern.domain_name == "*" || pattern.domain_name == self.domain_name;
        let typ = pattern.type_name == "*" || pattern.type_name == self.type_name;
        dom && typ
    }
}

/// `Property` (§2.2): named `any` value. Carries QoS, filterable data, header fields.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Property {
    /// Property name.
    pub name: String,
    /// `any` value.
    pub value: CorbaAny,
}

impl Property {
    /// Constructor.
    #[must_use]
    pub fn new(name: impl Into<String>, value: CorbaAny) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }
}

/// `sequence<Property>`.
pub type PropertySeq = Vec<Property>;

/// `FixedEventHeader` (§2.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FixedEventHeader {
    /// `event_type`.
    pub event_type: EventType,
    /// `event_name`.
    pub event_name: String,
}

/// `EventHeader` (§2.2): fixed header + variable PropertySeq.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EventHeader {
    /// `fixed_header`.
    pub fixed_header: FixedEventHeader,
    /// `variable_header` (per-event QoS overrides).
    pub variable_header: PropertySeq,
}

/// `StructuredEvent` (§2.2) — the central notification datum.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StructuredEvent {
    /// `header`.
    pub header: EventHeader,
    /// `filterable_data` — properties that filters match against.
    pub filterable_data: PropertySeq,
    /// `remainder_of_body` — the payload as `any`.
    pub remainder_of_body: CorbaAny,
}

impl StructuredEvent {
    /// Builds an event with type (`domain::type`), name, and payload; empty header/
    /// filterable sequences.
    #[must_use]
    pub fn new(
        domain: impl Into<String>,
        type_name: impl Into<String>,
        event_name: impl Into<String>,
        body: CorbaAny,
    ) -> Self {
        Self {
            header: EventHeader {
                fixed_header: FixedEventHeader {
                    event_type: EventType::new(domain, type_name),
                    event_name: event_name.into(),
                },
                variable_header: PropertySeq::new(),
            },
            filterable_data: PropertySeq::new(),
            remainder_of_body: body,
        }
    }

    /// This event's `EventType` (for filter/subscription).
    #[must_use]
    pub fn event_type(&self) -> &EventType {
        &self.header.fixed_header.event_type
    }

    /// Adds a filterable property (builder style).
    #[must_use]
    pub fn with_filterable(mut self, name: impl Into<String>, value: CorbaAny) -> Self {
        self.filterable_data.push(Property::new(name, value));
        self
    }

    /// Looks up a filterable property by name.
    #[must_use]
    pub fn filterable(&self, name: &str) -> Option<&CorbaAny> {
        self.filterable_data
            .iter()
            .find(|p| p.name == name)
            .map(|p| &p.value)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_cdr::AnyValue;

    #[test]
    fn event_type_wildcard_match() {
        let e = EventType::new("Telecom", "CallEvent");
        assert!(e.matches(&EventType::new("Telecom", "CallEvent")));
        assert!(e.matches(&EventType::new("*", "CallEvent")));
        assert!(e.matches(&EventType::new("Telecom", "*")));
        assert!(e.matches(&EventType::new("*", "*")));
        assert!(!e.matches(&EventType::new("Finance", "CallEvent")));
    }

    #[test]
    fn structured_event_build_and_query() {
        let ev = StructuredEvent::new(
            "Telecom",
            "CallEvent",
            "call-42",
            CorbaAny(AnyValue::Str("payload".into())),
        )
        .with_filterable("priority", CorbaAny(AnyValue::Long(7)));
        assert_eq!(ev.event_type(), &EventType::new("Telecom", "CallEvent"));
        assert_eq!(ev.header.fixed_header.event_name, "call-42");
        assert_eq!(
            ev.filterable("priority"),
            Some(&CorbaAny(AnyValue::Long(7)))
        );
        assert_eq!(ev.filterable("missing"), None);
    }
}
