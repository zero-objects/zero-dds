// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Name + NameComponent — Spec §2.4.2.
//!
//! ```text
//! struct NameComponent {
//!     Istring id;
//!     Istring kind;
//! };
//! typedef sequence<NameComponent> Name;
//! ```

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Name-Component — `id` + `kind`-Tupel.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct NameComponent {
    /// Identifier part.
    pub id: String,
    /// Kind/type tag (often empty).
    pub kind: String,
}

impl NameComponent {
    /// Constructor with only `id` (kind = empty).
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: String::new(),
        }
    }

    /// Constructor with `id` + `kind`.
    #[must_use]
    pub fn with_kind(id: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
        }
    }
}

impl<'a> From<&'a str> for NameComponent {
    fn from(s: &'a str) -> Self {
        Self::new(s.to_string())
    }
}

/// Name = `sequence<NameComponent>`.
pub type Name = Vec<NameComponent>;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn name_component_default_kind_is_empty() {
        let nc = NameComponent::new("x");
        assert_eq!(nc.id, "x");
        assert!(nc.kind.is_empty());
    }

    #[test]
    fn with_kind_sets_both_fields() {
        let nc = NameComponent::with_kind("Trader", "Server");
        assert_eq!(nc.id, "Trader");
        assert_eq!(nc.kind, "Server");
    }

    #[test]
    fn from_str_creates_id_only() {
        let nc: NameComponent = "Hello".into();
        assert_eq!(nc.id, "Hello");
        assert!(nc.kind.is_empty());
    }
}
