// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Property-Liste — Name/Value-Paare für Plugin-Konfiguration.
//!
//! Spec OMG DDS-Security 1.1 §8.2.1 `Property_t` + `PropertyQosPolicy`.
//! Properties werden beim Participant erzeugt und an die Plugins
//! durchgereicht (z.B. Zertifikats-Pfade, HSM-URIs, Cipher-Suites).

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec::Vec;

/// Ein einzelnes Property: Name + Value.
///
/// `propagate=true` wird an Remote-Participants via SEDP gesendet (z.B.
/// `dds.sec.permissions_hash`); `false` bleibt lokal (z.B.
/// `dds.sec.auth.private_key_path` — nie ueber die Wire!).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// Key (reverse-DNS Convention, `dds.sec.xyz`).
    pub name: Cow<'static, str>,
    /// Wert (opaker UTF-8-String; Plugin interpretiert).
    pub value: Cow<'static, str>,
    /// `true` → via SEDP propagiert. Default `false` — niemals
    /// geheimnisvolle Strings an Remote leaken.
    pub propagate: bool,
}

impl Property {
    /// Konstruktor: lokal, nicht propagiert (Default).
    #[must_use]
    pub fn local(name: impl Into<Cow<'static, str>>, value: impl Into<Cow<'static, str>>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            propagate: false,
        }
    }

    /// Konstruktor: wird via SEDP propagiert.
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

/// Liste von Properties. Reihenfolge egal, Namen muessen eindeutig sein
/// — bei Duplikat gewinnt letzter Eintrag (wie OMG-Spec).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertyList {
    entries: Vec<Property>,
}

impl PropertyList {
    /// Leere Liste.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Fuegt ein Property hinzu (oder ersetzt, falls Name schon da).
    pub fn set(&mut self, p: Property) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.name == p.name) {
            *existing = p;
        } else {
            self.entries.push(p);
        }
    }

    /// Builder-Variante fuer `set`.
    #[must_use]
    pub fn with(mut self, p: Property) -> Self {
        self.set(p);
        self
    }

    /// Liefert den Wert zu einem Namen.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.value.as_ref())
    }

    /// Alle Properties (read-only).
    #[must_use]
    pub fn entries(&self) -> &[Property] {
        &self.entries
    }

    /// Nur propagierbare Properties (fuer SEDP-Wire).
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
