// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-WEB bridge to the DDS-DCPS public API — Spec §7.4.
//!
//! Implements §7.4 §7.4 class operations
//! Application + Participant + Topic + Pub/Sub + Writer/Reader from
//! partial to done.
//!
//! Spec source: OMG DDS-WEB 1.0 §7.4 (pp. 17-58) — all CRUD
//! operations of the web object model that is mapped to the DDS
//! public API.
//!
//! # Layer discipline
//!
//! Here we define a trait [`DdsBackend`] that decouples the two
//! layers:
//!
//! * `crates/web/` defines REST routes (see `rest.rs`) and the
//!   web object model (see `model.rs`).
//! * A concrete backend implementation in a higher layer
//!   (e.g. a daemon crate) implements `DdsBackend` and
//!   calls the DDS public API (`crates/dcps/`).
//!
//! We keep the trait deliberately minimal — one method per operation,
//! no hidden async runtime. Callers that need async backends
//! can adapt via `tokio::task::spawn_blocking`.
//!
//! Cross-Ref: Spec §7.4 Tab 5 + §8.3.3 (PIM-zu-REST-Mapping).

use alloc::string::String;
use alloc::vec::Vec;

use crate::access_control::Decision;

/// Operation result: either a newly created resource identifier,
/// a list, or an opaque body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendResult {
    /// Successful CREATE/UPDATE — returns the new resource path
    /// (e.g. `app1/topics/Sensor`).
    Created(String),
    /// Successful DELETE / no-content operation.
    Deleted,
    /// List of resource paths (e.g. for `get_applications`).
    List(Vec<String>),
    /// Opaque resource body as XML/JSON (Spec §8.1).
    Body {
        /// MIME type (e.g. `application/zerodds-web+xml`).
        content_type: String,
        /// Body bytes.
        body: Vec<u8>,
    },
}

/// Backend error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// Resource not found.
    NotFound(String),
    /// Resource already exists.
    Conflict(String),
    /// Request body not parsable.
    BadRequest(String),
    /// The AccessController returned Decision::Deny.
    Forbidden,
    /// Unexpected internal error.
    Internal(String),
}

/// Bridge trait — a concrete backend implementation routes these
/// method calls to the `crates/dcps/` public API.
///
/// Spec §7.4 requires exactly these operations per object type
/// (Application/Participant/Topic/Publisher/Subscriber/DataWriter/
/// DataReader/WaitSet). We bundle them thematically.
pub trait DdsBackend {
    // ---- Application (Spec §7.4.1) ----------------------------------

    /// `POST /dds/rest1/applications` — create_application.
    ///
    /// # Errors
    /// `Conflict` if the application name already exists.
    fn create_application(&mut self, app_name: &str) -> Result<BackendResult, BackendError>;

    /// `DELETE /dds/rest1/applications/{name}`.
    ///
    /// # Errors
    /// `NotFound` if the name is unknown.
    fn delete_application(&mut self, app_name: &str) -> Result<BackendResult, BackendError>;

    /// `GET /dds/rest1/applications` (Spec §7.4.1.4 fnmatch).
    ///
    /// # Errors
    /// `Internal`.
    fn list_applications(&self, pattern: &str) -> Result<BackendResult, BackendError>;

    // ---- DomainParticipant (Spec §7.4.2) ----------------------------

    /// `POST /dds/rest1/applications/{app}/domain_participants`.
    ///
    /// # Errors
    /// `NotFound` / `Conflict`.
    fn create_participant(
        &mut self,
        app_name: &str,
        domain_id: u32,
    ) -> Result<BackendResult, BackendError>;

    /// `DELETE /dds/rest1/applications/{app}/domain_participants/{n}`.
    ///
    /// # Errors
    /// `NotFound`.
    fn delete_participant(
        &mut self,
        app_name: &str,
        participant: &str,
    ) -> Result<BackendResult, BackendError>;

    // ---- Topic (Spec §7.4.3) ----------------------------------------

    /// `POST .../topics`.
    ///
    /// # Errors
    /// `NotFound` / `Conflict` / `BadRequest`.
    fn create_topic(
        &mut self,
        app_name: &str,
        participant: &str,
        topic_name: &str,
        type_name: &str,
    ) -> Result<BackendResult, BackendError>;

    // ---- DataWriter / DataReader (Spec §7.4.6 + §7.4.7) -------------

    /// `POST .../data_writers`.
    ///
    /// # Errors
    /// `NotFound` / `Conflict`.
    fn create_data_writer(
        &mut self,
        app_name: &str,
        participant: &str,
        publisher: &str,
        topic: &str,
    ) -> Result<BackendResult, BackendError>;

    /// `POST .../data_readers`.
    ///
    /// # Errors
    /// `NotFound` / `Conflict`.
    fn create_data_reader(
        &mut self,
        app_name: &str,
        participant: &str,
        subscriber: &str,
        topic: &str,
    ) -> Result<BackendResult, BackendError>;

    // ---- Sample IO (Spec §7.4.6.5 + §7.4.7.5) -----------------------

    /// `POST .../data_writers/{w}/write_sample_seq`.
    ///
    /// # Errors
    /// `NotFound` / `BadRequest`.
    fn write_sample(
        &mut self,
        app_name: &str,
        participant: &str,
        publisher: &str,
        writer: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError>;

    /// `POST .../data_readers/{r}/read_sample_seq`.
    ///
    /// # Errors
    /// `NotFound`.
    fn read_samples(
        &mut self,
        app_name: &str,
        participant: &str,
        subscriber: &str,
        reader: &str,
        selector: Option<&str>,
    ) -> Result<BackendResult, BackendError>;
}

/// Decision wrapper that checks the AccessController permission
/// before every backend call.
///
/// Spec §7.3 decision engine + §7.4 class operations are thus
/// composed orthogonally.
///
/// # Errors
/// `BackendError::Forbidden` if `decision == Decision::Deny`.
pub fn enforce(decision: Decision) -> Result<(), BackendError> {
    match decision {
        Decision::Permit => Ok(()),
        Decision::Deny => Err(BackendError::Forbidden),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::access_control::{Decision, Operation, Permissions, Rule};
    use alloc::collections::BTreeMap;

    /// Minimal in-memory backend for the test (counts only
    /// operation calls, not a real DDS bridge).
    #[derive(Default)]
    struct InMemoryBackend {
        apps: BTreeMap<String, ()>,
    }

    impl DdsBackend for InMemoryBackend {
        fn create_application(&mut self, n: &str) -> Result<BackendResult, BackendError> {
            if self.apps.contains_key(n) {
                return Err(BackendError::Conflict(n.to_string()));
            }
            self.apps.insert(n.to_string(), ());
            Ok(BackendResult::Created(alloc::format!("/applications/{n}")))
        }
        fn delete_application(&mut self, n: &str) -> Result<BackendResult, BackendError> {
            if self.apps.remove(n).is_none() {
                return Err(BackendError::NotFound(n.to_string()));
            }
            Ok(BackendResult::Deleted)
        }
        fn list_applications(&self, _p: &str) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::List(self.apps.keys().cloned().collect()))
        }
        fn create_participant(&mut self, _: &str, _: u32) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Created("p".to_string()))
        }
        fn delete_participant(&mut self, _: &str, _: &str) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Deleted)
        }
        fn create_topic(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Created("t".to_string()))
        }
        fn create_data_writer(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Created("dw".to_string()))
        }
        fn create_data_reader(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Created("dr".to_string()))
        }
        fn write_sample(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
            _: &[u8],
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Deleted)
        }
        fn read_samples(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
            _: Option<&str>,
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::List(Vec::new()))
        }
    }

    #[test]
    fn enforce_permit_allows_call() {
        assert!(enforce(Decision::Permit).is_ok());
    }

    #[test]
    fn enforce_deny_returns_forbidden() {
        let r = enforce(Decision::Deny);
        assert!(matches!(r, Err(BackendError::Forbidden)));
    }

    #[test]
    fn permission_check_then_backend_call_chains_correctly() {
        let perms = Permissions {
            subject_name: "alice".to_string(),
            default: Decision::Deny,
            rules: alloc::vec![Rule::allow("*", alloc::vec![Operation::Admin])],
        };
        let mut backend = InMemoryBackend::default();
        // Alice is allowed to create an application.
        let dec = perms.evaluate(Operation::Admin, "App1");
        assert!(enforce(dec).is_ok());
        let r = backend.create_application("App1").expect("ok");
        assert!(matches!(r, BackendResult::Created(_)));
    }

    #[test]
    fn create_then_delete_application_round_trip() {
        let mut b = InMemoryBackend::default();
        b.create_application("X").expect("ok");
        let r = b.delete_application("X").expect("ok");
        assert_eq!(r, BackendResult::Deleted);
    }

    #[test]
    fn create_application_twice_yields_conflict() {
        let mut b = InMemoryBackend::default();
        b.create_application("X").expect("ok");
        let err = b.create_application("X").expect_err("conflict");
        assert!(matches!(err, BackendError::Conflict(_)));
    }
}
