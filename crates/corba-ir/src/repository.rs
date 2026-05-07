// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Repository + Container-Hierarchie — Spec §10.5.4-§10.5.6.
//!
//! Repository ist der Top-Level-Container; jeder Container kann
//! Module/Interfaces/etc. halten. Wir liefern eine in-memory-
//! Implementation mit `BTreeMap`-Lookup nach Repository-ID und
//! nach simple-Name.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::definition_kind::DefinitionKind;
use crate::error::{IrError, IrResult};
use crate::type_code::TypeCode;

/// Definition — generischer Container fuer alle DefinitionKinds.
///
/// Wir tragen die Spec-Felder in einem flachen Struct:
#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    /// `repository_id` (kanonisch `IDL:<scoped>:<m>.<n>`).
    pub repository_id: String,
    /// Simple-Name.
    pub name: String,
    /// Version `<m>.<n>`.
    pub version: String,
    /// Definition-Kind (Spec §10.5.2).
    pub kind: DefinitionKind,
    /// Optional: TypeCode (fuer Typed-Definitions).
    pub type_code: Option<TypeCode>,
    /// Sub-Definitions (Module, Interface).
    pub contents: Vec<Definition>,
}

impl Definition {
    /// Konstruktor fuer einfache Definitionen ohne Sub-Contents.
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
            contents: Vec::new(),
        }
    }

    /// Setzt den TypeCode (Builder).
    #[must_use]
    pub fn with_type_code(mut self, tc: TypeCode) -> Self {
        self.type_code = Some(tc);
        self
    }

    /// Fuegt eine Sub-Definition hinzu (Builder).
    #[must_use]
    pub fn with_content(mut self, child: Definition) -> Self {
        self.contents.push(child);
        self
    }
}

/// Module — `dk_Module`-spezifischer Container.
pub type Module = Definition;

/// Container-Trait fuer Repository + Module.
pub trait Container {
    /// Liefert alle Sub-Definitions.
    fn contents(&self) -> &[Definition];
    /// Lookup per simple-Name (rekursiv durch Module-Hierarchie).
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

/// Repository — Top-Level (Spec §10.5.6).
#[derive(Debug, Clone, Default)]
pub struct Repository {
    by_repo_id: BTreeMap<String, Definition>,
}

impl Repository {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registriert eine Definition unter ihrer Repository-ID.
    ///
    /// # Errors
    /// `DuplicateRepositoryId` wenn ID schon belegt.
    pub fn register(&mut self, def: Definition) -> IrResult<()> {
        if self.by_repo_id.contains_key(&def.repository_id) {
            return Err(IrError::DuplicateRepositoryId(def.repository_id.clone()));
        }
        self.by_repo_id.insert(def.repository_id.clone(), def);
        Ok(())
    }

    /// `lookup_id` (Spec §10.5.6.4) — Lookup per `RepositoryId`.
    #[must_use]
    pub fn lookup_id(&self, id: &str) -> Option<&Definition> {
        self.by_repo_id.get(id)
    }

    /// Top-Level-`lookup_name` ueber alle registrierten Top-Level-
    /// Definitions.
    #[must_use]
    pub fn lookup_name(&self, name: &str) -> Option<&Definition> {
        self.by_repo_id
            .values()
            .find(|d| d.name == name)
            .or_else(|| self.by_repo_id.values().find_map(|d| d.lookup_name(name)))
    }

    /// Anzahl Top-Level-Definitionen.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_repo_id.len()
    }

    /// `true` wenn leer.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_repo_id.is_empty()
    }

    /// `describe_contents` (Spec §10.5.4.7): liefert alle Top-Level-
    /// Definitions, optional gefiltert nach Kind.
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

    /// Alle Repository-IDs (sortiert).
    #[must_use]
    pub fn ids(&self) -> Vec<String> {
        self.by_repo_id.keys().map(ToString::to_string).collect()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::type_code::{TcKind, TypeCode};

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
