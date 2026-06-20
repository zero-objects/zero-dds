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

/// Ordered list of name/value properties (DDS `PropertyQosPolicy`, string
/// properties only — binary properties are not yet modeled here).
///
/// ```
/// use zerodds_qos::PropertyQosPolicy;
/// let p = PropertyQosPolicy::new()
///     .with("dds.sec.log.plugin", "stderr,jsonl")
///     .with("dds.sec.log.level", "Notice");
/// assert_eq!(p.get("dds.sec.log.plugin"), Some("stderr,jsonl"));
/// assert_eq!(p.len(), 2);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertyQosPolicy {
    properties: Vec<(String, String)>,
}

impl PropertyQosPolicy {
    /// Empty policy.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder-style insert/update. Last value wins for a repeated name.
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.set(name, value);
        self
    }

    /// Insert or overwrite a property by name.
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        let value = value.into();
        if let Some(slot) = self.properties.iter_mut().find(|(n, _)| *n == name) {
            slot.1 = value;
        } else {
            self.properties.push((name, value));
        }
    }

    /// Look up a property value by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.properties
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Iterate over `(name, value)` pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.properties
            .iter()
            .map(|(n, v)| (n.as_str(), v.as_str()))
    }

    /// Number of properties.
    #[must_use]
    pub fn len(&self) -> usize {
        self.properties.len()
    }

    /// `true` if there are no properties.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
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
            .with("dds.sec.log.level", "Warning");
        assert_eq!(p.get("dds.sec.access.permissions"), Some("file:perms.p7s"));
        assert_eq!(p.get("dds.sec.log.level"), Some("Warning"));
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
    fn default_is_empty() {
        assert!(PropertyQosPolicy::default().is_empty());
    }
}
