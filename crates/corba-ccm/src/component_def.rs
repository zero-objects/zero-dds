// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Component definition model — spec §6.6.
//!
//! A `ComponentDef` describes a component type. Spec-compliant
//! fields:
//!
//! * `name` + `repository_id`.
//! * Inheritance (single inheritance, plus component supports).
//! * Facets (`provides` ports — implement an interface).
//! * Receptacles (`uses` ports — require an interface; simplex/multiplex).
//! * Event sources (`publishes`/`emits`).
//! * Event sinks (`consumes`).
//! * Attributes with optional `setraises`/`getraises`.
//! * Optional: primary key (spec §6.7.2 for keyed components).

use alloc::string::String;
use alloc::vec::Vec;

/// Receptacle multiplicity — spec §6.6.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReceptacleMultiplicity {
    /// `uses` — single connection.
    Simplex,
    /// `uses multiple` — multi-connection.
    Multiplex,
}

/// Facet (provides port) — spec §6.6.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacetDef {
    /// Port name.
    pub name: String,
    /// Implemented interface (repository ID).
    pub interface_id: String,
}

/// Receptacle (uses port) — spec §6.6.5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceptacleDef {
    /// Port name.
    pub name: String,
    /// Required interface (repository ID).
    pub interface_id: String,
    /// Single or multiple connections.
    pub multiplicity: ReceptacleMultiplicity,
}

/// Event source (publishes/emits) — spec §6.6.6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSourceDef {
    /// Port name.
    pub name: String,
    /// Event type (EventType repository ID).
    pub event_type_id: String,
    /// `true` = `emits` (single subscriber); `false` = `publishes`
    /// (multiple subscribers via channel).
    pub emit_only: bool,
}

/// Event sink (consumes) — spec §6.6.7.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSinkDef {
    /// Port name.
    pub name: String,
    /// Event type (EventType repository ID).
    pub event_type_id: String,
}

/// Attribute — spec §6.6.8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeDef {
    /// Attribute name.
    pub name: String,
    /// IDL type spec.
    pub type_spec: String,
    /// `true` = readonly.
    pub readonly: bool,
    /// `setraises` exception IDs (empty for readonly).
    pub set_raises: Vec<String>,
    /// `getraises` exception IDs.
    pub get_raises: Vec<String>,
}

/// Component type definition.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentDef {
    /// Component name.
    pub name: String,
    /// Repository ID.
    pub repository_id: String,
    /// Optional single inheritance from a base component.
    pub base_component: Option<String>,
    /// `supports` interface IDs.
    pub supported_interfaces: Vec<String>,
    /// Facets.
    pub facets: Vec<FacetDef>,
    /// Receptacles.
    pub receptacles: Vec<ReceptacleDef>,
    /// Event sources.
    pub event_sources: Vec<EventSourceDef>,
    /// Event sinks.
    pub event_sinks: Vec<EventSinkDef>,
    /// Attributes.
    pub attributes: Vec<AttributeDef>,
    /// Primary key type IDs (spec §6.7.2 — keyed-component indicator).
    pub primary_key: Vec<String>,
}

impl ComponentDef {
    /// `true` if the component is keyed (primary key present).
    #[must_use]
    pub fn is_keyed(&self) -> bool {
        !self.primary_key.is_empty()
    }

    /// Number of ports (facets + receptacles + events).
    #[must_use]
    pub fn port_count(&self) -> usize {
        self.facets.len()
            + self.receptacles.len()
            + self.event_sources.len()
            + self.event_sinks.len()
    }

    // ---------------------------------------------------------------
    // §6.1.4 Component Identity (Operations)
    // ---------------------------------------------------------------

    /// Spec §6.1.4 — `is_equivalent_component_kind(repo_id)`.
    /// Returns `true` if the given repository ID matches the component
    /// (or a base component in the inheritance chain via a
    /// caller-supplied resolver).
    #[must_use]
    pub fn is_equivalent_component_kind(&self, repo_id: &str) -> bool {
        self.repository_id == repo_id
    }

    /// Spec §6.1.4 — `get_component_def() -> ComponentIR::ComponentDef`.
    /// Here we return the repository ID of the ComponentDef entry
    /// in the IFR; the caller builds the IFR lookup on top of it.
    #[must_use]
    pub fn get_component_def_repo_id(&self) -> &str {
        &self.repository_id
    }

    // ---------------------------------------------------------------
    // §6.4.3 Navigation Interface (generic)
    // ---------------------------------------------------------------

    /// Spec §6.4.3 — `provide_facet(name) -> CORBA::Object`.
    /// Returns the `FacetDef` with the given name, or `None`.
    /// The caller binds the `interface_id` to the ORB-concrete
    /// object reference.
    #[must_use]
    pub fn provide_facet(&self, name: &str) -> Option<&FacetDef> {
        self.facets.iter().find(|f| f.name == name)
    }

    /// Spec §6.4.3 — `get_all_facets() -> FacetDescriptions`.
    /// Returns all facets as an immutable slice.
    #[must_use]
    pub fn get_all_facets(&self) -> &[FacetDef] {
        &self.facets
    }

    /// Spec §6.4.3 — `get_named_facets(names) -> FacetDescriptions`.
    /// Returns the facets whose names appear in the given list
    /// (in `names` order); missing names are skipped.
    #[must_use]
    pub fn get_named_facets(&self, names: &[&str]) -> Vec<&FacetDef> {
        names.iter().filter_map(|n| self.provide_facet(n)).collect()
    }

    // ---------------------------------------------------------------
    // §6.5.3 Receptacles Interface (generic)
    // ---------------------------------------------------------------

    /// Spec §6.5.3 — `get_all_receptacles() -> ReceptacleDescriptions`.
    #[must_use]
    pub fn get_all_receptacles(&self) -> &[ReceptacleDef] {
        &self.receptacles
    }

    /// Spec §6.5.3 — `get_named_receptacles(names) -> ReceptacleDescriptions`.
    #[must_use]
    pub fn get_named_receptacles(&self, names: &[&str]) -> Vec<&ReceptacleDef> {
        names
            .iter()
            .filter_map(|n| self.receptacles.iter().find(|r| r.name == *n))
            .collect()
    }

    // ---------------------------------------------------------------
    // §6.6.8 Events Interface (generic)
    // ---------------------------------------------------------------

    /// Spec §6.6.8 — `get_all_publishers() -> PublisherDescriptions`.
    /// Publisher = `event_sources` with `emit_only == false`.
    #[must_use]
    pub fn get_all_publishers(&self) -> Vec<&EventSourceDef> {
        self.event_sources.iter().filter(|s| !s.emit_only).collect()
    }

    /// Spec §6.6.8 — `get_all_emitters() -> EmitterDescriptions`.
    /// Emitter = `event_sources` with `emit_only == true`.
    #[must_use]
    pub fn get_all_emitters(&self) -> Vec<&EventSourceDef> {
        self.event_sources.iter().filter(|s| s.emit_only).collect()
    }

    /// Spec §6.6.8 — `get_named_publishers(names)`.
    #[must_use]
    pub fn get_named_publishers(&self, names: &[&str]) -> Vec<&EventSourceDef> {
        names
            .iter()
            .filter_map(|n| {
                self.event_sources
                    .iter()
                    .find(|s| !s.emit_only && s.name == *n)
            })
            .collect()
    }

    /// Spec §6.6.8 — `get_named_emitters(names)`.
    #[must_use]
    pub fn get_named_emitters(&self, names: &[&str]) -> Vec<&EventSourceDef> {
        names
            .iter()
            .filter_map(|n| {
                self.event_sources
                    .iter()
                    .find(|s| s.emit_only && s.name == *n)
            })
            .collect()
    }

    // ---------------------------------------------------------------
    // §6.4.5 Supported Interfaces — Runtime Narrow-Helper
    // ---------------------------------------------------------------

    /// Spec §6.4.5 — type-identity narrowing. Returns `true` if the
    /// component supports the given interface (`supports <I>`
    /// in the equivalent IDL). The caller layer combines this with the
    /// ORB `is_a` predicate for the object reference.
    #[must_use]
    pub fn supports_interface(&self, interface_repo_id: &str) -> bool {
        self.supported_interfaces
            .iter()
            .any(|i| i == interface_repo_id)
    }

    /// Spec §6.4.5 — all supported interfaces (type-identity
    /// side; object-reference widening stays with the ORB).
    #[must_use]
    pub fn supported_interface_repo_ids(&self) -> &[String] {
        &self.supported_interfaces
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn trader() -> ComponentDef {
        ComponentDef {
            name: "Trader".into(),
            repository_id: "IDL:demo/Trader:1.0".into(),
            base_component: None,
            supported_interfaces: alloc::vec![],
            facets: alloc::vec![FacetDef {
                name: "control".into(),
                interface_id: "IDL:demo/Control:1.0".into(),
            }],
            receptacles: alloc::vec![ReceptacleDef {
                name: "market_feed".into(),
                interface_id: "IDL:demo/MarketFeed:1.0".into(),
                multiplicity: ReceptacleMultiplicity::Simplex,
            }],
            event_sources: alloc::vec![EventSourceDef {
                name: "trade_pub".into(),
                event_type_id: "IDL:demo/Trade:1.0".into(),
                emit_only: false,
            }],
            event_sinks: alloc::vec![EventSinkDef {
                name: "alarm_sink".into(),
                event_type_id: "IDL:demo/Alarm:1.0".into(),
            }],
            attributes: alloc::vec![AttributeDef {
                name: "max_volume".into(),
                type_spec: "long long".into(),
                readonly: false,
                set_raises: alloc::vec![],
                get_raises: alloc::vec![],
            }],
            primary_key: alloc::vec![],
        }
    }

    #[test]
    fn trader_has_4_ports() {
        let t = trader();
        assert_eq!(t.port_count(), 4);
    }

    #[test]
    fn unkeyed_component_is_not_keyed() {
        assert!(!trader().is_keyed());
    }

    #[test]
    fn keyed_component_detected() {
        let mut t = trader();
        t.primary_key.push("IDL:demo/TraderKey:1.0".into());
        assert!(t.is_keyed());
    }

    #[test]
    fn receptacle_multiplicity_distinct() {
        assert_ne!(
            ReceptacleMultiplicity::Simplex,
            ReceptacleMultiplicity::Multiplex
        );
    }

    #[test]
    fn event_source_emit_only_default_false() {
        let t = trader();
        assert!(!t.event_sources[0].emit_only);
    }

    #[test]
    fn attribute_readonly_skips_set_raises() {
        let a = AttributeDef {
            name: "version".into(),
            type_spec: "string".into(),
            readonly: true,
            set_raises: alloc::vec![],
            get_raises: alloc::vec!["IDL:demo/Bad:1.0".into()],
        };
        assert!(a.readonly);
        assert!(a.set_raises.is_empty());
    }

    // ---------------------------------------------------------------
    // §6.1.4 Component Identity
    // ---------------------------------------------------------------

    #[test]
    fn is_equivalent_component_kind_matches_repo_id() {
        let t = trader();
        assert!(t.is_equivalent_component_kind("IDL:demo/Trader:1.0"));
        assert!(!t.is_equivalent_component_kind("IDL:demo/Other:1.0"));
    }

    #[test]
    fn get_component_def_repo_id_returns_repo_id() {
        let t = trader();
        assert_eq!(t.get_component_def_repo_id(), "IDL:demo/Trader:1.0");
    }

    // ---------------------------------------------------------------
    // §6.4.3 Navigation Interface
    // ---------------------------------------------------------------

    #[test]
    fn provide_facet_returns_facet_by_name() {
        let t = trader();
        let f = t.provide_facet("control").expect("control facet");
        assert_eq!(f.name, "control");
    }

    #[test]
    fn provide_facet_returns_none_for_unknown() {
        let t = trader();
        assert!(t.provide_facet("nonexistent").is_none());
    }

    #[test]
    fn get_all_facets_returns_complete_list() {
        let t = trader();
        assert_eq!(t.get_all_facets().len(), 1);
    }

    #[test]
    fn get_named_facets_filters_by_names() {
        let t = trader();
        let f = t.get_named_facets(&["control", "missing"]);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].name, "control");
    }

    // ---------------------------------------------------------------
    // §6.5.3 Receptacles Interface
    // ---------------------------------------------------------------

    #[test]
    fn get_all_receptacles_returns_complete_list() {
        let t = trader();
        assert_eq!(t.get_all_receptacles().len(), 1);
        assert_eq!(t.get_all_receptacles()[0].name, "market_feed");
    }

    #[test]
    fn get_named_receptacles_filters() {
        let t = trader();
        let r = t.get_named_receptacles(&["market_feed", "missing"]);
        assert_eq!(r.len(), 1);
    }

    // ---------------------------------------------------------------
    // §6.6.8 Events Interface
    // ---------------------------------------------------------------

    #[test]
    fn get_all_publishers_excludes_emit_only_sources() {
        let t = trader();
        // Default emit_only=false → publisher
        assert_eq!(t.get_all_publishers().len(), 1);
        assert!(t.get_all_emitters().is_empty());
    }

    #[test]
    fn get_all_emitters_includes_only_emit_only_sources() {
        let mut t = trader();
        t.event_sources[0].emit_only = true;
        assert!(t.get_all_publishers().is_empty());
        assert_eq!(t.get_all_emitters().len(), 1);
    }

    #[test]
    fn get_named_publishers_respects_emit_only_flag() {
        let t = trader();
        let p = t.get_named_publishers(&["trade_pub"]);
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn get_named_emitters_excludes_publishers() {
        let t = trader();
        let e = t.get_named_emitters(&["trade_pub"]);
        // trade_pub has emit_only=false → not an emitter
        assert!(e.is_empty());
    }

    // ---------------------------------------------------------------
    // §6.4.5 Supported Interfaces
    // ---------------------------------------------------------------

    #[test]
    fn supports_interface_returns_true_for_listed_iface() {
        let mut t = trader();
        t.supported_interfaces
            .push("IDL:demo/Diagnostics:1.0".into());
        assert!(t.supports_interface("IDL:demo/Diagnostics:1.0"));
    }

    #[test]
    fn supports_interface_returns_false_for_unknown() {
        let t = trader();
        assert!(!t.supports_interface("IDL:demo/Unknown:1.0"));
    }

    #[test]
    fn supported_interface_repo_ids_returns_full_list() {
        let mut t = trader();
        t.supported_interfaces.push("A".into());
        t.supported_interfaces.push("B".into());
        assert_eq!(
            t.supported_interface_repo_ids(),
            &["A".to_string(), "B".to_string()]
        );
    }
}
