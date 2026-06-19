// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ServiceTypeRepository` (CosTradingRepos) — the service-type inheritance graph.
//!
//! A service type declares its property interface (with modes) and can **inherit**
//! from other service types. As a result, a query on a base type also matches
//! offers of derived types (subtype matching), and the trader can check at export
//! time whether all *mandatory* properties are present.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

/// Mode of a property declaration (CosTradingRepos `PropertyMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyMode {
    /// Optional, writable.
    Normal,
    /// Optional, immutable after export.
    ReadOnly,
    /// Mandatory property (must be set in the offer).
    Mandatory,
    /// Mandatory + immutable.
    MandatoryReadOnly,
}

impl PropertyMode {
    /// Whether this property must be present in the offer.
    #[must_use]
    pub fn is_mandatory(self) -> bool {
        matches!(self, Self::Mandatory | Self::MandatoryReadOnly)
    }
}

/// Declaration of a property on a service type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyDecl {
    /// Property name.
    pub name: String,
    /// Mode.
    pub mode: PropertyMode,
}

impl PropertyDecl {
    /// New property declaration.
    #[must_use]
    pub fn new(name: impl Into<String>, mode: PropertyMode) -> Self {
        Self {
            name: name.into(),
            mode,
        }
    }
}

/// A service type: name + super-types + its own property declarations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceType {
    /// Type name (e.g. `"ColorPrinter"`).
    pub name: String,
    /// Direct super-types (CosTradingRepos `super_types`).
    pub super_types: Vec<String>,
    /// Properties declared on this type.
    pub properties: Vec<PropertyDecl>,
}

impl ServiceType {
    /// New service type without super-types/properties.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            super_types: Vec::new(),
            properties: Vec::new(),
        }
    }

    /// Builder: inherits from a super-type.
    #[must_use]
    pub fn extends(mut self, super_type: impl Into<String>) -> Self {
        self.super_types.push(super_type.into());
        self
    }

    /// Builder: declares a property.
    #[must_use]
    pub fn prop(mut self, name: impl Into<String>, mode: PropertyMode) -> Self {
        self.properties.push(PropertyDecl::new(name, mode));
        self
    }
}

/// Error when registering a service type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeError {
    /// A referenced super-type is not registered.
    UnknownSuperType(String),
    /// A type with this name already exists.
    DuplicateType(String),
}

/// The service-type inheritance graph (CosTradingRepos `ServiceTypeRepository`).
#[derive(Debug, Clone, Default)]
pub struct ServiceTypeRepository {
    types: BTreeMap<String, ServiceType>,
}

impl ServiceTypeRepository {
    /// Empty repository.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a service type. Super-types must already be registered — this
    /// structurally rules out cycles.
    ///
    /// # Errors
    /// [`TypeError::DuplicateType`] on a name collision;
    /// [`TypeError::UnknownSuperType`] if a super-type is missing.
    pub fn add(&mut self, ty: ServiceType) -> Result<(), TypeError> {
        if self.types.contains_key(&ty.name) {
            return Err(TypeError::DuplicateType(ty.name));
        }
        for sup in &ty.super_types {
            if !self.types.contains_key(sup) {
                return Err(TypeError::UnknownSuperType(sup.clone()));
            }
        }
        self.types.insert(ty.name.clone(), ty);
        Ok(())
    }

    /// Whether a type is registered.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.types.contains_key(name)
    }

    /// Subtype relation: is `sub` equal to `base` or a (transitive) subtype?
    #[must_use]
    pub fn is_a(&self, sub: &str, base: &str) -> bool {
        if sub == base {
            return true;
        }
        // BFS over the super-type chain.
        let mut stack: Vec<&str> = Vec::new();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        stack.push(sub);
        while let Some(cur) = stack.pop() {
            if !seen.insert(cur) {
                continue;
            }
            if let Some(def) = self.types.get(cur) {
                for sup in &def.super_types {
                    if sup == base {
                        return true;
                    }
                    stack.push(sup.as_str());
                }
            }
        }
        false
    }

    /// All (transitive) super-types of `name`, including `name` itself.
    #[must_use]
    pub fn all_super_types(&self, name: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut stack: Vec<String> = alloc::vec![String::from(name)];
        while let Some(cur) = stack.pop() {
            if !seen.insert(cur.clone()) {
                continue;
            }
            out.push(cur.clone());
            if let Some(def) = self.types.get(&cur) {
                for sup in &def.super_types {
                    stack.push(sup.clone());
                }
            }
        }
        out
    }

    /// All property declarations that apply to `name` (own + inherited).
    #[must_use]
    pub fn effective_properties(&self, name: &str) -> Vec<PropertyDecl> {
        let mut out: Vec<PropertyDecl> = Vec::new();
        for ty in self.all_super_types(name) {
            if let Some(def) = self.types.get(&ty) {
                for decl in &def.properties {
                    if !out.iter().any(|d| d.name == decl.name) {
                        out.push(decl.clone());
                    }
                }
            }
        }
        out
    }

    /// Names of all *mandatory* properties of `name` (own + inherited).
    #[must_use]
    pub fn mandatory_properties(&self, name: &str) -> Vec<String> {
        self.effective_properties(name)
            .into_iter()
            .filter(|d| d.mode.is_mandatory())
            .map(|d| d.name)
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn repo() -> ServiceTypeRepository {
        let mut r = ServiceTypeRepository::new();
        r.add(ServiceType::new("Printer").prop("Speed", PropertyMode::Mandatory))
            .unwrap();
        r.add(
            ServiceType::new("ColorPrinter")
                .extends("Printer")
                .prop("Colors", PropertyMode::Normal),
        )
        .unwrap();
        r.add(
            ServiceType::new("PhotoPrinter")
                .extends("ColorPrinter")
                .prop("Dpi", PropertyMode::Mandatory),
        )
        .unwrap();
        r
    }

    #[test]
    fn is_a_transitive() {
        let r = repo();
        assert!(r.is_a("PhotoPrinter", "Printer"));
        assert!(r.is_a("ColorPrinter", "Printer"));
        assert!(r.is_a("Printer", "Printer"));
        assert!(!r.is_a("Printer", "ColorPrinter"));
        assert!(!r.is_a("PhotoPrinter", "Scanner"));
    }

    #[test]
    fn unknown_super_type_rejected() {
        let mut r = ServiceTypeRepository::new();
        let err = r.add(ServiceType::new("X").extends("Nope")).unwrap_err();
        assert_eq!(err, TypeError::UnknownSuperType("Nope".into()));
    }

    #[test]
    fn duplicate_type_rejected() {
        let mut r = ServiceTypeRepository::new();
        r.add(ServiceType::new("A")).unwrap();
        assert_eq!(
            r.add(ServiceType::new("A")).unwrap_err(),
            TypeError::DuplicateType("A".into())
        );
    }

    #[test]
    fn mandatory_properties_inherited() {
        let r = repo();
        let mut m = r.mandatory_properties("PhotoPrinter");
        m.sort();
        assert_eq!(m, alloc::vec![String::from("Dpi"), String::from("Speed")]);
    }

    #[test]
    fn effective_properties_merge_without_dup() {
        let r = repo();
        let names: Vec<String> = r
            .effective_properties("PhotoPrinter")
            .into_iter()
            .map(|d| d.name)
            .collect();
        assert!(names.contains(&"Speed".into()));
        assert!(names.contains(&"Colors".into()));
        assert!(names.contains(&"Dpi".into()));
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn all_super_types_includes_self() {
        let r = repo();
        let mut s = r.all_super_types("ColorPrinter");
        s.sort();
        assert_eq!(
            s,
            alloc::vec![String::from("ColorPrinter"), String::from("Printer")]
        );
    }
}
