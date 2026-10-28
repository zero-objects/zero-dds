// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! NamingContext + NamingContextExt — Spec §2.5.4 + §3.5.1.
//!
//! In-memory implementation of the spec operations:
//!
//! * `bind` / `rebind` (object bindings)
//! * `bind_context` / `rebind_context` (sub-context bindings)
//! * `resolve` (recursive lookup)
//! * `unbind` / `destroy` / `new_context` / `bind_new_context`
//! * `list` (enumeration)
//! * NamingContextExt: `to_string` / `to_name` / `resolve_str` /
//!   `to_url`.
//!
//! Object references are opaque `ObjectRef` bytes (typically a
//! stringified IOR). The caller handles the marshalling.

#[cfg(feature = "std")]
use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use alloc::string::String;
#[cfg(feature = "std")]
use alloc::sync::Arc;
use alloc::vec::Vec;
#[cfg(feature = "std")]
use core::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "std")]
use std::sync::Mutex;

use zerodds_corba_ior::Ior;

#[cfg(feature = "std")]
use crate::error::{NamingError, NotFoundReason};
#[cfg(feature = "std")]
use crate::name::Name;
use crate::name::NameComponent;
#[cfg(feature = "std")]
use crate::stringified::{name_to_string, string_to_name};

/// Object reference for the naming service.
///
/// We carry a fully qualified IOR plus the prepared string — the
/// caller chooses which one to put on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectRef {
    /// Stringified IOR or corbaname URL.
    pub stringified: String,
    /// Optional decoded IOR (caller-cached).
    pub ior: Option<Ior>,
}

impl ObjectRef {
    /// Constructor from a stringified IOR.
    #[must_use]
    pub fn from_stringified(s: impl Into<String>) -> Self {
        Self {
            stringified: s.into(),
            ior: None,
        }
    }

    /// Constructor from an IOR (encoded to stringified).
    ///
    /// # Errors
    /// IOR encoding error.
    pub fn from_ior(ior: Ior) -> Result<Self, zerodds_corba_ior::IorError> {
        let stringified = zerodds_corba_ior::to_stringified(&ior, zerodds_wire::Endianness::Big)?;
        Ok(Self {
            stringified,
            ior: Some(ior),
        })
    }
}

/// Binding type — Spec §2.5.4 `BindingType` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingType {
    /// `nobject` — standard object binding.
    Object,
    /// `ncontext` — sub-context binding.
    Context,
}

/// Binding entry (visible from `list`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// Component name (single-component name).
    pub binding_name: NameComponent,
    /// Type.
    pub binding_type: BindingType,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub(crate) enum Entry {
    Object(ObjectRef),
    Context(Arc<NamingContext>),
}

/// The next id [`NamingContext::new`] assigns. 0 is reserved for the
/// well-known root — a naming graph's root is never one of these, it is
/// given [`NamingContext::with_id`]`(0)` by whoever owns the graph (see
/// `zerodds-corba-cosnaming`'s `NamingGraph`, feature `std`).
#[cfg(feature = "std")]
static NEXT_CONTEXT_ID: AtomicU64 = AtomicU64::new(1);

/// NamingContext — Spec §2.5.4.
#[cfg(feature = "std")]
pub struct NamingContext {
    id: u64,
    bindings: Mutex<BTreeMap<NameComponent, Entry>>,
}

#[cfg(feature = "std")]
impl Default for NamingContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
impl core::fmt::Debug for NamingContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let len = self.bindings.lock().ok().map(|g| g.len()).unwrap_or(0);
        f.debug_struct("NamingContext")
            .field("id", &self.id)
            .field("bindings", &len)
            .finish()
    }
}

#[cfg(feature = "std")]
impl NamingContext {
    /// Constructor — empty context with a freshly allocated id.
    #[must_use]
    pub fn new() -> Self {
        Self::with_id(NEXT_CONTEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Constructor with an explicit id — a naming graph's own object-key
    /// registry uses it as the stable key of this context (`id` 0 is
    /// conventionally the graph's root), and reloading a persisted graph
    /// reconstructs every node with the id it was saved under so that an
    /// IOR a client already holds keeps resolving after a restart.
    ///
    /// Also advances the counter [`Self::new`] draws from past `id`, so a
    /// context created afterwards never collides with one loaded from a
    /// file.
    #[must_use]
    pub fn with_id(id: u64) -> Self {
        NEXT_CONTEXT_ID.fetch_max(id.saturating_add(1), Ordering::Relaxed);
        Self {
            id,
            bindings: Mutex::new(BTreeMap::new()),
        }
    }

    /// This context's stable id (see [`Self::with_id`]).
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// Spec §2.5.4.1 `bind`. Binds an object under the name.
    ///
    /// # Errors
    /// `AlreadyBound`, `NotFound`, `InvalidName`, `CannotProceed`.
    pub fn bind(&self, name: &Name, obj: ObjectRef) -> Result<(), NamingError> {
        self.bind_internal(name, Entry::Object(obj), /*allow_replace=*/ false)
    }

    /// Spec §2.5.4.3 `rebind` — like `bind`, but overwrites.
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

    /// Spec §2.5.4.5 `resolve`. Returns the bound object or a
    /// sub-context reference in ObjectRef form (the caller must
    /// interpret it accordingly).
    ///
    /// # Errors
    /// `NotFound { why: NotObject }` when the path leads to a context;
    /// `NotFound { why: NotContext }` on a path break;
    /// `NotFound { why: MissingNode }` on an unknown name.
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
        // Recurse into the sub-context.
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

    /// Spec §2.5.4.7 `new_context` — create an empty sub-context
    /// (not bound into self).
    #[must_use]
    pub fn new_context(&self) -> Arc<NamingContext> {
        Arc::new(NamingContext::new())
    }

    /// Spec §2.5.4.8 `bind_new_context` — create an empty sub-context
    /// and bind it immediately.
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

    /// Spec §2.5.4.10 `list(how_many, bl, bi)`. Returns up to `how_many`
    /// bindings directly (`BindingList`); if more remain, the rest is a
    /// [`BindingIterator`] the caller drives with [`BindingIterator::next_one`]
    /// or [`BindingIterator::next_n`] — `None` when `how_many` already
    /// covered every binding.
    #[must_use]
    pub fn list(&self, how_many: u32) -> (Vec<Binding>, Option<BindingIterator>) {
        let mut all = self.all_bindings();
        let n = (how_many as usize).min(all.len());
        let rest: Vec<Binding> = all.split_off(n);
        let iter = if rest.is_empty() {
            None
        } else {
            Some(BindingIterator::new(rest))
        };
        (all, iter)
    }

    /// Every binding, in one `Vec` — the whole-graph views persistence and
    /// diagnostics need, where `list`'s `how_many` split would be in the way.
    #[must_use]
    pub fn all_bindings(&self) -> Vec<Binding> {
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

    /// Every `(name, entry)` pair, entries cloned out from under the lock —
    /// a persisted snapshot's source, and how a caller reaches a bound
    /// sub-context's own [`NamingContext`] without going through `resolve`
    /// (which returns [`ObjectRef`]/[`ResolveResult`], not the `Arc` itself).
    #[must_use]
    pub(crate) fn raw_entries(&self) -> Vec<(NameComponent, Entry)> {
        let g = match self.bindings.lock() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        g.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    /// Inserts a raw entry directly — bypasses `bind`'s duplicate check.
    /// Used only to rebuild a [`NamingContext`] from a persisted snapshot,
    /// where every name is bound exactly once by construction.
    pub(crate) fn insert_raw(&self, name: NameComponent, entry: Entry) {
        if let Ok(mut g) = self.bindings.lock() {
            g.insert(name, entry);
        }
    }

    /// NamingContextExt §3.5.1 `resolve_str` — like `resolve`, but
    /// takes a stringified name.
    ///
    /// # Errors
    /// Like `resolve` plus `InvalidName` on parser errors.
    pub fn resolve_str(&self, s: &str) -> Result<ResolveResult, NamingError> {
        let name = string_to_name(s)?;
        self.resolve(&name)
    }

    /// NamingContextExt §3.5.1 `to_string` (`Name -> string`).
    ///
    /// # Errors
    /// `InvalidName` when `name` is empty.
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

    /// NamingContextExt §3.5.1 `to_url` — builds a `corbaname:` URL.
    ///
    /// `address` is the `iiop:host:port/object_key` form (the caller
    /// supplies the address of its NameService endpoint).
    ///
    /// # Errors
    /// `InvalidName` when `name_part` is empty.
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
        // Recurse — head must be an existing sub-context.
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

/// Resolve result — either an object or a sub-context.
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub enum ResolveResult {
    /// Object reference.
    Object(ObjectRef),
    /// Sub-context.
    Context(Arc<NamingContext>),
}

/// `CosNaming::BindingIterator` — Spec §2.5.5. The tail [`NamingContext::list`]
/// did not return directly, driven one [`Self::next_one`] or batch
/// [`Self::next_n`] at a time.
///
/// Holds its own copy of the remaining bindings — `list` took the
/// snapshot under the context's lock, so the iterator does not see a
/// bind or unbind that happens while it is being drained, matching the
/// spec's silence on concurrent modification (an implementation may
/// either reflect or not reflect later changes; a stable snapshot is the
/// simpler contract to hand a caller and the one every reference
/// implementation measured for this project's CORBA column gives).
#[derive(Debug, Default)]
pub struct BindingIterator {
    remaining: VecDeque<Binding>,
}

impl BindingIterator {
    #[cfg(feature = "std")]
    fn new(remaining: Vec<Binding>) -> Self {
        Self {
            remaining: remaining.into(),
        }
    }

    /// Spec §2.5.5.1 `next_one` — the next binding, or `false` (nothing
    /// written) once the iterator is exhausted.
    pub fn next_one(&mut self) -> Option<Binding> {
        self.remaining.pop_front()
    }

    /// Spec §2.5.5.2 `next_n` — up to `how_many` bindings; the returned
    /// `Vec` is shorter than `how_many` exactly when the iterator ran out.
    pub fn next_n(&mut self, how_many: u32) -> Vec<Binding> {
        let n = (how_many as usize).min(self.remaining.len());
        self.remaining.drain(..n).collect()
    }

    /// Spec §2.5.5.3 `destroy` — releases the iterator. An in-memory
    /// snapshot holds no resource beyond `self`; the servant that dropped
    /// this instance out of its own registry already did the releasing
    /// [`Self::destroy`] exists to let a caller ask for explicitly.
    pub fn destroy(self) {}

    /// `true` once every binding has been returned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    /// Bindings not yet returned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.remaining.len()
    }
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
        let (bindings, iter) = ctx.list(u32::MAX);
        assert_eq!(bindings.len(), 2);
        assert!(iter.is_none(), "how_many covered every binding");
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
    fn list_with_how_many_zero_returns_the_iterator_only() {
        let ctx = NamingContext::new();
        ctx.bind(&alloc::vec![nc("x")], obj("IOR:1")).unwrap();
        let (first, iter) = ctx.list(0);
        assert!(first.is_empty());
        let mut iter = iter.expect("one binding is left for the iterator");
        assert_eq!(iter.len(), 1);
        assert_eq!(iter.next_one().unwrap().binding_name.id, "x");
        assert!(iter.next_one().is_none());
    }

    /// Spec §2.5.4.10: `list` hands back at most `how_many` directly and a
    /// `BindingIterator` over the rest. More bindings than `how_many`.
    #[test]
    fn list_over_more_bindings_than_how_many_yields_a_draining_iterator() {
        let ctx = NamingContext::new();
        for i in 0..7 {
            ctx.bind(&alloc::vec![nc(&alloc::format!("o{i}"))], obj("IOR:x"))
                .unwrap();
        }
        let (first, iter) = ctx.list(3);
        assert_eq!(first.len(), 3);
        let mut iter = iter.expect("4 bindings remain");
        assert_eq!(iter.len(), 4);

        // next_n takes fewer than remain.
        let batch = iter.next_n(2);
        assert_eq!(batch.len(), 2);
        assert_eq!(iter.len(), 2);

        // next_one drains the rest one at a time.
        let mut seen = first.len() + batch.len();
        while let Some(_binding) = iter.next_one() {
            seen += 1;
        }
        assert_eq!(seen, 7);
        assert!(iter.is_empty());

        // next_n past the end returns fewer than asked, not an error.
        assert!(iter.next_n(5).is_empty());
        iter.destroy();
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
