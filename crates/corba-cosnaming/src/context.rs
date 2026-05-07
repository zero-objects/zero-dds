// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! NamingContext + NamingContextExt — Spec §2.5.4 + §3.5.1.
//!
//! In-Memory-Implementation der Spec-Operations:
//!
//! * `bind` / `rebind` (Object-Bindings)
//! * `bind_context` / `rebind_context` (Sub-Context-Bindings)
//! * `resolve` (rekursiver Lookup)
//! * `unbind` / `destroy` / `new_context` / `bind_new_context`
//! * `list` (Aufzaehlung)
//! * NamingContextExt: `to_string` / `to_name` / `resolve_str` /
//!   `to_url`.
//!
//! Object-References sind opaque `ObjectRef`-Bytes (typisch eine
//! stringified-IOR). Der Caller macht das Marshalling.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::sync::Mutex;

use zerodds_corba_ior::Ior;

use crate::error::{NamingError, NotFoundReason};
use crate::name::{Name, NameComponent};
use crate::stringified::{name_to_string, string_to_name};

/// Object-Reference fuer das Naming-Service.
///
/// Wir tragen ein voll-qualifiziertes IOR + den aufbereiteten String —
/// der Caller waehlt, was er fuer das Wire benutzt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectRef {
    /// Stringified-IOR oder corbaname-URL.
    pub stringified: String,
    /// Optional decodiertes IOR (caller-cached).
    pub ior: Option<Ior>,
}

impl ObjectRef {
    /// Konstruktor aus stringified-IOR.
    #[must_use]
    pub fn from_stringified(s: impl Into<String>) -> Self {
        Self {
            stringified: s.into(),
            ior: None,
        }
    }

    /// Konstruktor aus IOR (encoded zu stringified).
    ///
    /// # Errors
    /// IOR-Encoding-Fehler.
    pub fn from_ior(ior: Ior) -> Result<Self, zerodds_corba_ior::IorError> {
        let stringified = zerodds_corba_ior::to_stringified(&ior, zerodds_cdr::Endianness::Big)?;
        Ok(Self {
            stringified,
            ior: Some(ior),
        })
    }
}

/// Binding-Type — Spec §2.5.4 `BindingType` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingType {
    /// `nobject` — Standard-Object-Binding.
    Object,
    /// `ncontext` — Sub-Context-Binding.
    Context,
}

/// Binding-Eintrag (sichtbar aus `list`).
#[derive(Debug, Clone)]
pub struct Binding {
    /// Komponentenname (single-component Name).
    pub binding_name: NameComponent,
    /// Type.
    pub binding_type: BindingType,
}

#[derive(Debug)]
enum Entry {
    Object(ObjectRef),
    Context(Arc<NamingContext>),
}

/// NamingContext — Spec §2.5.4.
pub struct NamingContext {
    bindings: Mutex<BTreeMap<NameComponent, Entry>>,
}

impl Default for NamingContext {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for NamingContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let len = self.bindings.lock().ok().map(|g| g.len()).unwrap_or(0);
        f.debug_struct("NamingContext")
            .field("bindings", &len)
            .finish()
    }
}

impl NamingContext {
    /// Konstruktor — leerer Root-Context.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bindings: Mutex::new(BTreeMap::new()),
        }
    }

    /// Spec §2.5.4.1 `bind`. Bindet ein Object unter dem Namen.
    ///
    /// # Errors
    /// `AlreadyBound`, `NotFound`, `InvalidName`, `CannotProceed`.
    pub fn bind(&self, name: &Name, obj: ObjectRef) -> Result<(), NamingError> {
        self.bind_internal(name, Entry::Object(obj), /*allow_replace=*/ false)
    }

    /// Spec §2.5.4.3 `rebind` — wie `bind`, ueberschreibt aber.
    ///
    /// # Errors
    /// `NotFound`, `InvalidName`, `CannotProceed`.
    pub fn rebind(&self, name: &Name, obj: ObjectRef) -> Result<(), NamingError> {
        self.bind_internal(name, Entry::Object(obj), /*allow_replace=*/ true)
    }

    /// Spec §2.5.4.2 `bind_context`.
    ///
    /// # Errors
    /// `AlreadyBound`, `NotFound`, `InvalidName`, `CannotProceed`.
    pub fn bind_context(&self, name: &Name, ctx: Arc<NamingContext>) -> Result<(), NamingError> {
        self.bind_internal(name, Entry::Context(ctx), /*allow_replace=*/ false)
    }

    /// Spec §2.5.4.4 `rebind_context`.
    ///
    /// # Errors
    /// `NotFound`, `InvalidName`, `CannotProceed`.
    pub fn rebind_context(&self, name: &Name, ctx: Arc<NamingContext>) -> Result<(), NamingError> {
        self.bind_internal(name, Entry::Context(ctx), /*allow_replace=*/ true)
    }

    /// Spec §2.5.4.5 `resolve`. Liefert das gebundene Object oder
    /// einen Sub-Context-Verweis als ObjectRef-Form (Caller muss
    /// passend interpretieren).
    ///
    /// # Errors
    /// `NotFound { why: NotObject }` wenn der Pfad zu einem Context
    /// fuehrt; `NotFound { why: NotContext }` bei Pfad-Bruch;
    /// `NotFound { why: MissingNode }` bei unbekanntem Namen.
    pub fn resolve(&self, name: &Name) -> Result<ResolveResult, NamingError> {
        if name.is_empty() {
            return Err(NamingError::InvalidName);
        }
        let (head, tail) = (&name[0], &name[1..]);
        let bindings = self.lock_bindings()?;
        match bindings.get(head) {
            None => Err(NamingError::NotFound {
                why: NotFoundReason::MissingNode,
                rest_of_name: name.clone(),
            }),
            Some(Entry::Object(o)) => {
                if !tail.is_empty() {
                    Err(NamingError::NotFound {
                        why: NotFoundReason::NotContext,
                        rest_of_name: tail.to_vec(),
                    })
                } else {
                    Ok(ResolveResult::Object(o.clone()))
                }
            }
            Some(Entry::Context(ctx)) => {
                if tail.is_empty() {
                    Ok(ResolveResult::Context(Arc::clone(ctx)))
                } else {
                    let rest = tail.to_vec();
                    let next_ctx = Arc::clone(ctx);
                    drop(bindings);
                    next_ctx.resolve(&rest)
                }
            }
        }
    }

    /// Spec §2.5.4.6 `unbind`.
    ///
    /// # Errors
    /// `NotFound`, `InvalidName`.
    pub fn unbind(&self, name: &Name) -> Result<(), NamingError> {
        if name.is_empty() {
            return Err(NamingError::InvalidName);
        }
        if name.len() == 1 {
            let mut bindings = self.lock_bindings()?;
            return bindings
                .remove(&name[0])
                .map(|_| ())
                .ok_or(NamingError::NotFound {
                    why: NotFoundReason::MissingNode,
                    rest_of_name: name.clone(),
                });
        }
        // Recurse in Sub-Context.
        let (head, tail) = (&name[0], &name[1..]);
        let bindings = self.lock_bindings()?;
        match bindings.get(head) {
            Some(Entry::Context(ctx)) => {
                let next = Arc::clone(ctx);
                drop(bindings);
                next.unbind(&tail.to_vec())
            }
            Some(Entry::Object(_)) => Err(NamingError::NotFound {
                why: NotFoundReason::NotContext,
                rest_of_name: tail.to_vec(),
            }),
            None => Err(NamingError::NotFound {
                why: NotFoundReason::MissingNode,
                rest_of_name: name.clone(),
            }),
        }
    }

    /// Spec §2.5.4.7 `new_context` — leeres Sub-Context erzeugen
    /// (nicht in self bound).
    #[must_use]
    pub fn new_context(&self) -> Arc<NamingContext> {
        Arc::new(NamingContext::new())
    }

    /// Spec §2.5.4.8 `bind_new_context` — leeres Sub-Context erzeugen
    /// und sofort binden.
    ///
    /// # Errors
    /// `AlreadyBound`, `InvalidName`, `NotFound`, `CannotProceed`.
    pub fn bind_new_context(&self, name: &Name) -> Result<Arc<NamingContext>, NamingError> {
        let ctx = self.new_context();
        self.bind_context(name, Arc::clone(&ctx))?;
        Ok(ctx)
    }

    /// Spec §2.5.4.9 `destroy`.
    ///
    /// # Errors
    /// `NotEmpty`.
    pub fn destroy(&self) -> Result<(), NamingError> {
        let bindings = self.lock_bindings()?;
        if !bindings.is_empty() {
            return Err(NamingError::NotEmpty);
        }
        Ok(())
    }

    /// Spec §2.5.4.10 `list` — liefert alle Bindings als Vec.
    #[must_use]
    pub fn list(&self) -> Vec<Binding> {
        let g = match self.bindings.lock() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        g.iter()
            .map(|(k, v)| Binding {
                binding_name: k.clone(),
                binding_type: match v {
                    Entry::Object(_) => BindingType::Object,
                    Entry::Context(_) => BindingType::Context,
                },
            })
            .collect()
    }

    /// NamingContextExt §3.5.1 `resolve_str` — wie `resolve`, aber
    /// nimmt eine stringified-Name.
    ///
    /// # Errors
    /// Wie `resolve` plus `InvalidName` bei Parser-Fehlern.
    pub fn resolve_str(&self, s: &str) -> Result<ResolveResult, NamingError> {
        let name = string_to_name(s)?;
        self.resolve(&name)
    }

    /// NamingContextExt §3.5.1 `to_string` (`Name -> string`).
    ///
    /// # Errors
    /// `InvalidName` wenn `name` leer ist.
    pub fn to_string(&self, name: &Name) -> Result<String, NamingError> {
        name_to_string(name)
    }

    /// NamingContextExt §3.5.1 `to_name` (`string -> Name`).
    ///
    /// # Errors
    /// `InvalidName`.
    pub fn to_name(&self, s: &str) -> Result<Name, NamingError> {
        string_to_name(s)
    }

    /// NamingContextExt §3.5.1 `to_url` — baut eine `corbaname:`-URL.
    ///
    /// `address` ist die `iiop:host:port/object_key`-Form (Caller
    /// liefert die addr ihres NameService-Endpoints).
    ///
    /// # Errors
    /// `InvalidName` wenn `name_part` leer ist.
    pub fn to_url(&self, address: &str, name_part: &str) -> Result<String, NamingError> {
        if name_part.is_empty() {
            return Err(NamingError::InvalidName);
        }
        Ok(alloc::format!("corbaname:{address}#{name_part}"))
    }

    fn bind_internal(
        &self,
        name: &Name,
        entry: Entry,
        allow_replace: bool,
    ) -> Result<(), NamingError> {
        if name.is_empty() {
            return Err(NamingError::InvalidName);
        }
        if name.len() == 1 {
            let mut bindings = self.lock_bindings()?;
            if !allow_replace && bindings.contains_key(&name[0]) {
                return Err(NamingError::AlreadyBound);
            }
            bindings.insert(name[0].clone(), entry);
            return Ok(());
        }
        // Recurse — head muss ein bestehender Sub-Context sein.
        let (head, tail) = (&name[0], &name[1..]);
        let bindings = self.lock_bindings()?;
        match bindings.get(head) {
            Some(Entry::Context(ctx)) => {
                let next = Arc::clone(ctx);
                drop(bindings);
                next.bind_internal(&tail.to_vec(), entry, allow_replace)
            }
            Some(Entry::Object(_)) => Err(NamingError::NotFound {
                why: NotFoundReason::NotContext,
                rest_of_name: tail.to_vec(),
            }),
            None => Err(NamingError::NotFound {
                why: NotFoundReason::MissingNode,
                rest_of_name: name.clone(),
            }),
        }
    }

    fn lock_bindings(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, BTreeMap<NameComponent, Entry>>, NamingError> {
        self.bindings
            .lock()
            .map_err(|_| NamingError::CannotProceed {
                rest_of_name: Name::new(),
            })
    }
}

/// Resolve-Ergebnis — entweder ein Object oder ein Sub-Context.
#[derive(Debug, Clone)]
pub enum ResolveResult {
    /// Object-Reference.
    Object(ObjectRef),
    /// Sub-Context.
    Context(Arc<NamingContext>),
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn obj(s: &str) -> ObjectRef {
        ObjectRef::from_stringified(s)
    }

    fn nc(id: &str) -> NameComponent {
        NameComponent::new(id)
    }

    #[test]
    fn bind_and_resolve_simple() {
        let ctx = NamingContext::new();
        let n = alloc::vec![nc("x")];
        ctx.bind(&n, obj("IOR:abc")).unwrap();
        match ctx.resolve(&n).unwrap() {
            ResolveResult::Object(o) => assert_eq!(o.stringified, "IOR:abc"),
            _ => panic!("expected Object"),
        }
    }

    #[test]
    fn double_bind_yields_already_bound() {
        let ctx = NamingContext::new();
        let n = alloc::vec![nc("x")];
        ctx.bind(&n, obj("IOR:1")).unwrap();
        let err = ctx.bind(&n, obj("IOR:2")).unwrap_err();
        assert_eq!(err, NamingError::AlreadyBound);
    }

    #[test]
    fn rebind_replaces_existing() {
        let ctx = NamingContext::new();
        let n = alloc::vec![nc("x")];
        ctx.bind(&n, obj("IOR:1")).unwrap();
        ctx.rebind(&n, obj("IOR:2")).unwrap();
        match ctx.resolve(&n).unwrap() {
            ResolveResult::Object(o) => assert_eq!(o.stringified, "IOR:2"),
            _ => panic!(),
        }
    }

    #[test]
    fn nested_context_resolve() {
        let root = NamingContext::new();
        let mid = root.bind_new_context(&alloc::vec![nc("MyApp")]).unwrap();
        mid.bind(&alloc::vec![nc("Trader")], obj("IOR:trader"))
            .unwrap();
        // root.resolve("MyApp/Trader").
        let n = alloc::vec![nc("MyApp"), nc("Trader")];
        match root.resolve(&n).unwrap() {
            ResolveResult::Object(o) => assert_eq!(o.stringified, "IOR:trader"),
            _ => panic!(),
        }
    }

    #[test]
    fn resolve_to_context_returns_context() {
        let root = NamingContext::new();
        let _mid = root.bind_new_context(&alloc::vec![nc("MyApp")]).unwrap();
        match root.resolve(&alloc::vec![nc("MyApp")]).unwrap() {
            ResolveResult::Context(_) => {}
            _ => panic!("expected Context"),
        }
    }

    #[test]
    fn resolve_unknown_yields_missing_node() {
        let ctx = NamingContext::new();
        let err = ctx.resolve(&alloc::vec![nc("nope")]).unwrap_err();
        assert!(matches!(
            err,
            NamingError::NotFound {
                why: NotFoundReason::MissingNode,
                ..
            }
        ));
    }

    #[test]
    fn unbind_removes_entry() {
        let ctx = NamingContext::new();
        let n = alloc::vec![nc("x")];
        ctx.bind(&n, obj("IOR:1")).unwrap();
        ctx.unbind(&n).unwrap();
        assert!(ctx.resolve(&n).is_err());
    }

    #[test]
    fn destroy_with_bindings_yields_not_empty() {
        let ctx = NamingContext::new();
        ctx.bind(&alloc::vec![nc("x")], obj("IOR:1")).unwrap();
        let err = ctx.destroy().unwrap_err();
        assert_eq!(err, NamingError::NotEmpty);
    }

    #[test]
    fn destroy_empty_is_ok() {
        let ctx = NamingContext::new();
        ctx.destroy().unwrap();
    }

    #[test]
    fn list_returns_all_bindings_with_correct_type() {
        let ctx = NamingContext::new();
        ctx.bind(&alloc::vec![nc("o")], obj("IOR:1")).unwrap();
        let _ = ctx.bind_new_context(&alloc::vec![nc("c")]).unwrap();
        let bindings = ctx.list();
        assert_eq!(bindings.len(), 2);
        assert!(
            bindings
                .iter()
                .any(|b| b.binding_name.id == "o" && b.binding_type == BindingType::Object)
        );
        assert!(
            bindings
                .iter()
                .any(|b| b.binding_name.id == "c" && b.binding_type == BindingType::Context)
        );
    }

    #[test]
    fn resolve_str_via_naming_context_ext() {
        let root = NamingContext::new();
        let mid = root.bind_new_context(&alloc::vec![nc("App")]).unwrap();
        mid.bind(&alloc::vec![nc("Service")], obj("IOR:s")).unwrap();
        match root.resolve_str("App/Service").unwrap() {
            ResolveResult::Object(o) => assert_eq!(o.stringified, "IOR:s"),
            _ => panic!(),
        }
    }

    #[test]
    fn to_url_constructs_corbaname() {
        let ctx = NamingContext::new();
        let url = ctx.to_url(":host:2809/NameService", "App/Service").unwrap();
        assert_eq!(url, "corbaname::host:2809/NameService#App/Service");
    }

    #[test]
    fn empty_name_is_invalid() {
        let ctx = NamingContext::new();
        let err = ctx.bind(&Name::new(), obj("IOR:x")).unwrap_err();
        assert_eq!(err, NamingError::InvalidName);
    }
}
