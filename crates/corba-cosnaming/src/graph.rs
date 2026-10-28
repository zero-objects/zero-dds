// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `NamingGraph` — the object-key registry a naming service needs on top
//! of the pure [`NamingContext`] tree: every context and every live
//! `BindingIterator` reachable over GIOP needs a stable object key, and
//! [`NamingContext`] itself (`crate::context`) knows nothing about
//! object keys, IORs or a bind address — it is the reusable spec-shape
//! data structure both this daemon and a plain in-process embedding use.
//!
//! Feature `serve` (needs `zerodds-corba-iiop` for the IIOP profile an
//! [`Ior`] carries).

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use zerodds_corba_iiop::{IiopProfileBody, IiopVersion};
use zerodds_corba_ior::{Ior, TaggedProfile};
use zerodds_wire::Endianness;

use crate::context::{BindingIterator, NamingContext};

/// The root context's well-known object key — Spec (this crate's README,
/// and the task's own wording): `corbaloc::host:2809/NameService`.
pub const ROOT_OBJECT_KEY: &[u8] = b"NameService";

/// Repository id this daemon's context objects advertise. Every
/// `NamingContext` this crate serves also implements `NamingContextExt`
/// (`to_string`/`to_name`/`to_url`/`resolve_str`), so the more derived
/// interface is the honest `type_id` — CORBA object references are
/// substitutable on the wire regardless of which name a caller narrows
/// through.
pub const NAMING_CONTEXT_TYPE_ID: &str = "IDL:omg.org/CosNaming/NamingContextExt:1.0";

/// Repository id of the base `NamingContext` interface —
/// [`NAMING_CONTEXT_TYPE_ID`]'s base, and one of the ids `_is_a`
/// (CORBA 3.3 Part 2 §15.4.2) must answer `true` for on a context object.
pub const NAMING_CONTEXT_BASE_TYPE_ID: &str = "IDL:omg.org/CosNaming/NamingContext:1.0";

/// Repository id of a `BindingIterator` object reference.
pub const BINDING_ITERATOR_TYPE_ID: &str = "IDL:omg.org/CosNaming/BindingIterator:1.0";

/// What one object key resolves to.
pub(crate) enum Bound {
    /// A `NamingContext` (root or a sub-context).
    Context(Arc<NamingContext>),
    /// A live `BindingIterator`, still being drained.
    Iterator(Mutex<BindingIterator>),
}

/// The registry: every `NamingContext` and every live `BindingIterator`
/// this server hands an object key to, keyed by that object key.
pub struct NamingGraph {
    root: Arc<NamingContext>,
    by_key: Mutex<BTreeMap<Vec<u8>, Bound>>,
    next_iterator_id: AtomicU64,
}

impl NamingGraph {
    /// A fresh, empty naming graph.
    #[must_use]
    pub fn new() -> Self {
        let root = Arc::new(NamingContext::with_id(0));
        Self::from_root(root)
    }

    /// Wraps an already-built root (typically [`crate::persist::load`]'s
    /// result) in a registry, eagerly indexing every context reachable
    /// from it — a client may address a sub-context by an object key it
    /// cached before the last restart without resolving through the
    /// root again first.
    #[must_use]
    pub fn from_root(root: Arc<NamingContext>) -> Self {
        let graph = Self {
            root,
            by_key: Mutex::new(BTreeMap::new()),
            next_iterator_id: AtomicU64::new(1),
        };
        graph.reindex();
        graph
    }

    /// The root context.
    #[must_use]
    pub fn root(&self) -> Arc<NamingContext> {
        Arc::clone(&self.root)
    }

    /// Walks every context reachable from the root and registers its
    /// object key. Idempotent — safe to call again after a bulk change
    /// (e.g. right after [`crate::persist::load`]).
    pub fn reindex(&self) {
        let mut stack = alloc::vec![Arc::clone(&self.root)];
        while let Some(ctx) = stack.pop() {
            self.register_context(&ctx);
            for (_, entry) in ctx.raw_entries() {
                if let crate::context::Entry::Context(child) = entry {
                    stack.push(child);
                }
            }
        }
    }

    fn register_context(&self, ctx: &Arc<NamingContext>) {
        let key = context_object_key(ctx);
        if let Ok(mut g) = self.by_key.lock() {
            g.entry(key)
                .or_insert_with(|| Bound::Context(Arc::clone(ctx)));
        }
    }

    /// The object key for `ctx` — the root's is [`ROOT_OBJECT_KEY`],
    /// every other context's is derived from its
    /// [`NamingContext::id`]. Registers it if this is the first time
    /// (a context [`NamingContext::new_context`] just created, not yet
    /// reachable from the root, still needs a key to hand back in its
    /// `Object` reply).
    pub fn object_key_for(&self, ctx: &Arc<NamingContext>) -> Vec<u8> {
        self.register_context(ctx);
        context_object_key(ctx)
    }

    /// The context bound to `key`, if any.
    #[must_use]
    pub fn context_for_key(&self, key: &[u8]) -> Option<Arc<NamingContext>> {
        match self.by_key.lock().ok()?.get(key)? {
            Bound::Context(c) => Some(Arc::clone(c)),
            Bound::Iterator(_) => None,
        }
    }

    /// Registers a freshly created `BindingIterator`, returning its
    /// object key.
    pub fn register_iterator(&self, iter: BindingIterator) -> Vec<u8> {
        let id = self.next_iterator_id.fetch_add(1, Ordering::Relaxed);
        let key = alloc::format!("iter-{id}").into_bytes();
        if let Ok(mut g) = self.by_key.lock() {
            g.insert(key.clone(), Bound::Iterator(Mutex::new(iter)));
        }
        key
    }

    /// Runs `f` with the iterator bound to `key` locked, if it exists.
    pub fn with_iterator<R>(
        &self,
        key: &[u8],
        f: impl FnOnce(&mut BindingIterator) -> R,
    ) -> Option<R> {
        let g = self.by_key.lock().ok()?;
        match g.get(key)? {
            Bound::Iterator(m) => m.lock().ok().map(|mut it| f(&mut it)),
            Bound::Context(_) => None,
        }
    }

    /// Releases an iterator's object key — `BindingIterator::destroy`.
    pub fn destroy_iterator(&self, key: &[u8]) {
        if let Ok(mut g) = self.by_key.lock() {
            g.remove(key);
        }
    }

    /// Whether `key` currently names a live iterator (as opposed to a
    /// context or nothing) — the dispatch table's routing question.
    #[must_use]
    pub fn is_iterator_key(&self, key: &[u8]) -> bool {
        matches!(
            self.by_key
                .lock()
                .ok()
                .and_then(|g| g.get(key).map(|b| matches!(b, Bound::Iterator(_)))),
            Some(true)
        )
    }

    /// Builds the [`Ior`] a client uses to reach `ctx` — its object key
    /// (registering it first if new) in one IIOP 1.2 profile at
    /// `host:port`.
    ///
    /// # Errors
    /// A CDR encode error from the IIOP profile codec (overflow only —
    /// `host`/`object_key` are both bounded in practice).
    pub fn ior_for_context(
        &self,
        ctx: &Arc<NamingContext>,
        host: &str,
        port: u16,
    ) -> Result<Ior, String> {
        let object_key = self.object_key_for(ctx);
        ior_for_object_key(&object_key, NAMING_CONTEXT_TYPE_ID, host, port)
    }

    /// Builds the [`Ior`] for the root context — the one published at
    /// `corbaloc::host:port/NameService`.
    ///
    /// # Errors
    /// As [`Self::ior_for_context`].
    pub fn root_ior(&self, host: &str, port: u16) -> Result<Ior, String> {
        ior_for_object_key(ROOT_OBJECT_KEY, NAMING_CONTEXT_TYPE_ID, host, port)
    }

    /// Builds the [`Ior`] for a live `BindingIterator`'s object key.
    ///
    /// # Errors
    /// As [`Self::ior_for_context`].
    pub fn ior_for_iterator_key(&self, key: &[u8], host: &str, port: u16) -> Result<Ior, String> {
        ior_for_object_key(key, BINDING_ITERATOR_TYPE_ID, host, port)
    }
}

impl Default for NamingGraph {
    fn default() -> Self {
        Self::new()
    }
}

fn context_object_key(ctx: &Arc<NamingContext>) -> Vec<u8> {
    if ctx.id() == 0 {
        ROOT_OBJECT_KEY.to_vec()
    } else {
        alloc::format!("ctx-{}", ctx.id()).into_bytes()
    }
}

fn ior_for_object_key(
    object_key: &[u8],
    type_id: &str,
    host: &str,
    port: u16,
) -> Result<Ior, String> {
    let body = IiopProfileBody::new(
        IiopVersion::V1_2,
        host.to_string(),
        port,
        object_key.to_vec(),
    );
    let profile =
        TaggedProfile::iiop(&body, Endianness::Little).map_err(|e| alloc::format!("{e:?}"))?;
    Ok(Ior {
        type_id: type_id.to_string(),
        profiles: alloc::vec![profile],
    })
}

/// The object key of an [`Ior`]'s first IIOP profile, if it has one —
/// used to recognize a `bind_context` target as one of this graph's own
/// contexts.
#[must_use]
pub fn object_key_of(ior: &Ior) -> Option<Vec<u8>> {
    ior.profiles
        .iter()
        .find_map(TaggedProfile::as_iiop)
        .and_then(Result::ok)
        .map(|body| body.object_key)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::name::NameComponent;

    #[test]
    fn root_object_key_is_name_service() {
        let g = NamingGraph::new();
        assert_eq!(context_object_key(&g.root()), ROOT_OBJECT_KEY);
    }

    #[test]
    fn a_sub_context_gets_a_distinct_stable_key() {
        let g = NamingGraph::new();
        let child = g
            .root()
            .bind_new_context(&alloc::vec![NameComponent::new("App")])
            .unwrap();
        let key1 = g.object_key_for(&child);
        let key2 = g.object_key_for(&child);
        assert_eq!(key1, key2);
        assert_ne!(key1, ROOT_OBJECT_KEY);
        assert_eq!(g.context_for_key(&key1).unwrap().id(), child.id());
    }

    #[test]
    fn reindex_finds_every_context_after_a_bulk_load() {
        let root = Arc::new(NamingContext::with_id(0));
        let mid = root
            .bind_new_context(&alloc::vec![NameComponent::new("mid")])
            .unwrap();
        let _leaf = mid
            .bind_new_context(&alloc::vec![NameComponent::new("leaf")])
            .unwrap();
        let g = NamingGraph::from_root(root);
        assert!(g.context_for_key(&context_object_key(&mid)).is_some());
    }

    #[test]
    fn iterator_registry_round_trips_and_destroy_removes_it() {
        let g = NamingGraph::new();
        let bindings = alloc::vec![crate::context::Binding {
            binding_name: NameComponent::new("x"),
            binding_type: crate::context::BindingType::Object,
        }];
        let iter = {
            // `BindingIterator::new` is crate-private; build one via
            // `NamingContext::list` the way the servant will.
            let ctx = NamingContext::new();
            ctx.bind(
                &alloc::vec![NameComponent::new("x")],
                crate::context::ObjectRef::from_stringified("IOR:00"),
            )
            .unwrap();
            let (_first, it) = ctx.list(0);
            it.unwrap()
        };
        let _ = bindings;
        let key = g.register_iterator(iter);
        assert!(g.is_iterator_key(&key));
        let n = g.with_iterator(&key, |it| it.len()).unwrap();
        assert_eq!(n, 1);
        g.destroy_iterator(&key);
        assert!(!g.is_iterator_key(&key));
        assert!(g.with_iterator(&key, |it| it.len()).is_none());
    }

    #[test]
    fn ior_for_context_round_trips_the_object_key() {
        let g = NamingGraph::new();
        let ior = g.root_ior("localhost", 2809).unwrap();
        assert_eq!(object_key_of(&ior), Some(ROOT_OBJECT_KEY.to_vec()));
    }
}
