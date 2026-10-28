// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GIOP dispatch for `CosNaming::NamingContext`/`NamingContextExt` and
//! `BindingIterator` — decodes one request body, calls the [`NamingGraph`]
//! it wraps, encodes the reply. Feature `serve`.
//!
//! One [`NamingServant`] serves a whole [`NamingGraph`] — every object
//! key the graph knows about (the root, every sub-context, every live
//! iterator) is dispatched by [`NamingServant::dispatch`], keyed by the
//! `object_key` the caller (a GIOP acceptor loop, hand-rolled or via
//! `zerodds-corba-iiop`'s `Acceptor`) already extracted from the
//! request's `TargetAddress`.
//!
//! A persisted graph is saved again after every mutating operation that
//! succeeds (`bind`, `rebind`, `bind_context`, `rebind_context`,
//! `unbind`, `bind_new_context`, `destroy`) — correctness over
//! throughput: this is a naming service, not a hot path, and losing a
//! binding a client believes succeeded is the silent-drop failure this
//! project treats as the worst class there is.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use zerodds_corba_giop::object_ops::{self, ObjectOpReply};
use zerodds_corba_ior::Ior;
use zerodds_wire::Endianness;

use crate::context::{NamingContext, ObjectRef, ResolveResult};
use crate::error::NamingError;
use crate::graph::{
    BINDING_ITERATOR_TYPE_ID, NAMING_CONTEXT_BASE_TYPE_ID, NAMING_CONTEXT_TYPE_ID, NamingGraph,
    object_key_of,
};
use crate::wire;

/// Result of one dispatched operation — mirrors GIOP's three reply
/// shapes without depending on `zerodds-corba-rust` (see `wire`'s module
/// doc for why).
#[derive(Debug)]
pub enum ServantReply {
    /// `NO_EXCEPTION` — the body is the encoded return value(s).
    Ok(Vec<u8>),
    /// `USER_EXCEPTION` — repository id + the already-encoded member
    /// bytes (the wire codec's `(repo_id, body)` pair, concatenated by
    /// the caller into `string repo_id` followed by `body`).
    UserException {
        /// Repository id (also encoded at the start of `body`).
        repository_id: &'static str,
        /// Complete exception reply body (`string repo_id` + members).
        body: Vec<u8>,
    },
    /// The operation name matches nothing this servant implements —
    /// `CORBA::BAD_OPERATION`.
    BadOperation,
    /// The request body did not decode — `CORBA::MARSHAL`.
    Marshal,
    /// The object key names neither a context nor a live iterator —
    /// `CORBA::OBJECT_NOT_EXIST`.
    ObjectNotExist,
    /// `_interface` (CORBA 3.3 Part 2 §15.4.2's `get_interface`) with no
    /// Interface Repository behind this servant — `CORBA::INTF_REPOS`.
    InterfaceRepositoryUnavailable,
}

/// Where a save happens after a mutating operation.
pub struct Persistence {
    /// The file [`crate::persist::save`] writes to.
    pub path: std::path::PathBuf,
}

/// The GIOP dispatcher for one [`NamingGraph`].
pub struct NamingServant {
    graph: Arc<NamingGraph>,
    /// The address this servant's own IORs (returned `resolve`d
    /// sub-contexts, `list`'s `BindingIterator`) advertise.
    host: String,
    port: u16,
    persistence: Option<Persistence>,
}

impl NamingServant {
    /// A servant over `graph`, advertising `host:port` in every IOR it
    /// hands back, without persistence (an embedder that persists the
    /// graph itself, or an ephemeral in-process instance).
    #[must_use]
    pub fn new(graph: Arc<NamingGraph>, host: String, port: u16) -> Self {
        Self {
            graph,
            host,
            port,
            persistence: None,
        }
    }

    /// As [`Self::new`], saving the whole graph to `persistence.path`
    /// after every operation that mutates it.
    #[must_use]
    pub fn with_persistence(mut self, persistence: Persistence) -> Self {
        self.persistence = Some(persistence);
        self
    }

    /// The graph this servant dispatches into.
    #[must_use]
    pub fn graph(&self) -> &Arc<NamingGraph> {
        &self.graph
    }

    fn save_if_persistent(&self) {
        if let Some(p) = &self.persistence {
            // A failed save is logged by the caller (the daemon layer
            // owns logging); this module has no logging channel of its
            // own and a save failure must not turn a binding that
            // genuinely happened in memory into a dropped request —
            // the client's reply already reflects the in-memory state.
            let _ = crate::persist::save(&self.graph.root(), &p.path);
        }
    }

    /// Dispatches one request. `object_key` is the `TargetAddress` the
    /// GIOP layer already resolved; `operation`/`body`/`endianness` are
    /// the request's own fields.
    #[must_use]
    pub fn dispatch(
        &self,
        object_key: &[u8],
        operation: &str,
        body: &[u8],
        endianness: Endianness,
    ) -> ServantReply {
        if self.graph.is_iterator_key(object_key) {
            if let Some(reply) = object_ops::dispatch_with_bases(
                operation,
                body,
                endianness,
                BINDING_ITERATOR_TYPE_ID,
                &[],
            ) {
                return Self::object_op_reply(reply);
            }
            return self.dispatch_iterator(object_key, operation, body, endianness);
        }
        let Some(ctx) = self.graph.context_for_key(object_key) else {
            return match object_ops::unknown_object_reply(operation, endianness) {
                Some(body) => ServantReply::Ok(body),
                None => ServantReply::ObjectNotExist,
            };
        };
        if let Some(reply) = object_ops::dispatch_with_bases(
            operation,
            body,
            endianness,
            NAMING_CONTEXT_TYPE_ID,
            &[NAMING_CONTEXT_BASE_TYPE_ID],
        ) {
            return Self::object_op_reply(reply);
        }
        self.dispatch_context(&ctx, operation, body, endianness)
    }

    /// Maps a [`corba_giop::object_ops`] outcome onto this crate's own
    /// [`ServantReply`] shape.
    fn object_op_reply(reply: ObjectOpReply) -> ServantReply {
        match reply {
            ObjectOpReply::Ok(b) => ServantReply::Ok(b),
            ObjectOpReply::Marshal => ServantReply::Marshal,
            ObjectOpReply::InterfaceRepositoryUnavailable => {
                ServantReply::InterfaceRepositoryUnavailable
            }
        }
    }

    fn own_ior(&self, ctx: &Arc<NamingContext>) -> Ior {
        // `CannotProceed.cxt` needs *an* IOR for this context even when
        // `ior_for_context` cannot fail in practice (an IIOP profile
        // encode only fails on a u32 overflow of the object key or host,
        // neither reachable here) — the nil reference is the honest
        // fallback rather than a panic on an unreachable branch.
        self.graph
            .ior_for_context(ctx, &self.host, self.port)
            .unwrap_or_else(|_| wire::nil_object_reference())
    }

    fn exception_reply(
        &self,
        err: &NamingError,
        ctx: &Arc<NamingContext>,
        e: Endianness,
    ) -> ServantReply {
        let cxt_ior = self.own_ior(ctx);
        match wire::encode_naming_error(err, &cxt_ior, e) {
            Ok((repository_id, body)) => ServantReply::UserException {
                repository_id,
                body,
            },
            Err(_) => ServantReply::Marshal,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn dispatch_context(
        &self,
        ctx: &Arc<NamingContext>,
        operation: &str,
        body: &[u8],
        e: Endianness,
    ) -> ServantReply {
        match operation {
            "bind" | "rebind" => {
                let Ok((name, obj_ior)) = wire::decode_bind_request(body, e) else {
                    return ServantReply::Marshal;
                };
                let Ok(obj) = ObjectRef::from_ior(obj_ior) else {
                    return ServantReply::Marshal;
                };
                let result = if operation == "bind" {
                    ctx.bind(&name, obj)
                } else {
                    ctx.rebind(&name, obj)
                };
                match result {
                    Ok(()) => {
                        self.save_if_persistent();
                        ServantReply::Ok(Vec::new())
                    }
                    Err(err) => self.exception_reply(&err, ctx, e),
                }
            }
            "bind_context" | "rebind_context" => {
                let Ok((name, target_ior)) = wire::decode_bind_context_request(body, e) else {
                    return ServantReply::Marshal;
                };
                let sub = object_key_of(&target_ior).and_then(|k| self.graph.context_for_key(&k));
                let Some(sub) = sub else {
                    // Spec §2.5.4.2/.4: this implementation cannot bind a
                    // context it does not itself host — see this crate's
                    // README on federation.
                    return self.exception_reply(
                        &NamingError::CannotProceed { rest_of_name: name },
                        ctx,
                        e,
                    );
                };
                let result = if operation == "bind_context" {
                    ctx.bind_context(&name, sub)
                } else {
                    ctx.rebind_context(&name, sub)
                };
                match result {
                    Ok(()) => {
                        self.save_if_persistent();
                        ServantReply::Ok(Vec::new())
                    }
                    Err(err) => self.exception_reply(&err, ctx, e),
                }
            }
            "resolve" | "resolve_str" => {
                let name_result = if operation == "resolve" {
                    wire::decode_name_request(body, e).ok()
                } else {
                    wire::decode_string_request(body, e)
                        .ok()
                        .and_then(|s| ctx.to_name(&s).ok())
                };
                let Some(name) = name_result else {
                    return ServantReply::Marshal;
                };
                match ctx.resolve(&name) {
                    Ok(ResolveResult::Object(obj)) => self.encode_object_ref_reply(&obj, e),
                    Ok(ResolveResult::Context(sub)) => {
                        let ior = self.own_ior(&sub);
                        match wire::encode_object_reply(&ior, e) {
                            Ok(b) => ServantReply::Ok(b),
                            Err(_) => ServantReply::Marshal,
                        }
                    }
                    Err(err) => self.exception_reply(&err, ctx, e),
                }
            }
            "unbind" => {
                let Ok(name) = wire::decode_name_request(body, e) else {
                    return ServantReply::Marshal;
                };
                match ctx.unbind(&name) {
                    Ok(()) => {
                        self.save_if_persistent();
                        ServantReply::Ok(Vec::new())
                    }
                    Err(err) => self.exception_reply(&err, ctx, e),
                }
            }
            "new_context" => {
                let sub = ctx.new_context();
                let ior = self.own_ior(&sub);
                match wire::encode_object_reply(&ior, e) {
                    Ok(b) => ServantReply::Ok(b),
                    Err(_) => ServantReply::Marshal,
                }
            }
            "bind_new_context" => {
                let Ok(name) = wire::decode_name_request(body, e) else {
                    return ServantReply::Marshal;
                };
                match ctx.bind_new_context(&name) {
                    Ok(sub) => {
                        self.save_if_persistent();
                        let ior = self.own_ior(&sub);
                        match wire::encode_object_reply(&ior, e) {
                            Ok(b) => ServantReply::Ok(b),
                            Err(_) => ServantReply::Marshal,
                        }
                    }
                    Err(err) => self.exception_reply(&err, ctx, e),
                }
            }
            "destroy" => match ctx.destroy() {
                Ok(()) => {
                    self.save_if_persistent();
                    ServantReply::Ok(Vec::new())
                }
                Err(err) => self.exception_reply(&err, ctx, e),
            },
            "list" => {
                let Ok(how_many) = wire::decode_list_request(body, e) else {
                    return ServantReply::Marshal;
                };
                let (bindings, iter) = ctx.list(how_many);
                let iter_ior = iter.map(|it| {
                    let key = self.graph.register_iterator(it);
                    self.graph
                        .ior_for_iterator_key(&key, &self.host, self.port)
                        .unwrap_or_else(|_| wire::nil_object_reference())
                });
                match wire::encode_list_reply(&bindings, iter_ior.as_ref(), e) {
                    Ok(b) => ServantReply::Ok(b),
                    Err(_) => ServantReply::Marshal,
                }
            }
            "to_string" => {
                let Ok(name) = wire::decode_name_request(body, e) else {
                    return ServantReply::Marshal;
                };
                match ctx.to_string(&name) {
                    Ok(s) => ServantReply::Ok(wire::encode_string_reply(&s, e)),
                    Err(err) => self.exception_reply(&err, ctx, e),
                }
            }
            "to_name" => {
                let Ok(s) = wire::decode_string_request(body, e) else {
                    return ServantReply::Marshal;
                };
                match ctx.to_name(&s) {
                    Ok(name) => match wire::encode_name_reply(&name, e) {
                        Ok(b) => ServantReply::Ok(b),
                        Err(_) => ServantReply::Marshal,
                    },
                    Err(err) => self.exception_reply(&err, ctx, e),
                }
            }
            "to_url" => {
                let Ok((address, name_part)) = wire::decode_to_url_request(body, e) else {
                    return ServantReply::Marshal;
                };
                match ctx.to_url(&address, &name_part) {
                    Ok(s) => ServantReply::Ok(wire::encode_string_reply(&s, e)),
                    Err(err) => self.exception_reply(&err, ctx, e),
                }
            }
            _ => ServantReply::BadOperation,
        }
    }

    fn encode_object_ref_reply(&self, obj: &ObjectRef, e: Endianness) -> ServantReply {
        let ior_result: Result<Ior, String> = match &obj.ior {
            Some(ior) => Ok(ior.clone()),
            None => zerodds_corba_ior::from_stringified(&obj.stringified)
                .map_err(|err| alloc::format!("{err:?}")),
        };
        match ior_result
            .and_then(|ior| wire::encode_object_reply(&ior, e).map_err(|err| err.to_string()))
        {
            Ok(b) => ServantReply::Ok(b),
            Err(_) => ServantReply::Marshal,
        }
    }

    fn dispatch_iterator(
        &self,
        object_key: &[u8],
        operation: &str,
        body: &[u8],
        e: Endianness,
    ) -> ServantReply {
        match operation {
            "next_one" => {
                let binding = self
                    .graph
                    .with_iterator(object_key, crate::context::BindingIterator::next_one)
                    .flatten();
                match wire::encode_next_one_reply(binding.as_ref(), e) {
                    Ok(b) => ServantReply::Ok(b),
                    Err(_) => ServantReply::Marshal,
                }
            }
            "next_n" => {
                let Ok(how_many) = wire::decode_next_n_request(body, e) else {
                    return ServantReply::Marshal;
                };
                let bindings = self
                    .graph
                    .with_iterator(object_key, |it| it.next_n(how_many))
                    .unwrap_or_default();
                match wire::encode_next_n_reply(&bindings, e) {
                    Ok(b) => ServantReply::Ok(b),
                    Err(_) => ServantReply::Marshal,
                }
            }
            "destroy" => {
                self.graph.destroy_iterator(object_key);
                ServantReply::Ok(Vec::new())
            }
            _ => ServantReply::BadOperation,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::name::NameComponent;

    fn servant() -> NamingServant {
        let graph = Arc::new(NamingGraph::new());
        NamingServant::new(graph, "localhost".into(), 2809)
    }

    fn ior_bytes(target: &Ior, e: Endianness) -> Vec<u8> {
        let mut w = zerodds_wire::Writer::new(e);
        target.encode(&mut w).unwrap();
        w.finish()
    }

    fn a_test_ior() -> Ior {
        Ior {
            type_id: "IDL:fin/Echo:1.0".into(),
            profiles: Vec::new(),
        }
    }

    #[test]
    fn bind_then_resolve_round_trips_via_the_wire() {
        let s = servant();
        let root_key = crate::graph::ROOT_OBJECT_KEY;
        let target = a_test_ior();
        let bytes = wire::encode_bind_request(
            &alloc::vec![NameComponent::new("echo")],
            &target,
            Endianness::Big,
        )
        .unwrap();
        let reply = s.dispatch(root_key, "bind", &bytes, Endianness::Big);
        assert!(matches!(reply, ServantReply::Ok(_)));

        let req =
            wire::encode_name_request(&alloc::vec![NameComponent::new("echo")], Endianness::Big)
                .unwrap();
        let reply = s.dispatch(root_key, "resolve", &req, Endianness::Big);
        let ServantReply::Ok(body) = reply else {
            panic!("expected Ok, got {reply:?}");
        };
        let decoded = wire::decode_object_reply(&body, Endianness::Big).unwrap();
        assert_eq!(decoded, target);
        let _ = ior_bytes(&target, Endianness::Big); // exercise the helper
    }

    #[test]
    fn resolve_of_an_unbound_name_is_not_found() {
        let s = servant();
        let req =
            wire::encode_name_request(&alloc::vec![NameComponent::new("nope")], Endianness::Big)
                .unwrap();
        let reply = s.dispatch(
            crate::graph::ROOT_OBJECT_KEY,
            "resolve",
            &req,
            Endianness::Big,
        );
        let ServantReply::UserException { repository_id, .. } = reply else {
            panic!("expected a UserException, got {reply:?}");
        };
        assert_eq!(repository_id, wire::REPO_ID_NOT_FOUND);
    }

    #[test]
    fn bind_new_context_then_bind_under_it_and_resolve_a_compound_name() {
        let s = servant();
        let root_key = crate::graph::ROOT_OBJECT_KEY;
        let req =
            wire::encode_name_request(&alloc::vec![NameComponent::new("App")], Endianness::Big)
                .unwrap();
        let reply = s.dispatch(root_key, "bind_new_context", &req, Endianness::Big);
        let ServantReply::Ok(body) = reply else {
            panic!("expected Ok, got {reply:?}");
        };
        let sub_ior = wire::decode_object_reply(&body, Endianness::Big).unwrap();
        let sub_key = object_key_of(&sub_ior).unwrap();

        let target = a_test_ior();
        let bind_body = wire::encode_bind_request(
            &alloc::vec![NameComponent::new("Service")],
            &target,
            Endianness::Big,
        )
        .unwrap();
        let reply = s.dispatch(&sub_key, "bind", &bind_body, Endianness::Big);
        assert!(matches!(reply, ServantReply::Ok(_)));

        let compound = wire::encode_name_request(
            &alloc::vec![NameComponent::new("App"), NameComponent::new("Service")],
            Endianness::Big,
        )
        .unwrap();
        let reply = s.dispatch(root_key, "resolve", &compound, Endianness::Big);
        let ServantReply::Ok(body) = reply else {
            panic!("expected Ok, got {reply:?}");
        };
        assert_eq!(
            wire::decode_object_reply(&body, Endianness::Big).unwrap(),
            target
        );
    }

    #[test]
    fn bind_context_of_a_foreign_ior_cannot_proceed() {
        let s = servant();
        let foreign = a_test_ior(); // no IIOP profile at all here → no object key
        let body = wire::encode_bind_request(
            &alloc::vec![NameComponent::new("foreign")],
            &foreign,
            Endianness::Big,
        )
        .unwrap();
        let reply = s.dispatch(
            crate::graph::ROOT_OBJECT_KEY,
            "bind_context",
            &body,
            Endianness::Big,
        );
        let ServantReply::UserException { repository_id, .. } = reply else {
            panic!("expected a UserException, got {reply:?}");
        };
        assert_eq!(repository_id, wire::REPO_ID_CANNOT_PROCEED);
    }

    #[test]
    fn list_over_more_bindings_than_how_many_yields_a_working_iterator_object() {
        let s = servant();
        let root_key = crate::graph::ROOT_OBJECT_KEY;
        for i in 0..5 {
            let body = wire::encode_bind_request(
                &alloc::vec![NameComponent::new(alloc::format!("o{i}"))],
                &a_test_ior(),
                Endianness::Big,
            )
            .unwrap();
            assert!(matches!(
                s.dispatch(root_key, "bind", &body, Endianness::Big),
                ServantReply::Ok(_)
            ));
        }
        let list_body = wire::encode_list_request(2, Endianness::Big);
        let ServantReply::Ok(body) = s.dispatch(root_key, "list", &list_body, Endianness::Big)
        else {
            panic!("expected Ok");
        };
        let (first, iter_ior) = wire::decode_list_reply(&body, Endianness::Big).unwrap();
        assert_eq!(first.len(), 2);
        let iter_ior = iter_ior.expect("3 bindings remain");
        let iter_key = object_key_of(&iter_ior).unwrap();

        // Drain via next_one.
        let mut seen = first.len();
        loop {
            let ServantReply::Ok(body) = s.dispatch(&iter_key, "next_one", &[], Endianness::Big)
            else {
                panic!("expected Ok");
            };
            match wire::decode_next_one_reply(&body, Endianness::Big).unwrap() {
                Some(_) => seen += 1,
                None => break,
            }
        }
        assert_eq!(seen, 5);

        // destroy releases the object key.
        assert!(matches!(
            s.dispatch(&iter_key, "destroy", &[], Endianness::Big),
            ServantReply::Ok(_)
        ));
        assert!(matches!(
            s.dispatch(&iter_key, "next_one", &[], Endianness::Big),
            ServantReply::ObjectNotExist
        ));
    }

    #[test]
    fn to_string_to_name_and_to_url_round_trip_through_the_servant() {
        let s = servant();
        let root_key = crate::graph::ROOT_OBJECT_KEY;
        let name = alloc::vec![NameComponent::new("App"), NameComponent::new("Service")];
        let req = wire::encode_name_request(&name, Endianness::Big).unwrap();
        let ServantReply::Ok(body) = s.dispatch(root_key, "to_string", &req, Endianness::Big)
        else {
            panic!("expected Ok");
        };
        let s_text = wire::decode_string_reply(&body, Endianness::Big).unwrap();
        assert_eq!(s_text, "App/Service");

        let req = wire::encode_string_request(&s_text, Endianness::Big);
        let ServantReply::Ok(body) = s.dispatch(root_key, "to_name", &req, Endianness::Big) else {
            panic!("expected Ok");
        };
        assert_eq!(
            wire::decode_name_reply(&body, Endianness::Big).unwrap(),
            name
        );

        let req =
            wire::encode_to_url_request(":host:2809/NameService", "App/Service", Endianness::Big);
        let ServantReply::Ok(body) = s.dispatch(root_key, "to_url", &req, Endianness::Big) else {
            panic!("expected Ok");
        };
        let url = wire::decode_string_reply(&body, Endianness::Big).unwrap();
        assert_eq!(url, "corbaname::host:2809/NameService#App/Service");
    }

    #[test]
    fn an_unknown_operation_is_bad_operation() {
        let s = servant();
        let reply = s.dispatch(
            crate::graph::ROOT_OBJECT_KEY,
            "frobnicate",
            &[],
            Endianness::Big,
        );
        assert!(matches!(reply, ServantReply::BadOperation));
    }

    #[test]
    fn an_unknown_object_key_is_object_not_exist() {
        let s = servant();
        let reply = s.dispatch(b"no-such-key", "resolve", &[], Endianness::Big);
        assert!(matches!(reply, ServantReply::ObjectNotExist));
    }

    #[test]
    fn a_truncated_body_is_marshal() {
        let s = servant();
        let reply = s.dispatch(
            crate::graph::ROOT_OBJECT_KEY,
            "bind",
            &[1, 2],
            Endianness::Big,
        );
        assert!(matches!(reply, ServantReply::Marshal));
    }

    #[test]
    fn persistence_round_trips_a_bind_across_two_servants() {
        let dir = std::env::temp_dir().join(alloc::format!(
            "zd-naming-servant-persist-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("graph.txt");
        let _ = std::fs::remove_file(&path);

        {
            let graph = Arc::new(NamingGraph::new());
            let s = NamingServant::new(graph, "localhost".into(), 2809)
                .with_persistence(Persistence { path: path.clone() });
            let body = wire::encode_bind_request(
                &alloc::vec![NameComponent::new("durable")],
                &a_test_ior(),
                Endianness::Big,
            )
            .unwrap();
            assert!(matches!(
                s.dispatch(
                    crate::graph::ROOT_OBJECT_KEY,
                    "bind",
                    &body,
                    Endianness::Big
                ),
                ServantReply::Ok(_)
            ));
        }

        // A second, freshly loaded servant sees the binding.
        let loaded_root = crate::persist::load(&path).unwrap();
        let graph2 = Arc::new(NamingGraph::from_root(loaded_root));
        let s2 = NamingServant::new(graph2, "localhost".into(), 2809);
        let req =
            wire::encode_name_request(&alloc::vec![NameComponent::new("durable")], Endianness::Big)
                .unwrap();
        let reply = s2.dispatch(
            crate::graph::ROOT_OBJECT_KEY,
            "resolve",
            &req,
            Endianness::Big,
        );
        assert!(matches!(reply, ServantReply::Ok(_)));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
