// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Property list — name/value pairs for plugin configuration.
//!
//! Spec OMG DDS-Security 1.2 §7.3.2 `Property_t`, §7.3.21 `PropertyQosPolicy`.
//! Properties are created on the participant and passed through to the
//! plugins (e.g. certificate paths, HSM URIs, cipher suites).

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec::Vec;

/// A single property: name + value.
///
/// `propagate=true` is sent to remote participants via SEDP (e.g.
/// `dds.sec.permissions_hash`); `false` stays local (e.g.
/// `dds.sec.auth.private_key_path` — never over the wire!).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// Key (reverse-DNS convention, `dds.sec.xyz`).
    pub name: Cow<'static, str>,
    /// Value (opaque UTF-8 string; interpreted by the plugin).
    pub value: Cow<'static, str>,
    /// `true` → propagated via SEDP. Default `false` — never
    /// leak secret strings to remote.
    pub propagate: bool,
}

impl Property {
    /// Constructor: local, not propagated (default).
    #[must_use]
    pub fn local(name: impl Into<Cow<'static, str>>, value: impl Into<Cow<'static, str>>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            propagate: false,
        }
    }

    /// Constructor: propagated via SEDP.
    #[must_use]
    pub fn propagated(
        name: impl Into<Cow<'static, str>>,
        value: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            propagate: true,
        }
    }
}

/// List of properties. Order does not matter, names must be unique
/// — on a duplicate the last entry wins (as per OMG spec). It holds the
/// `BinaryPropertySeq` of the `PropertyQosPolicy` as well (DDS-Security 1.2
/// §7.3.21, §7.3.3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertyList {
    entries: Vec<Property>,
    binary: Vec<crate::token::BinaryProperty>,
}

impl PropertyList {
    /// Empty list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a property (or replaces it, if the name already exists).
    pub fn set(&mut self, p: Property) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.name == p.name) {
            *existing = p;
        } else {
            self.entries.push(p);
        }
    }

    /// Builder variant of `set`.
    #[must_use]
    pub fn with(mut self, p: Property) -> Self {
        self.set(p);
        self
    }

    /// Removes the property with that name and returns it.
    pub fn remove(&mut self, name: &str) -> Option<Property> {
        let at = self.entries.iter().position(|p| p.name == name)?;
        Some(self.entries.remove(at))
    }

    /// Returns the value for a name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.value.as_ref())
    }

    /// All properties (read-only).
    #[must_use]
    pub fn entries(&self) -> &[Property] {
        &self.entries
    }

    /// Only propagatable properties (for the SEDP wire).
    pub fn propagatable(&self) -> impl Iterator<Item = &Property> {
        self.entries.iter().filter(|p| p.propagate)
    }

    /// Adds a binary property (or replaces it, if the name already exists).
    pub fn set_binary(&mut self, p: crate::token::BinaryProperty) {
        if let Some(existing) = self.binary.iter_mut().find(|e| e.name == p.name) {
            *existing = p;
        } else {
            self.binary.push(p);
        }
    }

    /// The value of the binary property with that name.
    #[must_use]
    pub fn get_binary(&self, name: &str) -> Option<&[u8]> {
        self.binary
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.value.as_slice())
    }

    /// All binary properties (read-only).
    #[must_use]
    pub fn binary_entries(&self) -> &[crate::token::BinaryProperty] {
        &self.binary
    }

    /// Only the binary properties with `propagate = TRUE` (for discovery).
    pub fn propagatable_binary(&self) -> impl Iterator<Item = &crate::token::BinaryProperty> {
        self.binary.iter().filter(|p| p.propagate)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// DDS-Security 1.2 §7.3.3: a BinaryProperty_t has `propagate` too; only
    /// the propagated ones are for discovery, and the flag is not on the wire
    /// of a token (§7.3.3.1 `@non-serialized`).
    #[test]
    fn binary_properties_carry_propagate_and_it_is_not_serialized() {
        use crate::token::{BinaryProperty, DataHolder};
        let mut list = PropertyList::new();
        list.set_binary(BinaryProperty::new("com.example.local", [1u8, 2]));
        list.set_binary(BinaryProperty::propagated("com.example.shared", [3u8]));
        assert_eq!(list.get_binary("com.example.local"), Some(&[1u8, 2][..]));
        let shared: Vec<_> = list
            .propagatable_binary()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(shared, ["com.example.shared"]);
        // A replaced name keeps one entry and takes the new flag.
        list.set_binary(BinaryProperty::propagated("com.example.local", [9u8]));
        assert_eq!(list.binary_entries().len(), 2);
        assert_eq!(list.propagatable_binary().count(), 2);
        // On the wire of a token the flag is gone.
        let mut holder = DataHolder::new("com.example.Token");
        holder
            .binary_properties
            .push(BinaryProperty::propagated("k", [7u8]));
        let back = DataHolder::from_cdr_le(&holder.to_cdr_le()).unwrap();
        assert_eq!(back.binary_properties[0].value, [7u8]);
        assert!(!back.binary_properties[0].propagate);
    }

    #[test]
    fn set_replaces_duplicate() {
        let mut list = PropertyList::new();
        list.set(Property::local("k", "v1"));
        list.set(Property::local("k", "v2"));
        assert_eq!(list.entries().len(), 1);
        assert_eq!(list.get("k"), Some("v2"));
    }

    #[test]
    fn remove_takes_the_named_property_and_leaves_the_rest() {
        let mut list = PropertyList::new()
            .with(Property::local("a", "1"))
            .with(Property::local("b", "2"));
        let taken = list.remove("a").unwrap();
        assert_eq!(taken.value, "1");
        assert!(list.get("a").is_none());
        assert_eq!(list.get("b"), Some("2"));
        assert!(list.remove("a").is_none(), "removing twice is a no-op");
    }

    #[test]
    fn propagatable_filter() {
        let list = PropertyList::new()
            .with(Property::local("secret", "sensitive"))
            .with(Property::propagated("public", "hashed"));
        let propagated: Vec<_> = list.propagatable().collect();
        assert_eq!(propagated.len(), 1);
        assert_eq!(propagated[0].name, "public");
    }
}
