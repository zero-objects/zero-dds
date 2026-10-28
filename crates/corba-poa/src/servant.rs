// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Servant — Spec §11.3.3.
//!
//! A `Servant` is the implementation that processes requests for an
//! active object. The spec leaves the concrete language form open
//! (C++ trait, Java class, etc.) — we model it as a Rust trait
//! with a minimal spec surface (`type_id` + request dispatch).

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_corba_giop::object_ops::{self, OBJECT_REPOSITORY_ID, ObjectOpReply};
use zerodds_corba_ir::{IrResult, RepositoryId};
use zerodds_wire::Endianness;

/// Servant — Spec §11.3.3.
pub trait Servant: core::fmt::Debug + Send + Sync {
    /// Returns the repository ID of the primary interface type
    /// (Spec §11.3.5.20.4 `_primary_interface`).
    fn primary_interface(&self) -> String;

    /// Returns the structured [`RepositoryId`] of the primary interface.
    /// Default impl: parses `primary_interface()` per Spec §10.7.3.1.
    ///
    /// Implementers can override this if they hold the structured
    /// form directly and want to avoid the string roundtrip.
    ///
    /// # Errors
    /// `IrError::InvalidRepositoryId` if `primary_interface()` does not
    /// yield a valid `IDL:<scoped>:<m>.<n>` format.
    fn primary_repository_id(&self) -> IrResult<RepositoryId> {
        RepositoryId::parse(&self.primary_interface())
    }

    /// Returns the list of all repository IDs implemented by this
    /// servant (Spec §11.3.5.20.5 `_all_interfaces`).
    fn all_interfaces(&self) -> Vec<String> {
        alloc::vec![self.primary_interface()]
    }

    /// `true` if the servant implements the given repository ID
    /// (Spec §11.3.5.20.4 `_is_a`) — every object reference answers
    /// `true` for [`OBJECT_REPOSITORY_ID`] regardless of what
    /// [`Servant::all_interfaces`] lists (CORBA 3.3 Part 1 §4.3.1: it
    /// is the root of the object-reference type hierarchy).
    fn is_a(&self, repository_id: &str) -> bool {
        repository_id == OBJECT_REPOSITORY_ID
            || self.all_interfaces().iter().any(|i| i == repository_id)
    }

    /// Typed variant of [`Servant::is_a`] — compares against
    /// the canonical string form of the [`RepositoryId`].
    fn is_a_typed(&self, repository_id: &RepositoryId) -> bool {
        self.is_a(&repository_id.to_canonical())
    }

    /// Answers one of the six `CORBA::Object` operations CORBA 3.3
    /// Part 2 §15.4.2 lets a client send as a GIOP request without the
    /// target interface having declared it (`_interface`, `_is_a`,
    /// `_non_existent`/`_not_existent`, `_domain_managers`,
    /// `_component`, `_repository_id`) — Part 1 §8.3 gives their
    /// semantics on `CORBA::Object` itself.
    ///
    /// `None` when `operation` is not one of the six — the POA's own
    /// dispatch continues to [`Servant::invoke`] as before. The default
    /// implementation uses [`Servant::primary_interface`] as the
    /// most-derived repository id and [`Servant::is_a`] for the
    /// base-interface chain; a servant with a structured identity it
    /// would rather not restring can override it directly.
    fn dispatch_object_operation(
        &self,
        operation: &str,
        body: &[u8],
        endianness: Endianness,
    ) -> Option<ObjectOpReply> {
        object_ops::dispatch(
            operation,
            body,
            endianness,
            &self.primary_interface(),
            &|id| self.is_a(id),
        )
    }

    /// Processes a request — opaque body bytes in, reply
    /// body bytes out. The GIOP header layer is managed by the
    /// POA caller.
    ///
    /// # Errors
    /// Implementer-specific (user exception or system exception).
    fn invoke(&self, operation: &str, request_body: &[u8]) -> Vec<u8>;
}

/// Test helper servant for unit tests.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct EchoServant {
    pub repo_id: String,
}

#[cfg(test)]
impl Servant for EchoServant {
    fn primary_interface(&self) -> String {
        self.repo_id.clone()
    }

    fn invoke(&self, _operation: &str, request_body: &[u8]) -> Vec<u8> {
        request_body.to_vec()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn echo_servant_returns_body() {
        let s = EchoServant {
            repo_id: "IDL:demo/Echo:1.0".into(),
        };
        let out = s.invoke("ping", &[1, 2, 3]);
        assert_eq!(out, alloc::vec![1, 2, 3]);
    }

    #[test]
    fn is_a_checks_repository_ids() {
        let s = EchoServant {
            repo_id: "IDL:demo/Echo:1.0".into(),
        };
        assert!(s.is_a("IDL:demo/Echo:1.0"));
        assert!(!s.is_a("IDL:demo/Other:1.0"));
    }

    #[test]
    fn is_a_is_true_for_corba_object_regardless_of_the_servant() {
        let s = EchoServant {
            repo_id: "IDL:demo/Echo:1.0".into(),
        };
        assert!(s.is_a(OBJECT_REPOSITORY_ID));
    }

    #[test]
    fn dispatch_object_operation_answers_is_a_repository_id_and_non_existent() {
        let s = EchoServant {
            repo_id: "IDL:demo/Echo:1.0".into(),
        };
        let e = Endianness::Big;

        let body = {
            let mut w = zerodds_wire::Writer::new(e);
            w.write_string("IDL:demo/Echo:1.0");
            w.finish()
        };
        let Some(ObjectOpReply::Ok(reply)) = s.dispatch_object_operation("_is_a", &body, e) else {
            panic!("expected Ok");
        };
        let mut r = zerodds_wire::Reader::new(&reply, e);
        assert!(r.read_bool().unwrap());

        let Some(ObjectOpReply::Ok(reply)) = s.dispatch_object_operation("_repository_id", &[], e)
        else {
            panic!("expected Ok");
        };
        let mut r = zerodds_wire::Reader::new(&reply, e);
        assert_eq!(r.read_string().unwrap(), "IDL:demo/Echo:1.0");

        let Some(ObjectOpReply::Ok(reply)) = s.dispatch_object_operation("_non_existent", &[], e)
        else {
            panic!("expected Ok");
        };
        let mut r = zerodds_wire::Reader::new(&reply, e);
        assert!(!r.read_bool().unwrap());

        // Not one of the six — the servant's own `invoke` continues to
        // handle it.
        assert_eq!(s.dispatch_object_operation("ping", &[], e), None);
    }

    #[test]
    fn primary_repository_id_parses_to_typed_form() {
        let s = EchoServant {
            repo_id: "IDL:demo/Echo:1.0".into(),
        };
        let r = s.primary_repository_id().expect("parse");
        assert_eq!(r.scoped_name, "demo/Echo");
        assert_eq!(r.major, 1);
        assert_eq!(r.minor, 0);
        assert!(s.is_a_typed(&r));
    }

    #[test]
    fn primary_repository_id_invalid_form_returns_error() {
        #[derive(Debug)]
        struct Bad;
        impl Servant for Bad {
            fn primary_interface(&self) -> String {
                "not-a-repo-id".into()
            }
            fn invoke(&self, _: &str, _: &[u8]) -> Vec<u8> {
                Vec::new()
            }
        }
        assert!(Bad.primary_repository_id().is_err());
    }
}
