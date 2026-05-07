// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `Components::*` Core-Types Model — Spec §6.4-§6.7 + §6.10.
//!
//! Spec §6.4.3.3 (S. 15) + §6.5.2.4 (S. 21) + §6.5.3 (S. 22) + §6.6.1.2
//! (S. 25) + §6.6.8 (S. 29) + §6.7.6 (S. 40) definieren das `Components`-
//! Modul. Wir modellieren die Daten-Wertobjekte als plain Rust-Structs,
//! sodass jede Codegen-Pipeline diese als ScopedNames referenzieren kann
//! ohne ein "Components.idl"-File explizit einlesen zu muessen.

use alloc::string::String;
use alloc::vec::Vec;

/// `Components::FeatureName` — Spec §6.4.3.3 (S. 15) — `typedef string
/// FeatureName;`.
pub type FeatureName = String;

/// `CORBA::RepositoryId` — Spec §6.4.3.3 (S. 15) referenziert.
pub type RepositoryId = String;

/// `Components::FailureReason` — Spec §6.7.6 (S. 40) — `typedef
/// unsigned long FailureReason;`.
pub type FailureReason = u32;

/// `Components::Cookie`-valuetype — Spec §6.5.2.4 (S. 21).
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
/// Cookies werden von Multiplex-Receptacles erzeugt und identifizieren
/// eine konkrete Connection auf dem Receptacle (Spec §6.5.2.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    /// `private CORBA::OctetSeq cookieValue;` — opaque Bytes,
    /// Receptacle-implementation-defined.
    pub cookie_value: Vec<u8>,
}

impl Cookie {
    /// Konstruktor — Spec §6.5.2.4 (S. 21).
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
        // Bei der Truncation bleibt der Octet-Sequence erhalten — das ist
        // die normative Garantie. Derived-Subtype-State geht verloren.
        Self {
            cookie_value: self.cookie_value.clone(),
        }
    }
}

/// `Components::PortDescription`-valuetype — Spec §6.4.3.3 (S. 15).
///
/// Base-valuetype fuer FacetDescription, ReceptacleDescription,
/// ConsumerDescription, EmitterDescription, PublisherDescription
/// (jeweils im jeweiligen Spec-Abschnitt §6.4.3.3 / §6.5.3 / §6.6.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortDescription {
    /// Port-Name (Spec FeatureName).
    pub name: FeatureName,
    /// CORBA-Repository-ID des Port-Interface-Typs.
    pub type_id: RepositoryId,
}

/// `Components::FacetDescription : PortDescription` — Spec §6.4.3.3
/// (S. 15).
///
/// `valuetype FacetDescription : PortDescription { public Object
/// facet_ref; };` — `facet_ref` ist in unserem ORB-freien Kontext eine
/// abstrakte Object-Reference (modelliert als Repository-ID-String).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacetDescription {
    /// Inherited port description.
    pub base: PortDescription,
    /// `Object facet_ref;` — opaque Object-Reference-Identifier.
    pub facet_ref: RepositoryId,
}

/// `Components::ConnectionDescription`-valuetype — Spec §6.5.3 (S. 22).
///
/// `valuetype ConnectionDescription { public Cookie ck; public Object
/// objref; };`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionDescription {
    /// Cookie der Connection (bei Multiplex; bei Simplex `default`).
    pub cookie: Cookie,
    /// Verbundener Object-Reference (Repository-ID-Identifier).
    pub objref: RepositoryId,
}

/// `Components::ReceptacleDescription : PortDescription` — Spec §6.5.3
/// (S. 22).
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
/// (S. 30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerDescription {
    /// Inherited port description.
    pub base: PortDescription,
    /// `EventConsumerBase consumer;` — opaque Object-Reference.
    pub consumer: RepositoryId,
}

/// `Components::EmitterDescription : PortDescription` — Spec §6.6.8
/// (S. 30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmitterDescription {
    /// Inherited port description.
    pub base: PortDescription,
    /// `EventConsumerBase consumer;` — gegenstueck-Endpunkt.
    pub consumer: RepositoryId,
}

/// `Components::SubscriberDescription`-valuetype — Spec §6.6.8 (S. 30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriberDescription {
    /// Cookie unter dem die Subscription registriert ist.
    pub cookie: Cookie,
    /// `EventConsumerBase consumer;`.
    pub consumer: RepositoryId,
}

/// `Components::PublisherDescription : PortDescription` — Spec §6.6.8
/// (S. 30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublisherDescription {
    /// Inherited port description.
    pub base: PortDescription,
    /// `SubscriberDescriptions consumers;`.
    pub consumers: Vec<SubscriberDescription>,
}

/// `Components::ConfigValue`-valuetype — Spec §6.10.1.2 (S. 45).
///
/// `valuetype ConfigValue { public FeatureName name; public any value;
/// };`. Wir modellieren `any` als opaque Bytes (CDR-marshaled).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValue {
    /// Attribut-Name.
    pub name: FeatureName,
    /// CDR-marshaled `any`-Wert.
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
        // Spec §6.5.2.4 (S. 22): "information preserved in the
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
        // Spec §6.5.3 (S. 22) — `is_multiple` differenziert.
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
        // Spec §6.6.5 (S. 27) "multiple subscribers".
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
        // Spec §6.10.1.2 (S. 45).
        let cv = ConfigValue {
            name: String::from("rate_hz"),
            value: alloc::vec![0, 0, 0, 100],
        };
        assert_eq!(cv.name, "rate_hz");
        assert_eq!(cv.value.len(), 4);
    }
}
