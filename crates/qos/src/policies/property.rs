// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `PropertyQosPolicy` — ordered name/value property pairs.
//!
//! DDS-Security (§7.2.5 `DDS:Auth:*`, `DDS:Access:*`) and DDS-XML carry plugin
//! configuration as `(name, value)` properties on the participant's QoS. ZeroDDS
//! uses the same convention with the `dds.sec.*` namespace — e.g.
//! `dds.sec.access.permissions`, `dds.sec.log.plugin`. This policy is the
//! transport for that configuration; consumers (the security/logging wireup)
//! read it by key.

use alloc::string::String;
use alloc::vec::Vec;

/// One `Property_t` of the `PropertyQosPolicy` (DDS-Security 1.2 §7.3.3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry<V> {
    name: String,
    value: V,
    /// `propagate`: only an entry with `propagate = TRUE` goes out over
    /// discovery (§7.3.21).
    propagate: bool,
}

/// Ordered list of name/value properties (DDS-Security 1.2 §7.3.21
/// `PropertyQosPolicy`: `value` as `PropertySeq` and `binary_value` as
/// `BinaryPropertySeq`).
///
/// ```
/// use zerodds_qos::PropertyQosPolicy;
/// let p = PropertyQosPolicy::new()
///     .with("dds.sec.log.plugin", "stderr,jsonl")
///     .with("org.zerodds.sec.log.level", "Notice");
/// assert_eq!(p.get("dds.sec.log.plugin"), Some("stderr,jsonl"));
/// assert_eq!(p.len(), 2);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertyQosPolicy {
    properties: Vec<Entry<String>>,
    binary: Vec<Entry<Vec<u8>>>,
}

impl PropertyQosPolicy {
    /// Empty policy.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder-style insert/update with `propagate = FALSE`. Last value wins
    /// for a repeated name.
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.set(name, value);
        self
    }

    /// Builder-style insert/update with `propagate = TRUE`.
    #[must_use]
    pub fn with_propagated(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.set_propagated(name, value, true);
        self
    }

    /// Insert or overwrite a property by name, `propagate = FALSE`.
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.set_propagated(name, value, false);
    }

    /// Insert or overwrite a property by name with its `propagate` flag.
    pub fn set_propagated(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
        propagate: bool,
    ) {
        let name = name.into();
        let value = value.into();
        if let Some(slot) = self.properties.iter_mut().find(|e| e.name == name) {
            slot.value = value;
            slot.propagate = propagate;
        } else {
            self.properties.push(Entry {
                name,
                value,
                propagate,
            });
        }
    }

    /// Insert or overwrite a `BinaryProperty_t` by name.
    pub fn set_binary(&mut self, name: impl Into<String>, value: Vec<u8>, propagate: bool) {
        let name = name.into();
        if let Some(slot) = self.binary.iter_mut().find(|e| e.name == name) {
            slot.value = value;
            slot.propagate = propagate;
        } else {
            self.binary.push(Entry {
                name,
                value,
                propagate,
            });
        }
    }

    /// Look up a property value by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.properties
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.value.as_str())
    }

    /// Look up a binary property value by name.
    #[must_use]
    pub fn get_binary(&self, name: &str) -> Option<&[u8]> {
        self.binary
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.value.as_slice())
    }

    /// Iterate over `(name, value)` pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.properties
            .iter()
            .map(|e| (e.name.as_str(), e.value.as_str()))
    }

    /// The `(name, value)` pairs with `propagate = TRUE`, the ones that
    /// appear in the discovery data (§7.3.21).
    pub fn propagated(&self) -> impl Iterator<Item = (&str, &str)> {
        self.properties
            .iter()
            .filter(|e| e.propagate)
            .map(|e| (e.name.as_str(), e.value.as_str()))
    }

    /// Iterate over `(name, value, propagate)` of the string properties in
    /// insertion order.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str, bool)> {
        self.properties
            .iter()
            .map(|e| (e.name.as_str(), e.value.as_str(), e.propagate))
    }

    /// Iterate over `(name, value, propagate)` of the binary properties in
    /// insertion order.
    pub fn binary_entries(&self) -> impl Iterator<Item = (&str, &[u8], bool)> {
        self.binary
            .iter()
            .map(|e| (e.name.as_str(), e.value.as_slice(), e.propagate))
    }

    /// Iterate over the binary `(name, value)` pairs in insertion order.
    pub fn iter_binary(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.binary
            .iter()
            .map(|e| (e.name.as_str(), e.value.as_slice()))
    }

    /// The binary pairs with `propagate = TRUE`.
    pub fn propagated_binary(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.binary
            .iter()
            .filter(|e| e.propagate)
            .map(|e| (e.name.as_str(), e.value.as_slice()))
    }

    /// Number of string properties.
    #[must_use]
    pub fn len(&self) -> usize {
        self.properties.len()
    }

    /// `true` if there are neither string nor binary properties.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty() && self.binary.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn insert_get_roundtrip() {
        let p = PropertyQosPolicy::new()
            .with("dds.sec.access.permissions", "file:perms.p7s")
            .with("org.zerodds.sec.log.level", "Warning");
        assert_eq!(p.get("dds.sec.access.permissions"), Some("file:perms.p7s"));
        assert_eq!(p.get("org.zerodds.sec.log.level"), Some("Warning"));
        assert_eq!(p.get("missing"), None);
        assert_eq!(p.len(), 2);
    }

    #[test]
    fn last_value_wins_and_preserves_order() {
        let mut p = PropertyQosPolicy::new().with("a", "1").with("b", "2");
        p.set("a", "3");
        assert_eq!(p.get("a"), Some("3"));
        assert_eq!(p.len(), 2);
        let pairs: Vec<_> = p.iter().collect();
        assert_eq!(pairs, [("a", "3"), ("b", "2")]);
    }

    #[test]
    fn only_propagate_true_entries_are_propagated() {
        let mut p = PropertyQosPolicy::new()
            .with("local", "1")
            .with_propagated("shared", "2");
        p.set_binary("blob", alloc::vec![1, 2], true);
        p.set_binary("private", alloc::vec![3], false);
        assert_eq!(p.propagated().collect::<Vec<_>>(), [("shared", "2")]);
        assert_eq!(
            p.propagated_binary().collect::<Vec<_>>(),
            [("blob", &[1_u8, 2][..])]
        );
        assert_eq!(p.get_binary("private"), Some(&[3_u8][..]));
        assert!(!p.is_empty());
        // A repeated name keeps one entry and takes the new flag.
        p.set_propagated("local", "9", true);
        assert_eq!(p.len(), 2);
        assert_eq!(p.propagated().count(), 2);
    }

    /// `entries` and `binary_entries` give each entry with its `propagate`
    /// flag, in insertion order.
    #[test]
    fn the_entries_come_with_their_propagate_flag() {
        let mut p = PropertyQosPolicy::new()
            .with("a", "1")
            .with_propagated("b", "2");
        p.set_binary("c", vec![9], true);
        p.set_binary("d", vec![8, 7], false);
        assert_eq!(
            p.entries().collect::<Vec<_>>(),
            vec![("a", "1", false), ("b", "2", true)]
        );
        assert_eq!(
            p.binary_entries().collect::<Vec<_>>(),
            vec![("c", &[9u8][..], true), ("d", &[8u8, 7][..], false)]
        );
    }

    #[test]
    fn default_is_empty() {
        assert!(PropertyQosPolicy::default().is_empty());
    }
}
