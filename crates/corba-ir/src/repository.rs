// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Repository + container hierarchy — spec §10.5.4-§10.5.6.
//!
//! Repository is the top-level container; every container can hold
//! modules/interfaces/etc. We provide an in-memory implementation
//! with `BTreeMap` lookup by repository ID and by simple name.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::definition_kind::DefinitionKind;
use crate::error::{IrError, IrResult};
use crate::type_code::TypeCode;

/// Parameter direction of an operation (spec §10.5.20 `ParameterMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterMode {
    /// `in` parameter.
    In,
    /// `out` parameter.
    Out,
    /// `inout` parameter.
    InOut,
}

/// Operation mode (spec §10.5.20 `OperationMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMode {
    /// Normal (two-way) operation.
    Normal,
    /// `oneway` operation.
    Oneway,
}

/// Attribute mode (spec §10.5.16 `AttributeMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeMode {
    /// `readonly attribute`.
    ReadOnly,
    /// `attribute` (readable/writable).
    ReadWrite,
}

/// An operation parameter (spec §10.5.20 `ParameterDescription`).
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterDef {
    /// Parameter name.
    pub name: String,
    /// Type.
    pub type_code: TypeCode,
    /// Direction.
    pub mode: ParameterMode,
}

/// Typed detail description of a definition (spec §10.5.x `*Def`
/// interfaces) — beyond the generic `kind`/`type_code`.
#[derive(Debug, Clone, PartialEq)]
pub enum DefDetails {
    /// `OperationDef` (§10.5.20): result + parameters + mode + exceptions/contexts.
    Operation {
        /// Return TypeCode.
        result: TypeCode,
        /// Parameters in declaration order.
        params: Vec<ParameterDef>,
        /// Normal/oneway.
        mode: OperationMode,
        /// RepositoryIds of the `raises` exceptions.
        exceptions: Vec<String>,
        /// `context` property names.
        contexts: Vec<String>,
    },
    /// `AttributeDef` (§10.5.16): type + mode.
    Attribute {
        /// Attribute type.
        type_code: TypeCode,
        /// readonly/readwrite.
        mode: AttributeMode,
    },
    /// `InterfaceDef` (§10.5.18): base interfaces + abstract flag.
    Interface {
        /// RepositoryIds of the base interfaces.
        base_interfaces: Vec<String>,
        /// `abstract interface`?
        is_abstract: bool,
    },
}

/// Definition — generic container for all DefinitionKinds.
///
/// We carry the spec fields in a flat struct:
#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    /// `repository_id` (canonical `IDL:<scoped>:<m>.<n>`).
    pub repository_id: String,
    /// Simple name.
    pub name: String,
    /// Version `<m>.<n>`.
    pub version: String,
    /// Definition kind (spec §10.5.2).
    pub kind: DefinitionKind,
    /// Optional: TypeCode (for typed definitions).
    pub type_code: Option<TypeCode>,
    /// Optional: typed detail description (`OperationDef`/`AttributeDef`/…).
    pub details: Option<DefDetails>,
    /// Sub-definitions (module, interface).
    pub contents: Vec<Definition>,
}

impl Definition {
    /// Constructor for simple definitions without sub-contents.
    #[must_use]
    pub fn new(
        repository_id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        kind: DefinitionKind,
    ) -> Self {
        Self {
            repository_id: repository_id.into(),
            name: name.into(),
            version: version.into(),
            kind,
            type_code: None,
            details: None,
            contents: Vec::new(),
        }
    }

    /// Sets the TypeCode (builder).
    #[must_use]
    pub fn with_type_code(mut self, tc: TypeCode) -> Self {
        self.type_code = Some(tc);
        self
    }

    /// Sets the typed detail description (builder).
    #[must_use]
    pub fn with_details(mut self, details: DefDetails) -> Self {
        self.details = Some(details);
        self
    }

    /// Adds a sub-definition (builder).
    #[must_use]
    pub fn with_content(mut self, child: Definition) -> Self {
        self.contents.push(child);
        self
    }

    /// Looks up a nested definition by **scoped name** relative to
    /// this container (`"Iface::op"`, `"Inner::Type"`). Spec §10.5.4 `lookup`.
    #[must_use]
    pub fn lookup(&self, scoped_name: &str) -> Option<&Definition> {
        let scoped = scoped_name.trim_start_matches("::");
        let (head, rest) = match scoped.split_once("::") {
            Some((h, r)) => (h, Some(r)),
            None => (scoped, None),
        };
        let child = self.contents.iter().find(|d| d.name == head)?;
        match rest {
            Some(r) => child.lookup(r),
            None => Some(child),
        }
    }
}

/// Module — `dk_Module`-specific container.
pub type Module = Definition;

/// Container trait for Repository + Module.
pub trait Container {
    /// Returns all sub-definitions.
    fn contents(&self) -> &[Definition];
    /// Lookup by simple name (recursive through the module hierarchy).
    fn lookup_name(&self, name: &str) -> Option<&Definition>;
}

impl Container for Definition {
    fn contents(&self) -> &[Definition] {
        &self.contents
    }

    fn lookup_name(&self, name: &str) -> Option<&Definition> {
        self.contents
            .iter()
            .find(|d| d.name == name)
            .or_else(|| self.contents.iter().find_map(|d| d.lookup_name(name)))
    }
}

/// Repository — top level (spec §10.5.6).
#[derive(Debug, Clone, Default)]
pub struct Repository {
    by_repo_id: BTreeMap<String, Definition>,
}

impl Repository {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a definition under its repository ID.
    ///
    /// # Errors
    /// `DuplicateRepositoryId` if the ID is already taken.
    pub fn register(&mut self, def: Definition) -> IrResult<()> {
        if self.by_repo_id.contains_key(&def.repository_id) {
            return Err(IrError::DuplicateRepositoryId(def.repository_id.clone()));
        }
        self.by_repo_id.insert(def.repository_id.clone(), def);
        Ok(())
    }

    /// `lookup_id` (spec §10.5.6.4) — lookup by `RepositoryId`.
    #[must_use]
    pub fn lookup_id(&self, id: &str) -> Option<&Definition> {
        self.by_repo_id.get(id)
    }

    /// Top-level `lookup_name` over all registered top-level
    /// definitions.
    #[must_use]
    pub fn lookup_name(&self, name: &str) -> Option<&Definition> {
        self.by_repo_id
            .values()
            .find(|d| d.name == name)
            .or_else(|| self.by_repo_id.values().find_map(|d| d.lookup_name(name)))
    }

    /// `lookup` (spec §10.5.4.5) — resolves a **scoped name** from the
    /// repository root (`"CosNaming::NamingContext::resolve"`).
    #[must_use]
    pub fn lookup(&self, scoped_name: &str) -> Option<&Definition> {
        let scoped = scoped_name.trim_start_matches("::");
        let (head, rest) = match scoped.split_once("::") {
            Some((h, r)) => (h, Some(r)),
            None => (scoped, None),
        };
        let top = self.by_repo_id.values().find(|d| d.name == head)?;
        match rest {
            Some(r) => top.lookup(r),
            None => Some(top),
        }
    }

    /// `Contained::absolute_name` (spec §10.5.3) — fully-qualified name of the
    /// definition with `repository_id` (`"::CosNaming::NamingContext::resolve"`).
    #[must_use]
    pub fn absolute_name(&self, repository_id: &str) -> Option<String> {
        for top in self.by_repo_id.values() {
            if let Some(path) = absolute_name_walk(top, repository_id) {
                return Some(path);
            }
        }
        None
    }

    /// Number of top-level definitions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_repo_id.len()
    }

    /// `true` if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_repo_id.is_empty()
    }

    /// `describe_contents` (spec §10.5.4.7): returns all top-level
    /// definitions, optionally filtered by kind.
    #[must_use]
    pub fn describe_contents(&self, filter: Option<DefinitionKind>) -> Vec<&Definition> {
        self.by_repo_id
            .values()
            .filter(|d| filter.is_none_or(|k| k == d.kind))
            .collect()
    }

    /// Removes a definition by Repository-ID.
    pub fn remove(&mut self, id: &str) -> Option<Definition> {
        self.by_repo_id.remove(id)
    }

    /// All repository IDs (sorted).
    #[must_use]
    pub fn ids(&self) -> Vec<String> {
        self.by_repo_id.keys().map(ToString::to_string).collect()
    }
}

/// zerodds-lint: recursion-depth 16
/// Searches for `target_id` under `node` and builds the absolute `::` path.
fn absolute_name_walk(node: &Definition, target_id: &str) -> Option<String> {
    if node.repository_id == target_id {
        return Some(alloc::format!("::{}", node.name));
    }
    for child in &node.contents {
        if let Some(sub) = absolute_name_walk(child, target_id) {
            return Some(alloc::format!("::{}{}", node.name, sub));
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::type_code::{TcKind, TypeCode};

    /// Builds `module CosNaming { interface NamingContext { NamingContext resolve(in Name n); } }`.
    fn naming_repo() -> Repository {
        let resolve = Definition::new(
            "IDL:CosNaming/NamingContext/resolve:1.0",
            "resolve",
            "1.0",
            DefinitionKind::Operation,
        )
        .with_details(DefDetails::Operation {
            result: TypeCode::primitive(TcKind::ObjRef),
            params: alloc::vec![ParameterDef {
                name: "n".into(),
                type_code: TypeCode::string(0),
                mode: ParameterMode::In,
            }],
            mode: OperationMode::Normal,
            exceptions: alloc::vec!["IDL:CosNaming/NamingContext/NotFound:1.0".into()],
            contexts: Vec::new(),
        });
        let iface = Definition::new(
            "IDL:CosNaming/NamingContext:1.0",
            "NamingContext",
            "1.0",
            DefinitionKind::Interface,
        )
        .with_details(DefDetails::Interface {
            base_interfaces: Vec::new(),
            is_abstract: false,
        })
        .with_content(resolve);
        let module = Definition::new(
            "IDL:CosNaming:1.0",
            "CosNaming",
            "1.0",
            DefinitionKind::Module,
        )
        .with_content(iface);
        let mut repo = Repository::new();
        repo.register(module).unwrap();
        repo
    }

    #[test]
    fn scoped_lookup_descends_hierarchy() {
        let repo = naming_repo();
        let op = repo
            .lookup("CosNaming::NamingContext::resolve")
            .expect("scoped lookup");
        assert_eq!(op.name, "resolve");
        assert_eq!(op.kind, DefinitionKind::Operation);
        // leading :: is also allowed.
        assert!(repo.lookup("::CosNaming::NamingContext").is_some());
        assert!(repo.lookup("CosNaming::Missing").is_none());
    }

    #[test]
    fn absolute_name_builds_scoped_path() {
        let repo = naming_repo();
        assert_eq!(
            repo.absolute_name("IDL:CosNaming/NamingContext/resolve:1.0")
                .as_deref(),
            Some("::CosNaming::NamingContext::resolve")
        );
        assert_eq!(
            repo.absolute_name("IDL:CosNaming:1.0").as_deref(),
            Some("::CosNaming")
        );
        assert_eq!(repo.absolute_name("IDL:Nope:1.0"), None);
    }

    #[test]
    fn operation_def_carries_typed_signature() {
        let repo = naming_repo();
        let op = repo.lookup("CosNaming::NamingContext::resolve").unwrap();
        match op.details.as_ref().expect("op details") {
            DefDetails::Operation {
                params,
                mode,
                exceptions,
                ..
            } => {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].name, "n");
                assert_eq!(params[0].mode, ParameterMode::In);
                assert_eq!(*mode, OperationMode::Normal);
                assert_eq!(exceptions.len(), 1);
            }
            _ => panic!("expected OperationDef details"),
        }
    }

    #[test]
    fn interface_def_details() {
        let repo = naming_repo();
        let iface = repo.lookup("CosNaming::NamingContext").unwrap();
        assert!(matches!(
            iface.details,
            Some(DefDetails::Interface {
                is_abstract: false,
                ..
            })
        ));
    }

    fn echo_iface() -> Definition {
        Definition::new(
            "IDL:demo/Echo:1.0",
            "Echo",
            "1.0",
            DefinitionKind::Interface,
        )
    }

    #[test]
    fn register_and_lookup_id() {
        let mut r = Repository::new();
        r.register(echo_iface()).unwrap();
        let d = r.lookup_id("IDL:demo/Echo:1.0").unwrap();
        assert_eq!(d.name, "Echo");
    }

    #[test]
    fn duplicate_id_yields_error() {
        let mut r = Repository::new();
        r.register(echo_iface()).unwrap();
        let err = r.register(echo_iface()).unwrap_err();
        assert!(matches!(err, IrError::DuplicateRepositoryId(_)));
    }

    #[test]
    fn lookup_name_at_top_level() {
        let mut r = Repository::new();
        r.register(echo_iface()).unwrap();
        let d = r.lookup_name("Echo").unwrap();
        assert_eq!(d.repository_id, "IDL:demo/Echo:1.0");
    }

    #[test]
    fn nested_module_lookup_name_recurses() {
        let inner = Definition::new(
            "IDL:demo/Inner/Echo:1.0",
            "Echo",
            "1.0",
            DefinitionKind::Interface,
        );
        let module = Definition::new("IDL:demo/Inner:1.0", "Inner", "1.0", DefinitionKind::Module)
            .with_content(inner);
        let mut r = Repository::new();
        r.register(module).unwrap();
        let d = r.lookup_name("Echo").unwrap();
        assert_eq!(d.repository_id, "IDL:demo/Inner/Echo:1.0");
    }

    #[test]
    fn describe_contents_filters_by_kind() {
        let mut r = Repository::new();
        r.register(echo_iface()).unwrap();
        r.register(Definition::new(
            "IDL:demo/Color:1.0",
            "Color",
            "1.0",
            DefinitionKind::Enum,
        ))
        .unwrap();
        let interfaces = r.describe_contents(Some(DefinitionKind::Interface));
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].name, "Echo");
        let enums = r.describe_contents(Some(DefinitionKind::Enum));
        assert_eq!(enums.len(), 1);
        let all = r.describe_contents(None);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn definition_carries_type_code() {
        let d = Definition::new(
            "IDL:demo/Counter:1.0",
            "Counter",
            "1.0",
            DefinitionKind::Typedef,
        )
        .with_type_code(TypeCode::primitive(TcKind::Long));
        assert!(d.type_code.is_some());
        assert_eq!(d.type_code.as_ref().unwrap().kind, TcKind::Long);
    }

    #[test]
    fn remove_deletes_definition() {
        let mut r = Repository::new();
        r.register(echo_iface()).unwrap();
        let d = r.remove("IDL:demo/Echo:1.0").unwrap();
        assert_eq!(d.name, "Echo");
        assert!(r.is_empty());
    }
}
