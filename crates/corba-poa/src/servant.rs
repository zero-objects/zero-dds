// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Servant — Spec §11.3.3.
//!
//! Ein `Servant` ist die Implementation, die einem aktiven Object
//! Requests verarbeitet. Spec laesst die konkrete Form der Sprache
//! (C++-Trait, Java-Class, etc.) — wir modellieren als Rust-Trait
//! mit minimaler Spec-Surface (`type_id` + Request-Dispatch).

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_corba_ir::{IrResult, RepositoryId};

/// Servant — Spec §11.3.3.
pub trait Servant: core::fmt::Debug + Send + Sync {
    /// Liefert die Repository-ID des wichtigsten Interface-Types
    /// (Spec §11.3.5.20.4 `_primary_interface`).
    fn primary_interface(&self) -> String;

    /// Liefert die strukturierte [`RepositoryId`] des Primary-Interface.
    /// Default-Impl: parst `primary_interface()` per Spec §10.7.3.1.
    ///
    /// Implementer koennen das ueberschreiben, wenn sie die strukturierte
    /// Form direkt vorhalten und den String-Roundtrip vermeiden moechten.
    ///
    /// # Errors
    /// `IrError::InvalidRepositoryId`, wenn `primary_interface()` kein
    /// gueltiges `IDL:<scoped>:<m>.<n>`-Format liefert.
    fn primary_repository_id(&self) -> IrResult<RepositoryId> {
        RepositoryId::parse(&self.primary_interface())
    }

    /// Liefert die Liste aller von diesem Servant implementierten
    /// Repository-IDs (Spec §11.3.5.20.5 `_all_interfaces`).
    fn all_interfaces(&self) -> Vec<String> {
        alloc::vec![self.primary_interface()]
    }

    /// `true` wenn der Servant das angegebene Repository-ID
    /// implementiert (Spec §11.3.5.20.4 `_is_a`).
    fn is_a(&self, repository_id: &str) -> bool {
        self.all_interfaces().iter().any(|i| i == repository_id)
    }

    /// Typisierte Variante von [`Servant::is_a`] — vergleicht gegen
    /// die kanonische String-Form des [`RepositoryId`].
    fn is_a_typed(&self, repository_id: &RepositoryId) -> bool {
        self.is_a(&repository_id.to_canonical())
    }

    /// Verarbeitet einen Request — opaque Body-Bytes rein, Reply-
    /// Body-Bytes raus. Die GIOP-Header-Schicht wird vom POA-
    /// Caller verwaltet.
    ///
    /// # Errors
    /// Implementer-spezifisch (User-Exception oder System-Exception).
    fn invoke(&self, operation: &str, request_body: &[u8]) -> Vec<u8>;
}

/// Test-Hilfs-Servant fuer Unit-Tests.
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
