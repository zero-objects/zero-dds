// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `Components::*` core-types model — Spec §6.4-§6.7 + §6.10.
//!
//! Spec §6.4.3.3 (p. 15) + §6.5.2.4 (p. 21) + §6.5.3 (p. 22) + §6.6.1.2
//! (p. 25) + §6.6.8 (p. 29) + §6.7.6 (p. 40) define the `Components`
//! module. We model the data value objects as plain Rust structs, so
//! that any codegen pipeline can reference them as ScopedNames without
//! having to explicitly read a "Components.idl" file.

use alloc::string::String;
use alloc::vec::Vec;

/// `Components::FeatureName` — Spec §6.4.3.3 (p. 15) — `typedef string
/// FeatureName;`.
pub type FeatureName = String;

/// `CORBA::RepositoryId` — referenced by Spec §6.4.3.3 (p. 15).
pub type RepositoryId = String;

/// `Components::FailureReason` — Spec §6.7.6 (p. 40) — `typedef
/// unsigned long FailureReason;`.
pub type FailureReason = u32;

/// `Components::Cookie` valuetype — Spec §6.5.2.4 (p. 21).
///
/// Spec-IDL:
/// ```idl
/// module Components {
///     valuetype Cookie {
///         private CORBA::OctetSeq cookieValue;
///     };
/// };
/// ```
///
/// Cookies are created by multiplex receptacles and identify a specific
/// connection on the receptacle (Spec §6.5.2.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    /// `private CORBA::OctetSeq cookieValue;` — opaque bytes,
    /// receptacle-implementation-defined.
    pub cookie_value: Vec<u8>,
}

impl Cookie {
    /// Constructor — Spec §6.5.2.4 (p. 21).
    #[must_use]
    pub fn new(cookie_value: Vec<u8>) -> Self {
        Self { cookie_value }
    }

    /// Spec §6.5.2.4 (S. 22): "any derived cookie types shall be
    /// truncatable to Cookie, and the information preserved in the
    /// cookieValue octet sequence shall be sufficient for the receptacle
    /// implementation to identify the cookie and its associated
    /// connected reference."
    #[must_use]
    pub fn truncate_to_base(&self) -> Self {
        // Truncation preserves the octet sequence — that is the normative
        // guarantee. Derived-subtype state is lost.
        Self {
            cookie_value: self.cookie_value.clone(),
        }
    }
}

/// `Components::PortDescription` valuetype — Spec §6.4.3.3 (p. 15).
///
/// Base valuetype for FacetDescription, ReceptacleDescription,
/// ConsumerDescription, EmitterDescription, PublisherDescription
/// (each in its respective spec section §6.4.3.3 / §6.5.3 / §6.6.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortDescription {
    /// Port name (spec FeatureName).
    pub name: FeatureName,
    /// CORBA repository ID of the port interface type.
    pub type_id: RepositoryId,
}

/// `Components::FacetDescription : PortDescription` — Spec §6.4.3.3
/// (p. 15).
///
/// `valuetype FacetDescription : PortDescription { public Object
/// facet_ref; };` — in our ORB-free context, `facet_ref` is an abstract
/// object reference (modeled as a repository-ID string).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacetDescription {
    /// Inherited port description.
    pub base: PortDescription,
    /// `Object facet_ref;` — opaque object-reference identifier.
    pub facet_ref: RepositoryId,
}

/// `Components::ConnectionDescription` valuetype — Spec §6.5.3 (p. 22).
///
/// `valuetype ConnectionDescription { public Cookie ck; public Object
/// objref; };`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionDescription {
    /// Cookie of the connection (for multiplex; `default` for simplex).
    pub cookie: Cookie,
    /// Connected object reference (repository-ID identifier).
    pub objref: RepositoryId,
}

/// `Components::ReceptacleDescription : PortDescription` — Spec §6.5.3
/// (p. 22).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceptacleDescription {
    /// Inherited port description.
    pub base: PortDescription,
    /// `boolean is_multiple;`.
    pub is_multiple: bool,
    /// `ConnectionDescriptions connections;`.
    pub connections: Vec<ConnectionDescription>,
}

/// `Components::ConsumerDescription : PortDescription` — Spec §6.6.8
/// (p. 30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerDescription {
    /// Inherited port description.
    pub base: PortDescription,
    /// `EventConsumerBase consumer;` — opaque object reference.
    pub consumer: RepositoryId,
}

/// `Components::EmitterDescription : PortDescription` — Spec §6.6.8
/// (p. 30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmitterDescription {
    /// Inherited port description.
    pub base: PortDescription,
    /// `EventConsumerBase consumer;` — counterpart endpoint.
    pub consumer: RepositoryId,
}

/// `Components::SubscriberDescription` valuetype — Spec §6.6.8 (p. 30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriberDescription {
    /// Cookie under which the subscription is registered.
    pub cookie: Cookie,
    /// `EventConsumerBase consumer;`.
    pub consumer: RepositoryId,
}

/// `Components::PublisherDescription : PortDescription` — Spec §6.6.8
/// (p. 30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublisherDescription {
    /// Inherited port description.
    pub base: PortDescription,
    /// `SubscriberDescriptions consumers;`.
    pub consumers: Vec<SubscriberDescription>,
}

/// `Components::ConfigValue` valuetype — Spec §6.10.1.2 (p. 45).
///
/// `valuetype ConfigValue { public FeatureName name; public any value;
/// };`. We model `any` as opaque bytes (CDR-marshaled).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValue {
    /// Attribute name.
    pub name: FeatureName,
    /// CDR-marshaled `any` value.
    pub value: Vec<u8>,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn cookie_new_stores_octet_seq() {
        let c = Cookie::new(alloc::vec![1, 2, 3]);
        assert_eq!(c.cookie_value, alloc::vec![1, 2, 3]);
    }

    #[test]
    fn cookie_truncate_preserves_octet_seq() {
        // Spec §6.5.2.4 (p. 22): "information preserved in the
        // cookieValue octet sequence shall be sufficient".
        let c = Cookie::new(alloc::vec![0xDE, 0xAD]);
        let t = c.truncate_to_base();
        assert_eq!(t.cookie_value, c.cookie_value);
    }

    #[test]
    fn port_description_carries_name_and_type_id() {
        let p = PortDescription {
            name: String::from("foo"),
            type_id: String::from("IDL:M/I:1.0"),
        };
        assert_eq!(p.name, "foo");
        assert_eq!(p.type_id, "IDL:M/I:1.0");
    }

    #[test]
    fn receptacle_description_supports_simplex_and_multiplex() {
        // Spec §6.5.3 (p. 22) — `is_multiple` differentiates.
        let simplex = ReceptacleDescription {
            base: PortDescription {
                name: String::from("manager"),
                type_id: String::from("IDL:Stock/StockManager:1.0"),
            },
            is_multiple: false,
            connections: alloc::vec![],
        };
        assert!(!simplex.is_multiple);
        assert!(simplex.connections.is_empty());

        let multiplex = ReceptacleDescription {
            base: PortDescription {
                name: String::from("managers"),
                type_id: String::from("IDL:Stock/StockManager:1.0"),
            },
            is_multiple: true,
            connections: alloc::vec![
                ConnectionDescription {
                    cookie: Cookie::new(alloc::vec![1]),
                    objref: String::from("ref-A"),
                },
                ConnectionDescription {
                    cookie: Cookie::new(alloc::vec![2]),
                    objref: String::from("ref-B"),
                }
            ],
        };
        assert!(multiplex.is_multiple);
        assert_eq!(multiplex.connections.len(), 2);
    }

    #[test]
    fn publisher_description_can_have_multiple_subscribers() {
        // Spec §6.6.5 (p. 27) "multiple subscribers".
        let p = PublisherDescription {
            base: PortDescription {
                name: String::from("ticker"),
                type_id: String::from("IDL:Stock/Tick:1.0"),
            },
            consumers: alloc::vec![
                SubscriberDescription {
                    cookie: Cookie::new(alloc::vec![10]),
                    consumer: String::from("sub-A"),
                },
                SubscriberDescription {
                    cookie: Cookie::new(alloc::vec![11]),
                    consumer: String::from("sub-B"),
                }
            ],
        };
        assert_eq!(p.consumers.len(), 2);
    }

    #[test]
    fn config_value_carries_name_and_marshaled_value() {
        // Spec §6.10.1.2 (p. 45).
        let cv = ConfigValue {
            name: String::from("rate_hz"),
            value: alloc::vec![0, 0, 0, 100],
        };
        assert_eq!(cv.name, "rate_hz");
        assert_eq!(cv.value.len(), 4);
    }
}
