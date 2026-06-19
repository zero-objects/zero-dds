// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `base_name` resolver with cycle detection.
//!
//! DDS-XML 1.0 allows several building blocks (QoS profiles §7.3.2.4.2,
//! domain §7.3.4.4.2, domain participant §7.3.5.4.3) a `base_name`-attribute-
//! based inheritance. The spec requires the base definition to come before the
//! inheriting definition — naive implementations can nevertheless create
//! cycles across library boundaries.
//!
//! This module implements a generic inheritance resolution with
//! DAG checking. The resolution routine is parameterized over the
//! item type (e.g. QoS profile, domain, participant) and the `base_name`
//! lookup function.

use crate::errors::XmlError;
use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Maximum inheritance depth (DoS cap).
pub const MAX_INHERITANCE_DEPTH: usize = 32;

/// Resolves a `base_name`-chain starting at `name` and returns the
/// chain in **base-first** order, i.e. `[grandparent, parent, name]`.
///
/// The order enables callers to "merge" fields step by step
/// (base defaults first, then override).
///
/// # Parameters
/// * `name` — start point of the resolution.
/// * `lookup` — closure that, for a `name`, returns its `base_name` (or
///   `None` if no base is present). If the `name`
///   itself does not exist, [`XmlError::MissingRequiredElement`]
///   should be returned.
///
/// # Errors
/// * [`XmlError::CircularInheritance`] — if a cycle is detected.
/// * [`XmlError::LimitExceeded`] — if [`MAX_INHERITANCE_DEPTH`]
///   is exceeded.
/// * Errors from the `lookup` closure are passed through.
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
            // Cycle: `current` was already visited.
            chain.push(current.clone());
            let pretty = chain.join(" -> ");
            return Err(XmlError::CircularInheritance(pretty));
        }
        chain.push(current.clone());

        match lookup(&current)? {
            None => {
                // No base entry: resolution finished.
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
