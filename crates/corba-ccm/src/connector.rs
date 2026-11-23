// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Generic interaction support at run time — CORBA 3.3 Part 3 §7.
//!
//! # The IDL3+ model (§7.2)
//!
//! A port type ([`PortTypeDef`]) groups basic ports and attributes
//! (§7.4.3.1 rule 3); a connector ([`ConnectorDef`]) has basic ports,
//! extended ports (`port`, `mirrorport`) and attributes and may inherit
//! one connector (§7.4.4). [`ConnectorTypeTable::basic_ports`] turns a
//! connector into the basic ports §7.2.2.2 defines — a `provides` of a
//! `port` stays a facet, a `provides` of a `mirrorport` becomes a
//! receptacle and the reverse, the name is `<port>_<basic port>` and an
//! attribute `<port>_<attribute>` — with the ports and attributes of
//! every inherited connector (§7.2.3.3). [`ConnectorTypeTable::port_types_match`]
//! is the matching relation of §7.6.5.
//!
//! The fragments of §7.5 that realise a connector at run time are in
//! [`crate::fragment`].

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::component_def::{
    AttributeDef, ComponentDef, FacetDef, ReceptacleDef, ReceptacleMultiplicity,
};

// ---------------------------------------------------------------------------
// The IDL3+ model: port types and connectors (§7.2.2, §7.2.3, §7.4.3, §7.4.4)
// ---------------------------------------------------------------------------

/// A member of a port type — §7.4.3.1 rule (3): `<provides_dcl>`,
/// `<uses_dcl>` or `<attr_dcl>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortTypeMember {
    /// `provides <interface> <name>`.
    Provides {
        /// The basic port's name.
        name: String,
        /// The interface's repository id.
        interface_id: String,
    },
    /// `uses [multiple] <interface> <name>`.
    Uses {
        /// The basic port's name.
        name: String,
        /// The interface's repository id.
        interface_id: String,
        /// `uses multiple`.
        multiple: bool,
    },
    /// An attribute of the port type.
    Attribute(AttributeDef),
}

/// `porttype <name> { <port_export>+ }` — §7.4.3.1 rule (2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PortTypeDef {
    /// The port type's name.
    pub name: String,
    /// Its repository id.
    pub repository_id: String,
    /// Its basic ports and attributes, in declaration order.
    pub members: Vec<PortTypeMember>,
}

impl PortTypeDef {
    fn provides(&self) -> impl Iterator<Item = (&str, &str)> {
        self.members.iter().filter_map(|m| match m {
            PortTypeMember::Provides { name, interface_id } => {
                Some((name.as_str(), interface_id.as_str()))
            }
            _ => None,
        })
    }

    fn uses(&self) -> impl Iterator<Item = (&str, &str, bool)> {
        self.members.iter().filter_map(|m| match m {
            PortTypeMember::Uses {
                name,
                interface_id,
                multiple,
            } => Some((name.as_str(), interface_id.as_str(), *multiple)),
            _ => None,
        })
    }
}

/// A member of a connector body — §7.4.4 rule (9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorMember {
    /// `provides <interface> <name>`.
    Provides {
        /// The port's name.
        name: String,
        /// The interface's repository id.
        interface_id: String,
    },
    /// `uses [multiple] <interface> <name>`.
    Uses {
        /// The port's name.
        name: String,
        /// The interface's repository id.
        interface_id: String,
        /// `uses multiple`.
        multiple: bool,
    },
    /// `port <port type> <name>` (§7.4.3.2 rule (4)).
    Port {
        /// The extended port's name.
        name: String,
        /// The port type's repository id.
        port_type_id: String,
    },
    /// `mirrorport <port type> <name>`.
    MirrorPort {
        /// The extended port's name.
        name: String,
        /// The port type's repository id.
        port_type_id: String,
    },
    /// An attribute of the connector (§7.2.3.2): reflected in each
    /// fragment.
    Attribute(AttributeDef),
}

/// `connector <name> [: <base>] { <connector_export>* }` — §7.4.4.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnectorDef {
    /// The connector's name.
    pub name: String,
    /// Its repository id.
    pub repository_id: String,
    /// The connector it inherits (§7.2.3.3, rule (8): a single scoped
    /// name).
    pub base: Option<String>,
    /// Its ports and attributes, in declaration order.
    pub members: Vec<ConnectorMember>,
}

/// What [`ConnectorTypeTable`] refuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorModelError {
    /// No connector is registered under this repository id.
    UnknownConnector(String),
    /// A connector names a base that is not registered.
    UnknownBase {
        /// The derived connector.
        connector: String,
        /// The base it names.
        base: String,
    },
    /// An extended port names a port type that is not registered.
    UnknownPortType {
        /// The connector.
        connector: String,
        /// The port type it names.
        port_type: String,
    },
    /// The inheritance links close a cycle.
    Cycle(String),
    /// A port type without members: rule (2) requires `<port_export>+`.
    EmptyPortType(String),
    /// Two ports or attributes of the connector — declared, inherited or
    /// made by the translation of §7.2.2.2 — have the same name.
    RedefinedName {
        /// The connector declaring the second one.
        connector: String,
        /// The name.
        name: String,
    },
}

impl core::fmt::Display for ConnectorModelError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownConnector(id) => write!(f, "no connector `{id}`"),
            Self::UnknownBase { connector, base } => {
                write!(f, "connector `{connector}` inherits the unknown `{base}`")
            }
            Self::UnknownPortType {
                connector,
                port_type,
            } => write!(
                f,
                "connector `{connector}` uses the unknown port type `{port_type}`"
            ),
            Self::Cycle(id) => write!(f, "the inheritance of `{id}` is a cycle"),
            Self::EmptyPortType(id) => write!(f, "port type `{id}` has no member"),
            Self::RedefinedName { connector, name } => {
                write!(f, "connector `{connector}` defines `{name}` twice")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ConnectorModelError {}

/// The port types and connectors one system knows, by repository id.
#[derive(Debug, Clone, Default)]
pub struct ConnectorTypeTable {
    port_types: BTreeMap<String, PortTypeDef>,
    connectors: BTreeMap<String, ConnectorDef>,
}

impl ConnectorTypeTable {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a port type, replacing one of the same repository id.
    pub fn insert_port_type(&mut self, def: PortTypeDef) {
        self.port_types.insert(def.repository_id.clone(), def);
    }

    /// Registers a connector, replacing one of the same repository id.
    pub fn insert_connector(&mut self, def: ConnectorDef) {
        self.connectors.insert(def.repository_id.clone(), def);
    }

    /// The port type registered under `id`.
    #[must_use]
    pub fn port_type(&self, id: &str) -> Option<&PortTypeDef> {
        self.port_types.get(id)
    }

    /// The connector registered under `id`.
    #[must_use]
    pub fn connector(&self, id: &str) -> Option<&ConnectorDef> {
        self.connectors.get(id)
    }

    /// The inheritance chain of a connector, the connector first and the
    /// root last (§7.2.3.3).
    ///
    /// # Errors
    /// [`ConnectorModelError::UnknownConnector`],
    /// [`ConnectorModelError::UnknownBase`], [`ConnectorModelError::Cycle`].
    pub fn base_chain(&self, id: &str) -> Result<Vec<&ConnectorDef>, ConnectorModelError> {
        let mut current = self
            .connectors
            .get(id)
            .ok_or_else(|| ConnectorModelError::UnknownConnector(id.to_string()))?;
        let mut chain = Vec::new();
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(current.repository_id.as_str()) {
                return Err(ConnectorModelError::Cycle(current.repository_id.clone()));
            }
            chain.push(current);
            let Some(base) = current.base.as_deref() else {
                return Ok(chain);
            };
            current =
                self.connectors
                    .get(base)
                    .ok_or_else(|| ConnectorModelError::UnknownBase {
                        connector: current.repository_id.clone(),
                        base: base.to_string(),
                    })?;
        }
    }

    /// The connector as basic ports and attributes — §7.2.2.2 and
    /// §7.2.3.3: the members of every connector of the chain, the root's
    /// first; each extended port replaced by the basic ports of its port
    /// type, turned around for a `mirrorport`, named
    /// `<port>_<basic port>`; each attribute of a port type an attribute
    /// `<port>_<attribute>`. The result has no base: inheritance is
    /// resolved.
    ///
    /// A `provides` of a `mirrorport` becomes a simplex receptacle; a
    /// `uses` (simplex or multiple) of a `mirrorport` becomes a facet.
    ///
    /// # Errors
    /// As [`Self::base_chain`], plus
    /// [`ConnectorModelError::UnknownPortType`],
    /// [`ConnectorModelError::EmptyPortType`] and
    /// [`ConnectorModelError::RedefinedName`].
    pub fn basic_ports(&self, id: &str) -> Result<ComponentDef, ConnectorModelError> {
        let chain = self.base_chain(id)?;
        let derived = chain[0];
        let mut out = ComponentDef {
            name: derived.name.clone(),
            repository_id: derived.repository_id.clone(),
            ..ComponentDef::default()
        };
        let mut names: BTreeSet<String> = BTreeSet::new();
        for def in chain.iter().rev() {
            let mut claim = |name: &str| -> Result<(), ConnectorModelError> {
                if names.insert(name.to_string()) {
                    Ok(())
                } else {
                    Err(ConnectorModelError::RedefinedName {
                        connector: def.repository_id.clone(),
                        name: name.to_string(),
                    })
                }
            };
            for member in &def.members {
                match member {
                    ConnectorMember::Provides { name, interface_id } => {
                        claim(name)?;
                        out.facets.push(FacetDef {
                            name: name.clone(),
                            interface_id: interface_id.clone(),
                        });
                    }
                    ConnectorMember::Uses {
                        name,
                        interface_id,
                        multiple,
                    } => {
                        claim(name)?;
                        out.receptacles.push(ReceptacleDef {
                            name: name.clone(),
                            interface_id: interface_id.clone(),
                            multiplicity: multiplicity(*multiple),
                        });
                    }
                    ConnectorMember::Attribute(attribute) => {
                        claim(&attribute.name)?;
                        out.attributes.push(attribute.clone());
                    }
                    ConnectorMember::Port { name, port_type_id }
                    | ConnectorMember::MirrorPort { name, port_type_id } => {
                        let mirrored = matches!(member, ConnectorMember::MirrorPort { .. });
                        let port_type = self.port_types.get(port_type_id).ok_or_else(|| {
                            ConnectorModelError::UnknownPortType {
                                connector: def.repository_id.clone(),
                                port_type: port_type_id.clone(),
                            }
                        })?;
                        if port_type.members.is_empty() {
                            return Err(ConnectorModelError::EmptyPortType(port_type_id.clone()));
                        }
                        for basic in &port_type.members {
                            match basic {
                                PortTypeMember::Provides {
                                    name: basic_name,
                                    interface_id,
                                } => {
                                    let full = format!("{name}_{basic_name}");
                                    claim(&full)?;
                                    if mirrored {
                                        out.receptacles.push(ReceptacleDef {
                                            name: full,
                                            interface_id: interface_id.clone(),
                                            multiplicity: ReceptacleMultiplicity::Simplex,
                                        });
                                    } else {
                                        out.facets.push(FacetDef {
                                            name: full,
                                            interface_id: interface_id.clone(),
                                        });
                                    }
                                }
                                PortTypeMember::Uses {
                                    name: basic_name,
                                    interface_id,
                                    multiple,
                                } => {
                                    let full = format!("{name}_{basic_name}");
                                    claim(&full)?;
                                    if mirrored {
                                        out.facets.push(FacetDef {
                                            name: full,
                                            interface_id: interface_id.clone(),
                                        });
                                    } else {
                                        out.receptacles.push(ReceptacleDef {
                                            name: full,
                                            interface_id: interface_id.clone(),
                                            multiplicity: multiplicity(*multiple),
                                        });
                                    }
                                }
                                PortTypeMember::Attribute(attribute) => {
                                    let full = format!("{name}_{}", attribute.name);
                                    claim(&full)?;
                                    out.attributes.push(AttributeDef {
                                        name: full,
                                        ..attribute.clone()
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(out)
    }

    /// The matching relation of §7.6.5: "two such types are compatible
    /// when they present one by one compatible `UsesDef` and
    /// `ProvidesDef`". The port types have the same number of `uses` and
    /// of `provides`, and the n-th of one kind of `a` is compatible with
    /// the n-th of that kind of `b`: one interface is the other or
    /// derives from it (`is_a(derived, base)`), and two `uses` agree in
    /// `is_multiple`.
    ///
    /// # Errors
    /// [`ConnectorModelError::UnknownPortType`] (with an empty connector
    /// name) for an id that is not registered.
    pub fn port_types_match(
        &self,
        a: &str,
        b: &str,
        is_a: &dyn Fn(&str, &str) -> bool,
    ) -> Result<bool, ConnectorModelError> {
        let lookup = |id: &str| {
            self.port_types
                .get(id)
                .ok_or_else(|| ConnectorModelError::UnknownPortType {
                    connector: String::new(),
                    port_type: id.to_string(),
                })
        };
        let (a, b) = (lookup(a)?, lookup(b)?);
        let related = |x: &str, y: &str| x == y || is_a(x, y) || is_a(y, x);
        let provides_a: Vec<_> = a.provides().collect();
        let provides_b: Vec<_> = b.provides().collect();
        let uses_a: Vec<_> = a.uses().collect();
        let uses_b: Vec<_> = b.uses().collect();
        Ok(provides_a.len() == provides_b.len()
            && uses_a.len() == uses_b.len()
            && provides_a
                .iter()
                .zip(&provides_b)
                .all(|((_, x), (_, y))| related(x, y))
            && uses_a
                .iter()
                .zip(&uses_b)
                .all(|((_, x, mx), (_, y, my))| mx == my && related(x, y)))
    }
}

fn multiplicity(multiple: bool) -> ReceptacleMultiplicity {
    if multiple {
        ReceptacleMultiplicity::Multiplex
    } else {
        ReceptacleMultiplicity::Simplex
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const DATA: &str = "IDL:Demo/Data_Pusher:1.0";
    pub(crate) const CONTROL: &str = "IDL:Demo/FlowControl:1.0";

    pub(crate) fn attribute(name: &str, readonly: bool) -> AttributeDef {
        AttributeDef {
            name: name.into(),
            type_spec: "string".into(),
            readonly,
            set_raises: Vec::new(),
            get_raises: Vec::new(),
        }
    }

    /// The example of §7.2.2.1 and §7.2.3.1: `porttype
    /// Data_ControlledConsumer { provides Data_Pusher consumer; uses
    /// FlowControl control; }` and `connector Data_Cnx { mirrorport
    /// Data_ControlledConsumer cc; provides Data_Pusher p; }`, the
    /// connector with an attribute `topic`.
    pub(crate) fn table() -> ConnectorTypeTable {
        let mut t = ConnectorTypeTable::new();
        t.insert_port_type(PortTypeDef {
            name: "Data_ControlledConsumer".into(),
            repository_id: "IDL:Demo/Data_ControlledConsumer:1.0".into(),
            members: alloc::vec![
                PortTypeMember::Provides {
                    name: "consumer".into(),
                    interface_id: DATA.into(),
                },
                PortTypeMember::Uses {
                    name: "control".into(),
                    interface_id: CONTROL.into(),
                    multiple: false,
                },
                PortTypeMember::Attribute(attribute("depth", false)),
            ],
        });
        t.insert_connector(ConnectorDef {
            name: "Data_Cnx".into(),
            repository_id: "IDL:Demo/Data_Cnx:1.0".into(),
            base: None,
            members: alloc::vec![
                ConnectorMember::MirrorPort {
                    name: "cc".into(),
                    port_type_id: "IDL:Demo/Data_ControlledConsumer:1.0".into(),
                },
                ConnectorMember::Provides {
                    name: "p".into(),
                    interface_id: DATA.into(),
                },
                ConnectorMember::Attribute(attribute("topic", false)),
                ConnectorMember::Attribute(attribute("kind", true)),
            ],
        });
        t
    }

    #[test]
    fn corba_p3_7_2_2_2_a_mirrorport_turns_provides_into_uses_and_names_the_basic_ports() {
        let ports = table().basic_ports("IDL:Demo/Data_Cnx:1.0").unwrap();
        // mirrorport: provides consumer -> uses cc_consumer; uses control
        // -> provides cc_control.
        assert_eq!(
            ports.receptacles,
            alloc::vec![ReceptacleDef {
                name: "cc_consumer".into(),
                interface_id: DATA.into(),
                multiplicity: ReceptacleMultiplicity::Simplex,
            }]
        );
        let facets: Vec<(&str, &str)> = ports
            .facets
            .iter()
            .map(|f| (f.name.as_str(), f.interface_id.as_str()))
            .collect();
        assert_eq!(facets, [("cc_control", CONTROL), ("p", DATA)]);
        let attributes: Vec<&str> = ports.attributes.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(attributes, ["cc_depth", "topic", "kind"]);
    }

    #[test]
    fn corba_p3_7_2_2_2_a_port_keeps_the_direction_of_its_basic_ports() {
        let mut t = table();
        t.insert_connector(ConnectorDef {
            name: "K".into(),
            repository_id: "IDL:Demo/K:1.0".into(),
            base: None,
            members: alloc::vec![ConnectorMember::Port {
                name: "q".into(),
                port_type_id: "IDL:Demo/Data_ControlledConsumer:1.0".into(),
            }],
        });
        let ports = t.basic_ports("IDL:Demo/K:1.0").unwrap();
        assert_eq!(ports.facets[0].name, "q_consumer");
        assert_eq!(ports.receptacles[0].name, "q_control");
    }

    #[test]
    fn corba_p3_7_2_3_3_a_derived_connector_has_the_ports_and_attributes_of_its_base() {
        let mut t = table();
        t.insert_connector(ConnectorDef {
            name: "Data_Cnx2".into(),
            repository_id: "IDL:Demo/Data_Cnx2:1.0".into(),
            base: Some("IDL:Demo/Data_Cnx:1.0".into()),
            members: alloc::vec![
                ConnectorMember::Uses {
                    name: "monitor".into(),
                    interface_id: CONTROL.into(),
                    multiple: true,
                },
                ConnectorMember::Attribute(attribute("qos", false)),
            ],
        });
        let ports = t.basic_ports("IDL:Demo/Data_Cnx2:1.0").unwrap();
        let facets: Vec<&str> = ports.facets.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(facets, ["cc_control", "p"]);
        let receptacles: Vec<(&str, ReceptacleMultiplicity)> = ports
            .receptacles
            .iter()
            .map(|r| (r.name.as_str(), r.multiplicity))
            .collect();
        assert_eq!(
            receptacles,
            [
                ("cc_consumer", ReceptacleMultiplicity::Simplex),
                ("monitor", ReceptacleMultiplicity::Multiplex)
            ]
        );
        let attributes: Vec<&str> = ports.attributes.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(attributes, ["cc_depth", "topic", "kind", "qos"]);
        assert_eq!(ports.repository_id, "IDL:Demo/Data_Cnx2:1.0");
        assert!(ports.base_component.is_none());
    }

    #[test]
    fn corba_p3_7_2_3_3_a_redefined_name_an_unknown_base_and_a_cycle_are_refused() {
        let mut t = table();
        t.insert_connector(ConnectorDef {
            name: "Bad".into(),
            repository_id: "IDL:Demo/Bad:1.0".into(),
            base: Some("IDL:Demo/Data_Cnx:1.0".into()),
            members: alloc::vec![ConnectorMember::Provides {
                name: "cc_control".into(),
                interface_id: CONTROL.into(),
            }],
        });
        assert_eq!(
            t.basic_ports("IDL:Demo/Bad:1.0"),
            Err(ConnectorModelError::RedefinedName {
                connector: "IDL:Demo/Bad:1.0".into(),
                name: "cc_control".into(),
            })
        );
        t.insert_connector(ConnectorDef {
            name: "Orphan".into(),
            repository_id: "IDL:Demo/Orphan:1.0".into(),
            base: Some("IDL:Demo/None:1.0".into()),
            members: Vec::new(),
        });
        assert!(matches!(
            t.basic_ports("IDL:Demo/Orphan:1.0"),
            Err(ConnectorModelError::UnknownBase { .. })
        ));
        t.insert_connector(ConnectorDef {
            name: "A".into(),
            repository_id: "IDL:Demo/A:1.0".into(),
            base: Some("IDL:Demo/B:1.0".into()),
            members: Vec::new(),
        });
        t.insert_connector(ConnectorDef {
            name: "B".into(),
            repository_id: "IDL:Demo/B:1.0".into(),
            base: Some("IDL:Demo/A:1.0".into()),
            members: Vec::new(),
        });
        assert!(matches!(
            t.basic_ports("IDL:Demo/A:1.0"),
            Err(ConnectorModelError::Cycle(_))
        ));
        t.insert_connector(ConnectorDef {
            name: "C".into(),
            repository_id: "IDL:Demo/C:1.0".into(),
            base: None,
            members: alloc::vec![ConnectorMember::Port {
                name: "x".into(),
                port_type_id: "IDL:Demo/Missing:1.0".into(),
            }],
        });
        assert!(matches!(
            t.basic_ports("IDL:Demo/C:1.0"),
            Err(ConnectorModelError::UnknownPortType { .. })
        ));
    }

    #[test]
    fn corba_p3_7_6_5_port_types_match_one_by_one() {
        let mut t = table();
        // Same shape, the provided interface derived from Data_Pusher.
        t.insert_port_type(PortTypeDef {
            name: "Derived".into(),
            repository_id: "IDL:Demo/Derived:1.0".into(),
            members: alloc::vec![
                PortTypeMember::Provides {
                    name: "sink".into(),
                    interface_id: "IDL:Demo/Fast_Pusher:1.0".into(),
                },
                PortTypeMember::Uses {
                    name: "ctl".into(),
                    interface_id: CONTROL.into(),
                    multiple: false,
                },
            ],
        });
        // A multiple uses does not match a simplex one.
        t.insert_port_type(PortTypeDef {
            name: "Multi".into(),
            repository_id: "IDL:Demo/Multi:1.0".into(),
            members: alloc::vec![
                PortTypeMember::Provides {
                    name: "sink".into(),
                    interface_id: DATA.into(),
                },
                PortTypeMember::Uses {
                    name: "ctl".into(),
                    interface_id: CONTROL.into(),
                    multiple: true,
                },
            ],
        });
        let is_a =
            |derived: &str, base: &str| derived == "IDL:Demo/Fast_Pusher:1.0" && base == DATA;
        let base = "IDL:Demo/Data_ControlledConsumer:1.0";
        assert!(t.port_types_match(base, base, &is_a).unwrap());
        assert!(
            t.port_types_match(base, "IDL:Demo/Derived:1.0", &is_a)
                .unwrap()
        );
        assert!(
            !t.port_types_match(base, "IDL:Demo/Derived:1.0", &|_, _| false)
                .unwrap()
        );
        assert!(
            !t.port_types_match(base, "IDL:Demo/Multi:1.0", &is_a)
                .unwrap()
        );
        assert!(
            t.port_types_match(base, "IDL:Demo/None:1.0", &is_a)
                .is_err()
        );
    }
}
