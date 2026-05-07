// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `base_name`-Resolver mit Cycle-Detection.
//!
//! DDS-XML 1.0 erlaubt mehreren Building-Blocks (QoS-Profile §7.3.2.4.2,
//! Domain §7.3.4.4.2, DomainParticipant §7.3.5.4.3) eine `base_name`-Attribut-
//! basierte Vererbung. Die Spec verlangt, dass die Basis-Definition vor der
//! erbenden Definition steht — naive Implementierungen koennen ueber
//! Bibliotheks-Grenzen hinweg dennoch Zyklen erzeugen.
//!
//! Dieses Modul implementiert eine generische Inheritance-Aufloesung mit
//! DAG-Pruefung. Die Aufloesungs-Routine ist parametrisiert ueber den
//! Item-Typ (z.B. QoS-Profile, Domain, Participant) und die `base_name`-
//! Lookup-Funktion.

use crate::errors::XmlError;
use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Maximale Inheritance-Tiefe (DoS-Cap).
pub const MAX_INHERITANCE_DEPTH: usize = 32;

/// Resolves a `base_name`-chain starting at `name` and returns the
/// chain in **base-first** order, i.e. `[grandparent, parent, name]`.
///
/// Die Reihenfolge ermoeglicht es Aufrufern, Felder schrittweise zu
/// "merge"en (Base-Defaults zuerst, dann ueberschreiben).
///
/// # Parameter
/// * `name` — Startpunkt der Aufloesung.
/// * `lookup` — Closure, die fuer einen `name` den `base_name` (oder
///   `None`, wenn keine Basis vorhanden) zurueckgibt. Wenn der `name`
///   selbst nicht existiert, soll [`XmlError::MissingRequiredElement`]
///   zurueckgegeben werden.
///
/// # Errors
/// * [`XmlError::CircularInheritance`] — wenn ein Zyklus erkannt wird.
/// * [`XmlError::LimitExceeded`] — wenn [`MAX_INHERITANCE_DEPTH`]
///   ueberschritten wird.
/// * Fehler aus der `lookup`-Closure werden durchgereicht.
///
/// zerodds-lint: recursion-depth = no recursion (iterative loop with
/// MAX_INHERITANCE_DEPTH bound).
pub fn resolve_chain<F>(name: &str, mut lookup: F) -> Result<Vec<String>, XmlError>
where
    F: FnMut(&str) -> Result<Option<String>, XmlError>,
{
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut chain: Vec<String> = Vec::new();
    let mut current = name.to_string();

    for _ in 0..MAX_INHERITANCE_DEPTH {
        if !visited.insert(current.clone()) {
            // Zyklus: `current` wurde bereits besucht.
            chain.push(current.clone());
            let pretty = chain.join(" -> ");
            return Err(XmlError::CircularInheritance(pretty));
        }
        chain.push(current.clone());

        match lookup(&current)? {
            None => {
                // Kein Basis-Eintrag: Aufloesung beendet.
                chain.reverse();
                return Ok(chain);
            }
            Some(base) => {
                current = base;
            }
        }
    }

    Err(XmlError::LimitExceeded(format!(
        "base_name chain depth > {MAX_INHERITANCE_DEPTH}"
    )))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;
    use alloc::vec;

    fn make_lookup(
        items: BTreeMap<&'static str, Option<&'static str>>,
    ) -> impl FnMut(&str) -> Result<Option<String>, XmlError> {
        move |name: &str| {
            items
                .get(name)
                .copied()
                .ok_or_else(|| XmlError::MissingRequiredElement(name.to_string()))
                .map(|opt| opt.map(|s| s.to_string()))
        }
    }

    #[test]
    fn no_inheritance() {
        let mut items: BTreeMap<&str, Option<&str>> = BTreeMap::new();
        items.insert("A", None);
        let chain = resolve_chain("A", make_lookup(items)).expect("ok");
        assert_eq!(chain, vec!["A".to_string()]);
    }

    #[test]
    fn three_level_chain() {
        // C is base of B, B is base of A.
        let mut items: BTreeMap<&str, Option<&str>> = BTreeMap::new();
        items.insert("A", Some("B"));
        items.insert("B", Some("C"));
        items.insert("C", None);
        let chain = resolve_chain("A", make_lookup(items)).expect("ok");
        // base-first order
        assert_eq!(
            chain,
            vec!["C".to_string(), "B".to_string(), "A".to_string()]
        );
    }

    #[test]
    fn two_node_cycle() {
        // A -> B -> A
        let mut items: BTreeMap<&str, Option<&str>> = BTreeMap::new();
        items.insert("A", Some("B"));
        items.insert("B", Some("A"));
        let err = resolve_chain("A", make_lookup(items)).expect_err("cycle");
        match err {
            XmlError::CircularInheritance(msg) => {
                assert!(msg.contains("A -> B -> A") || msg.contains("A"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn self_cycle() {
        let mut items: BTreeMap<&str, Option<&str>> = BTreeMap::new();
        items.insert("A", Some("A"));
        let err = resolve_chain("A", make_lookup(items)).expect_err("self-cycle");
        assert!(matches!(err, XmlError::CircularInheritance(_)));
    }

    #[test]
    fn missing_base_propagates() {
        let mut items: BTreeMap<&str, Option<&str>> = BTreeMap::new();
        items.insert("A", Some("DOES_NOT_EXIST"));
        let err = resolve_chain("A", make_lookup(items)).expect_err("missing");
        assert!(matches!(err, XmlError::MissingRequiredElement(_)));
    }

    #[test]
    fn depth_cap_enforced() {
        // Build a chain of MAX_INHERITANCE_DEPTH+1 levels.
        // We can't easily do that with &'static strings, so we use
        // a closure that fabricates names on the fly.
        let lookup = |name: &str| -> Result<Option<String>, XmlError> {
            // Always return `name + "x"` -> infinite chain.
            Ok(Some(format!("{name}x")))
        };
        let err = resolve_chain("A", lookup).expect_err("depth");
        assert!(matches!(err, XmlError::LimitExceeded(_)));
    }
}
