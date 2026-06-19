// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Trader **federation** (CosTrading `Link` + `Proxy`).
//!
//! Multiple traders form a graph: a [`Link`] connects one trader to another, and
//! a query follows these links across trader boundaries — bounded by a **hop
//! count** and a **follow rule** ([`FollowOption`]), with loop avoidance. A
//! [`ProxyOffer`] additionally forwards a query to a target trader and ANDs in a
//! "recipe" constraint — the spec's redirect mechanism.

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::string::String;
use alloc::vec::Vec;

use crate::constraint::ConstraintError;
use crate::property::Value;
use crate::trader::{Offer, OfferId, Preference, Trader};

/// Follow rule of a link (CosTrading `FollowOption`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowOption {
    /// Never follow the link (local offers only).
    LocalOnly,
    /// Follow the link only if the local trader returned nothing.
    IfNoLocal,
    /// Always follow the link.
    Always,
}

/// A link to another trader node (CosTrading `Link`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Name of the target node.
    pub target: String,
    /// Follow rule.
    pub follow: FollowOption,
}

/// A proxy offer (CosTrading `Proxy`): forwards queries of `service_type` to the
/// target node, ANDing in `recipe` as an additional constraint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyOffer {
    /// Service type this proxy applies to.
    pub service_type: String,
    /// Target node forwarded to.
    pub target: String,
    /// Additional constraint (ANDed with the query constraint).
    pub recipe: String,
}

/// Error from federation operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FederationError {
    /// A referenced node does not exist.
    UnknownNode(String),
}

/// A hit from a federated query: source node + offer id + offer.
pub type FederatedHit<'a> = (String, OfferId, &'a Offer);

struct Node {
    trader: Trader,
    links: Vec<Link>,
    proxies: Vec<ProxyOffer>,
}

/// A federation of multiple named trader nodes with links and proxies.
#[derive(Default)]
pub struct Federation {
    nodes: BTreeMap<String, Node>,
}

/// Combines two constraint strings with `and`; empty operands are dropped.
fn combine_constraints(a: &str, b: &str) -> String {
    match (a.trim(), b.trim()) {
        ("", "") => String::new(),
        (x, "") => String::from(x),
        ("", y) => String::from(y),
        (x, y) => alloc::format!("({x}) and ({y})"),
    }
}

impl Federation {
    /// Empty federation.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a named trader node (replacing one with the same name).
    pub fn add_node(&mut self, name: impl Into<String>, trader: Trader) {
        self.nodes.insert(
            name.into(),
            Node {
                trader,
                links: Vec::new(),
                proxies: Vec::new(),
            },
        );
    }

    /// Mutable access to a node's trader (e.g. for exporting).
    pub fn node_mut(&mut self, name: &str) -> Option<&mut Trader> {
        self.nodes.get_mut(name).map(|n| &mut n.trader)
    }

    /// Creates a link `from → to` with a follow rule.
    ///
    /// # Errors
    /// [`FederationError::UnknownNode`] if an endpoint is missing.
    pub fn link(
        &mut self,
        from: &str,
        to: &str,
        follow: FollowOption,
    ) -> Result<(), FederationError> {
        if !self.nodes.contains_key(to) {
            return Err(FederationError::UnknownNode(String::from(to)));
        }
        let node = self
            .nodes
            .get_mut(from)
            .ok_or_else(|| FederationError::UnknownNode(String::from(from)))?;
        node.links.push(Link {
            target: String::from(to),
            follow,
        });
        Ok(())
    }

    /// Registers a proxy offer at a node.
    ///
    /// # Errors
    /// [`FederationError::UnknownNode`] if the node or the target is missing.
    pub fn add_proxy(&mut self, at: &str, proxy: ProxyOffer) -> Result<(), FederationError> {
        if !self.nodes.contains_key(&proxy.target) {
            return Err(FederationError::UnknownNode(proxy.target));
        }
        let node = self
            .nodes
            .get_mut(at)
            .ok_or_else(|| FederationError::UnknownNode(String::from(at)))?;
        node.proxies.push(proxy);
        Ok(())
    }

    /// Federated query starting at `start`: collects matching offers locally and
    /// follows links (up to `max_hops` deep, with loop avoidance) as well as
    /// proxy redirects. `preference` + `how_many` are applied globally to the
    /// merged result (over static properties).
    ///
    /// # Errors
    /// [`ConstraintError`] if `constraint`/recipe cannot be parsed.
    pub fn query(
        &self,
        start: &str,
        service_type: &str,
        constraint: &str,
        preference: &Preference,
        how_many: usize,
        max_hops: usize,
    ) -> Result<Vec<FederatedHit<'_>>, ConstraintError> {
        let mut results: Vec<FederatedHit<'_>> = Vec::new();
        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        queue.push_back((String::from(start), max_hops));

        while let Some((name, hops)) = queue.pop_front() {
            if !visited.insert(name.clone()) {
                continue;
            }
            let Some(node) = self.nodes.get(&name) else {
                continue;
            };

            let local = node
                .trader
                .query(service_type, constraint, &Preference::First, 0)?;
            let had_local = !local.is_empty();
            for (id, o) in local {
                results.push((name.clone(), id, o));
            }

            // Proxy redirects: to the target node with a recipe-AND constraint.
            for px in &node.proxies {
                if px.service_type != service_type {
                    continue;
                }
                if let Some(target) = self.nodes.get(&px.target) {
                    let combined = combine_constraints(constraint, &px.recipe);
                    let hits =
                        target
                            .trader
                            .query(service_type, &combined, &Preference::First, 0)?;
                    for (id, o) in hits {
                        results.push((px.target.clone(), id, o));
                    }
                }
            }

            if hops == 0 {
                continue;
            }
            for link in &node.links {
                let follow = match link.follow {
                    FollowOption::LocalOnly => false,
                    FollowOption::IfNoLocal => !had_local,
                    FollowOption::Always => true,
                };
                if follow && !visited.contains(&link.target) {
                    queue.push_back((link.target.clone(), hops - 1));
                }
            }
        }

        apply_preference(&mut results, preference);
        if how_many > 0 && results.len() > how_many {
            results.truncate(how_many);
        }
        Ok(results)
    }
}

/// Global sort of the merged result by `preference` (static properties).
fn apply_preference(results: &mut [FederatedHit<'_>], preference: &Preference) {
    let num = |o: &Offer, p: &str| o.properties.get(p).and_then(Value::as_f64);
    match preference {
        Preference::First => {}
        Preference::Max(prop) => results.sort_by(|a, b| {
            let av = num(a.2, prop).unwrap_or(f64::NEG_INFINITY);
            let bv = num(b.2, prop).unwrap_or(f64::NEG_INFINITY);
            bv.partial_cmp(&av).unwrap_or(core::cmp::Ordering::Equal)
        }),
        Preference::Min(prop) => results.sort_by(|a, b| {
            let av = num(a.2, prop).unwrap_or(f64::INFINITY);
            let bv = num(b.2, prop).unwrap_or(f64::INFINITY);
            av.partial_cmp(&bv).unwrap_or(core::cmp::Ordering::Equal)
        }),
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp
)]
mod tests {
    use super::*;

    fn offer(ty: &str, ior: u8, speed: i64) -> Offer {
        Offer::new(ty, alloc::vec![ior]).with("Speed", Value::Int(speed))
    }

    fn three_node_chain() -> Federation {
        // a → b → c (Always links). Each node has a Printer.
        let mut f = Federation::new();
        f.add_node("a", Trader::new());
        f.add_node("b", Trader::new());
        f.add_node("c", Trader::new());
        f.node_mut("a").unwrap().export(offer("Printer", 1, 10));
        f.node_mut("b").unwrap().export(offer("Printer", 2, 20));
        f.node_mut("c").unwrap().export(offer("Printer", 3, 30));
        f.link("a", "b", FollowOption::Always).unwrap();
        f.link("b", "c", FollowOption::Always).unwrap();
        f
    }

    #[test]
    fn query_follows_links_within_hop_limit() {
        let f = three_node_chain();
        // 1 hop from a → a + b, not c.
        let r = f
            .query("a", "Printer", "", &Preference::First, 0, 1)
            .unwrap();
        let nodes: Vec<&str> = r.iter().map(|(n, _, _)| n.as_str()).collect();
        assert_eq!(nodes, alloc::vec!["a", "b"]);
        // 2 hops → a + b + c.
        let r2 = f
            .query("a", "Printer", "", &Preference::First, 0, 2)
            .unwrap();
        assert_eq!(r2.len(), 3);
    }

    #[test]
    fn local_only_link_not_followed() {
        let mut f = three_node_chain();
        // a additionally has a LocalOnly link to c — must not be followed.
        f.link("a", "c", FollowOption::LocalOnly).unwrap();
        let r = f
            .query("a", "Printer", "", &Preference::First, 0, 0)
            .unwrap();
        // 0 hops: only a.
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, "a");
    }

    #[test]
    fn if_no_local_only_follows_when_empty() {
        let mut f = Federation::new();
        f.add_node("a", Trader::new()); // a has NO offer
        f.add_node("b", Trader::new());
        f.node_mut("b").unwrap().export(offer("Printer", 2, 20));
        f.link("a", "b", FollowOption::IfNoLocal).unwrap();
        // a empty → follows to b.
        let r = f
            .query("a", "Printer", "", &Preference::First, 0, 3)
            .unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, "b");

        // Now a gets its own offer → no longer follows.
        f.node_mut("a").unwrap().export(offer("Printer", 1, 10));
        let r2 = f
            .query("a", "Printer", "", &Preference::First, 0, 3)
            .unwrap();
        assert_eq!(r2.len(), 1);
        assert_eq!(r2[0].0, "a");
    }

    #[test]
    fn cycle_is_not_infinite() {
        let mut f = three_node_chain();
        f.link("c", "a", FollowOption::Always).unwrap(); // cycle a→b→c→a
        let r = f
            .query("a", "Printer", "", &Preference::First, 0, 10)
            .unwrap();
        // Each node exactly once (loop avoidance).
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn global_preference_orders_merged_result() {
        let f = three_node_chain();
        let r = f
            .query("a", "Printer", "", &Preference::Max("Speed".into()), 0, 5)
            .unwrap();
        let speeds: Vec<f64> = r
            .iter()
            .map(|(_, _, o)| o.properties.get("Speed").and_then(Value::as_f64).unwrap())
            .collect();
        assert_eq!(speeds, alloc::vec![30.0, 20.0, 10.0]);
    }

    #[test]
    fn proxy_offer_redirects_with_recipe() {
        let mut f = Federation::new();
        f.add_node("front", Trader::new());
        f.add_node("back", Trader::new());
        // back has two printers, one fast.
        f.node_mut("back").unwrap().export(offer("Printer", 5, 5));
        f.node_mut("back").unwrap().export(offer("Printer", 9, 90));
        // front has a proxy: Printer queries → back, recipe "Speed > 50".
        f.add_proxy(
            "front",
            ProxyOffer {
                service_type: "Printer".into(),
                target: "back".into(),
                recipe: "Speed > 50".into(),
            },
        )
        .unwrap();
        // Query to front with no further condition; the proxy appends the recipe
        // → only the fast printer (Speed 90) from back.
        let r = f
            .query("front", "Printer", "", &Preference::First, 0, 0)
            .unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, "back");
        assert_eq!(r[0].2.ior, alloc::vec![9]);
    }

    #[test]
    fn unknown_node_link_errors() {
        let mut f = Federation::new();
        f.add_node("a", Trader::new());
        assert_eq!(
            f.link("a", "ghost", FollowOption::Always),
            Err(FederationError::UnknownNode("ghost".into()))
        );
    }
}
