// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The CIDL model — CORBA 3.3 Part 3 §8.
//!
//! One model for the compiler and the runtime: `zerodds_ast::cidl`, which
//! `zerodds-idl-lower` produces from a `.cdl` file (`lower_cidl`) and a
//! container host builds from a deployment plan. This module re-exports
//! it and adds what the runtime knows of a home and its component:
//! [`home_facts`] turns a [`HomeDef`] and a [`ComponentDef`] into the
//! [`HomeFacts`] that [`Composition::validate`] checks a composition
//! against (the life cycle category constraints of Tables 8.3 to 8.5,
//! the segments of §8.11 and §8.13, the delegations of §8.14 to §8.18).

pub use zerodds_ast::cidl::*;

use crate::component_def::ComponentDef;
use crate::home::HomeDef;
use alloc::string::String;
use alloc::vec::Vec;

/// The facts [`Composition::validate`] reads, from the runtime's
/// definitions of a home and of the component it manages.
///
/// [`HomeDef`] carries the primary key and no explicit operations; the
/// result holds the orthodox operations of Part 3 §6.8.1 (`create`, and
/// for a keyed home `find_by_primary_key`, `remove` and
/// `get_primary_key`). A caller that knows the home's explicit factories,
/// finders and operations adds them to the fields of the result.
#[must_use]
pub fn home_facts(home: &HomeDef, component: &ComponentDef) -> HomeFacts {
    let keyed = home.primary_key_id.is_some();
    let mut operations: Vec<String> = alloc::vec![String::from("create")];
    if keyed {
        for name in ["find_by_primary_key", "remove", "get_primary_key"] {
            operations.push(String::from(name));
        }
    }
    let stateful_features = component
        .attributes
        .iter()
        .map(|a| a.name.clone())
        .chain(component.receptacles.iter().map(|r| r.name.clone()))
        .chain(component.event_sources.iter().map(|s| s.name.clone()))
        .collect();
    HomeFacts {
        primary_key: keyed,
        explicit_finders: Vec::new(),
        factories: alloc::vec![String::from("create")],
        operations,
        facets: component.facets.iter().map(|f| f.name.clone()).collect(),
        stateful_features,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::component_def::{AttributeDef, FacetDef};

    fn toon() -> ComponentDef {
        ComponentDef {
            name: "Toon".into(),
            repository_id: "IDL:LooneyToons/Toon:1.0".into(),
            facets: alloc::vec![FacetDef {
                name: "bird".into(),
                interface_id: "IDL:LooneyToons/Bird:1.0".into(),
            }],
            attributes: alloc::vec![AttributeDef {
                name: "episode".into(),
                type_spec: "string".into(),
                readonly: false,
                set_raises: Vec::new(),
                get_raises: Vec::new(),
            }],
            ..ComponentDef::default()
        }
    }

    #[test]
    fn corba_p3_8_5_1_an_entity_composition_needs_a_keyed_home_at_run_time() {
        let keyless = HomeDef {
            name: "ToonTown".into(),
            repository_id: "IDL:LooneyToons/ToonTown:1.0".into(),
            managed_component_id: "IDL:LooneyToons/Toon:1.0".into(),
            primary_key_id: None,
        };
        let keyed = HomeDef {
            primary_key_id: Some("IDL:LooneyToons/EpisodeName:1.0".into()),
            ..keyless.clone()
        };
        let entity = Composition {
            name: "MerryMelodies::ToonImpl".into(),
            category: CompositionCategory::Entity,
            home_executor: HomeExecutorDef {
                name: "ToonTownImpl".into(),
                implements: "LooneyToons::ToonTown".into(),
                executor: ExecutorDef {
                    name: "ToonEntityImpl".into(),
                    ..ExecutorDef::default()
                },
                ..HomeExecutorDef::default()
            },
            proxy_home: None,
        };
        let errors = entity.validate(&home_facts(&keyless, &toon()), None);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].clause, "CORBA 3.3 Part 3 §8.5.1, Table 8.5");
        assert!(
            entity
                .validate(&home_facts(&keyed, &toon()), None)
                .is_empty()
        );
        // The facet and the attribute of the component are what a
        // segment and a feature delegation may name.
        let facts = home_facts(&keyed, &toon());
        assert_eq!(facts.facets, alloc::vec![String::from("bird")]);
        assert_eq!(
            facts.stateful_features,
            alloc::vec![String::from("episode")]
        );
        assert!(facts.operations.iter().any(|o| o == "find_by_primary_key"));
    }
}
