// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `CORBA::Object` operations sent as GIOP requests — spec §15.4.2.
//!
//! CORBA 3.3 Part 2 §15.4.2 names six operations a client may send as an
//! ordinary GIOP [`crate::Request`] against any object reference, whether
//! or not the target interface declares them: `_interface`, `_is_a`,
//! `_non_existent` (`_not_existent` before CORBA 2.3), `_domain_managers`,
//! `_component`, `_repository_id`. Part 1 §8.3 gives their semantics on
//! `CORBA::Object` itself.
//!
//! This lives in the GIOP crate rather than `crates/corba-poa` (which
//! owns servant *routing* — deciding which servant answers an object
//! key) because the spec frames these six as requests the ORB run-time
//! answers itself, before or instead of reaching a servant — a
//! GIOP-level interception, not a POA policy decision, and §15.4.2 is
//! itself a GIOP-chapter section. Every GIOP-speaking server in this
//! workspace calls into this module from its own request-handling loop
//! instead of re-deriving the six names and their wire shapes.
//!
//! No CORBA 3.3 spec text is cached in this repository for this module:
//! `crate` doc references `internal/standards/cache/omg/corba-3.3-part2.pdf`
//! for GIOP itself, but nothing under `docs/spec-coverage`,
//! `documentation/specs` or that cache directory covers Part 1 §8.3's
//! `non_existent`/`get_interface` wording. The six operation names and
//! the `_is_a`/`_domain_managers`/`_component` shapes below come from the
//! bug report that led to this module (a captured omniORB `nameclt`
//! request/reply pair); the `non_existent`-on-an-unknown-key and
//! `_interface`-without-an-Interface-Repository behavior are implemented
//! from memory of the specification, not verified against spec text —
//! each such choice says so at the point it is made.

use alloc::vec::Vec;

use zerodds_wire::{Endianness, Reader, Writer};

/// `get_interface()` (Part 1 §4.3.1), sent over GIOP as `_interface`.
pub const OP_INTERFACE: &str = "_interface";
/// `is_a(in string logical_type_id)`, sent over GIOP as `_is_a`.
pub const OP_IS_A: &str = "_is_a";
/// `non_existent()`, sent over GIOP as `_non_existent` — CORBA 2.3+.
pub const OP_NON_EXISTENT: &str = "_non_existent";
/// The pre-2.3 spelling of [`OP_NON_EXISTENT`] some ORBs still send.
pub const OP_NOT_EXISTENT: &str = "_not_existent";
/// `get_domain_managers()`, sent over GIOP as `_domain_managers`.
pub const OP_DOMAIN_MANAGERS: &str = "_domain_managers";
/// `get_component()`, sent over GIOP as `_component`.
pub const OP_COMPONENT: &str = "_component";
/// The object's most-derived repository id, sent over GIOP as
/// `_repository_id`.
pub const OP_REPOSITORY_ID: &str = "_repository_id";

/// Repository id every object reference answers `_is_a` `true` for —
/// the root of the object-reference type hierarchy (Part 1 §4.3.1).
pub const OBJECT_REPOSITORY_ID: &str = "IDL:omg.org/CORBA/Object:1.0";

/// `true` for every operation name §15.4.2 lists — a caller whose own
/// dispatch would otherwise answer `BAD_OPERATION` for one of these
/// must route it through [`dispatch`]/[`dispatch_with_bases`] instead.
#[must_use]
pub fn is_object_operation(operation: &str) -> bool {
    matches!(
        operation,
        OP_INTERFACE
            | OP_IS_A
            | OP_NON_EXISTENT
            | OP_NOT_EXISTENT
            | OP_DOMAIN_MANAGERS
            | OP_COMPONENT
            | OP_REPOSITORY_ID
    )
}

/// The reply to one of the six operations, for a server that has
/// already resolved the target object — see [`unknown_object_reply`]
/// for the "no such object key here" case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectOpReply {
    /// `NO_EXCEPTION` — already-encoded CDR reply body.
    Ok(Vec<u8>),
    /// The request body did not decode as the operation's argument
    /// (`_is_a`'s `string`) — `CORBA::MARSHAL`.
    Marshal,
    /// `_interface` (`get_interface`) with no Interface Repository
    /// behind the server — `CORBA::INTF_REPOS`. From memory (not
    /// verified against spec text, see the module doc): Part 1
    /// §3.17.1 lists `INTF_REPOS` as raised when "the Interface
    /// Repository is unavailable or some error was encountered while
    /// attempting to use it", which is exactly this case, and no
    /// server in this workspace has an Interface Repository behind it.
    InterfaceRepositoryUnavailable,
}

/// Dispatches one of the six operations against an object the caller
/// has already resolved.
///
/// `repository_id` is the object's most-derived type; `is_a` answers
/// whether the object also implements a given repository id (its
/// base-interface chain) — [`OBJECT_REPOSITORY_ID`] and
/// `repository_id` itself do not need to be included in what `is_a`
/// accepts, this function checks those first.
///
/// `None` when `operation` is not one of the six — the caller's own
/// dispatch continues unchanged.
pub fn dispatch(
    operation: &str,
    body: &[u8],
    endianness: Endianness,
    repository_id: &str,
    is_a: &dyn Fn(&str) -> bool,
) -> Option<ObjectOpReply> {
    match operation {
        OP_IS_A => {
            let mut r = Reader::new(body, endianness);
            let Ok(candidate) = r.read_string() else {
                return Some(ObjectOpReply::Marshal);
            };
            let matches =
                candidate == OBJECT_REPOSITORY_ID || candidate == repository_id || is_a(&candidate);
            Some(ObjectOpReply::Ok(encode_bool(matches, endianness)))
        }
        OP_NON_EXISTENT | OP_NOT_EXISTENT => {
            // The object is resolved — it exists.
            Some(ObjectOpReply::Ok(encode_bool(false, endianness)))
        }
        OP_REPOSITORY_ID => Some(ObjectOpReply::Ok(encode_string(repository_id, endianness))),
        OP_INTERFACE => Some(ObjectOpReply::InterfaceRepositoryUnavailable),
        OP_DOMAIN_MANAGERS => Some(ObjectOpReply::Ok(encode_empty_sequence(endianness))),
        OP_COMPONENT => {
            // Nil object reference — CORBA 3.3 Part 2 §9.3.4 marshals an
            // object reference as its IOR (`string type_id; sequence
            // <TaggedProfile> profiles`); nil is the empty-type-id,
            // no-profiles form. No server in this workspace hosts a CCM
            // component reachable over GIOP, so this is unconditional;
            // a future one would need a way to supply the real
            // reference instead.
            Some(ObjectOpReply::Ok(encode_nil_object_reference(endianness)))
        }
        _ => None,
    }
}

/// As [`dispatch`], for a caller whose base-interface chain is a plain
/// list of repository ids rather than a predicate closure — the
/// callers that do not implement `corba_poa::Servant` (a `NamingContext`
/// / `NamingContextExt` / `BindingIterator` object, or a bridge's
/// configured `repo_id` mapping).
pub fn dispatch_with_bases(
    operation: &str,
    body: &[u8],
    endianness: Endianness,
    repository_id: &str,
    base_repository_ids: &[&str],
) -> Option<ObjectOpReply> {
    dispatch(operation, body, endianness, repository_id, &|id| {
        base_repository_ids.contains(&id)
    })
}

/// `_non_existent`/`_not_existent` sent for an object key the server
/// has no identity for at all. Unlike every other operation §15.4.2
/// lists, this one is defined to answer rather than raise
/// `OBJECT_NOT_EXIST` — from memory of Part 1 §8.3's `non_existent()`
/// (not verified against spec text, see the module doc): the whole
/// point of the operation is to let a client find out an object does
/// not exist without the exception a normal invocation on it would
/// raise, so an unknown key answers `true` here rather than falling
/// back to the exception every other operation gets.
///
/// `None` for any other operation — the caller's normal "unknown
/// object key" handling (`OBJECT_NOT_EXIST`) applies.
#[must_use]
pub fn unknown_object_reply(operation: &str, endianness: Endianness) -> Option<Vec<u8>> {
    matches!(operation, OP_NON_EXISTENT | OP_NOT_EXISTENT).then(|| encode_bool(true, endianness))
}

/// Encodes a `boolean` reply body — also useful to a caller building
/// its own `_is_a`/`_non_existent` reply outside [`dispatch`] (e.g. a
/// LocateRequest handler that wants the identical wire shape).
#[must_use]
pub fn encode_bool(value: bool, endianness: Endianness) -> Vec<u8> {
    let mut w = Writer::new(endianness);
    w.write_bool(value);
    w.finish()
}

fn encode_string(value: &str, endianness: Endianness) -> Vec<u8> {
    let mut w = Writer::new(endianness);
    w.write_string(value);
    w.finish()
}

fn encode_empty_sequence(endianness: Endianness) -> Vec<u8> {
    let mut w = Writer::new(endianness);
    w.write_u32(0);
    w.finish()
}

fn encode_nil_object_reference(endianness: Endianness) -> Vec<u8> {
    let mut w = Writer::new(endianness);
    w.write_string("");
    w.write_u32(0);
    w.finish()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn encode_is_a_request(id: &str, e: Endianness) -> Vec<u8> {
        let mut w = Writer::new(e);
        w.write_string(id);
        w.finish()
    }

    fn decode_bool(body: &[u8], e: Endianness) -> bool {
        let mut r = Reader::new(body, e);
        r.read_bool().unwrap()
    }

    #[test]
    fn is_object_operation_recognizes_all_six_and_the_legacy_spelling() {
        for op in [
            OP_INTERFACE,
            OP_IS_A,
            OP_NON_EXISTENT,
            OP_NOT_EXISTENT,
            OP_DOMAIN_MANAGERS,
            OP_COMPONENT,
            OP_REPOSITORY_ID,
        ] {
            assert!(is_object_operation(op));
        }
        assert!(!is_object_operation("frobnicate"));
    }

    #[test]
    fn is_a_true_for_own_repository_id() {
        let body = encode_is_a_request("IDL:demo/Echo:1.0", Endianness::Big);
        let reply = dispatch(
            OP_IS_A,
            &body,
            Endianness::Big,
            "IDL:demo/Echo:1.0",
            &|_| false,
        );
        let Some(ObjectOpReply::Ok(b)) = reply else {
            panic!("expected Ok");
        };
        assert!(decode_bool(&b, Endianness::Big));
    }

    #[test]
    fn is_a_true_for_corba_object_universal_base() {
        let body = encode_is_a_request(OBJECT_REPOSITORY_ID, Endianness::Little);
        let reply = dispatch(
            OP_IS_A,
            &body,
            Endianness::Little,
            "IDL:demo/Echo:1.0",
            &|_| false,
        );
        let Some(ObjectOpReply::Ok(b)) = reply else {
            panic!("expected Ok");
        };
        assert!(decode_bool(&b, Endianness::Little));
    }

    #[test]
    fn is_a_true_for_a_declared_base_interface() {
        let body = encode_is_a_request("IDL:omg.org/CosNaming/NamingContext:1.0", Endianness::Big);
        let reply = dispatch_with_bases(
            OP_IS_A,
            &body,
            Endianness::Big,
            "IDL:omg.org/CosNaming/NamingContextExt:1.0",
            &["IDL:omg.org/CosNaming/NamingContext:1.0"],
        );
        let Some(ObjectOpReply::Ok(b)) = reply else {
            panic!("expected Ok");
        };
        assert!(decode_bool(&b, Endianness::Big));
    }

    #[test]
    fn is_a_false_for_a_foreign_repository_id() {
        let body =
            encode_is_a_request("IDL:omg.org/CosNaming/BindingIterator:1.0", Endianness::Big);
        let reply = dispatch_with_bases(
            OP_IS_A,
            &body,
            Endianness::Big,
            "IDL:omg.org/CosNaming/NamingContextExt:1.0",
            &["IDL:omg.org/CosNaming/NamingContext:1.0"],
        );
        let Some(ObjectOpReply::Ok(b)) = reply else {
            panic!("expected Ok");
        };
        assert!(!decode_bool(&b, Endianness::Big));
    }

    #[test]
    fn is_a_on_a_truncated_body_is_marshal() {
        let reply = dispatch(
            OP_IS_A,
            &[1, 2],
            Endianness::Big,
            "IDL:demo/Echo:1.0",
            &|_| false,
        );
        assert_eq!(reply, Some(ObjectOpReply::Marshal));
    }

    #[test]
    fn non_existent_and_the_legacy_spelling_are_false_for_a_resolved_object() {
        for op in [OP_NON_EXISTENT, OP_NOT_EXISTENT] {
            let reply = dispatch(op, &[], Endianness::Big, "IDL:demo/Echo:1.0", &|_| false);
            let Some(ObjectOpReply::Ok(b)) = reply else {
                panic!("expected Ok for {op}");
            };
            assert!(!decode_bool(&b, Endianness::Big));
        }
    }

    #[test]
    fn unknown_object_reply_is_true_for_non_existent_and_none_for_everything_else() {
        for op in [OP_NON_EXISTENT, OP_NOT_EXISTENT] {
            let b = unknown_object_reply(op, Endianness::Big).unwrap();
            assert!(decode_bool(&b, Endianness::Big));
        }
        for op in [
            OP_IS_A,
            OP_INTERFACE,
            OP_REPOSITORY_ID,
            OP_DOMAIN_MANAGERS,
            OP_COMPONENT,
            "frobnicate",
        ] {
            assert_eq!(unknown_object_reply(op, Endianness::Big), None);
        }
    }

    #[test]
    fn repository_id_returns_the_most_derived_id() {
        let reply = dispatch(
            OP_REPOSITORY_ID,
            &[],
            Endianness::Big,
            "IDL:demo/Echo:1.0",
            &|_| false,
        );
        let Some(ObjectOpReply::Ok(b)) = reply else {
            panic!("expected Ok");
        };
        let mut r = Reader::new(&b, Endianness::Big);
        assert_eq!(r.read_string().unwrap(), "IDL:demo/Echo:1.0");
    }

    #[test]
    fn interface_without_an_interface_repository_is_the_marker_reply() {
        let reply = dispatch(
            OP_INTERFACE,
            &[],
            Endianness::Big,
            "IDL:demo/Echo:1.0",
            &|_| false,
        );
        assert_eq!(reply, Some(ObjectOpReply::InterfaceRepositoryUnavailable));
    }

    #[test]
    fn domain_managers_is_an_empty_sequence() {
        let reply = dispatch(
            OP_DOMAIN_MANAGERS,
            &[],
            Endianness::Big,
            "IDL:demo/Echo:1.0",
            &|_| false,
        );
        let Some(ObjectOpReply::Ok(b)) = reply else {
            panic!("expected Ok");
        };
        let mut r = Reader::new(&b, Endianness::Big);
        assert_eq!(r.read_u32().unwrap(), 0);
    }

    #[test]
    fn component_is_a_nil_object_reference() {
        let reply = dispatch(
            OP_COMPONENT,
            &[],
            Endianness::Big,
            "IDL:demo/Echo:1.0",
            &|_| false,
        );
        let Some(ObjectOpReply::Ok(b)) = reply else {
            panic!("expected Ok");
        };
        let mut r = Reader::new(&b, Endianness::Big);
        assert_eq!(r.read_string().unwrap(), "");
        assert_eq!(r.read_u32().unwrap(), 0);
    }

    #[test]
    fn an_unrelated_operation_is_not_intercepted() {
        assert_eq!(
            dispatch("ping", &[], Endianness::Big, "IDL:demo/Echo:1.0", &|_| {
                false
            }),
            None
        );
    }
}
