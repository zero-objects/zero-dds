// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Property list — name/value pairs for plugin configuration.
//!
//! Spec OMG DDS-Security 1.1 §8.2.1 `Property_t` + `PropertyQosPolicy`.
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
/// — on a duplicate the last entry wins (as per OMG spec).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertyList {
    entries: Vec<Property>,
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
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn set_replaces_duplicate() {
        let mut list = PropertyList::new();
        list.set(Property::local("k", "v1"));
        list.set(Property::local("k", "v2"));
        assert_eq!(list.entries().len(), 1);
        assert_eq!(list.get("k"), Some("v2"));
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
