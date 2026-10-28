// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-WEB bridge to the DDS-DCPS public API — Spec §7.4.
//!
//! Implements §7.4 class operations (Application, Participant, Topic,
//! Publisher, Subscriber, DataWriter, DataReader, WaitSet) in full:
//! create/delete/list for every entity Spec §7.4 Tab 5 models, plus
//! write/read for samples and create/wait/delete/list for WaitSets
//! (§7.4.9's long-poll `WaitSet::get`).
//!
//! Spec source: OMG DDS-WEB 1.0 §7.4 (pp. 17-58) — all CRUD
//! operations of the web object model that is mapped to the DDS
//! public API. QosLibrary/QosProfile and Type CRUD are Spec §7.3.1
//! `Root` operations, not §7.4 entity operations — they are metadata a
//! concrete daemon can serve without a DCPS round-trip, and are
//! deliberately not part of this trait.
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
        /// MIME type (e.g. `application/dds-web+xml`).
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

    /// `POST /dds/rest1/applications/{app}/domain_participants` —
    /// Application::create_participant (Spec §7.4.3.1). `body` is the
    /// `participantObjectRepresentation`: optionally a QoS/QosProfile,
    /// and optionally contained `Publisher`s (each optionally with
    /// contained `DataWriter`s) and `Subscriber`s (each optionally with
    /// contained `DataReader`s) — every contained entity's own `<name>`
    /// is the identifier `update_participant` later matches it by.
    ///
    /// # Errors
    /// `NotFound` / `Conflict`. `BadRequest` if `body`'s QoS does not
    /// parse, or a contained `DataWriter`/`DataReader` names a topic
    /// this participant does not have.
    fn create_participant(
        &mut self,
        app_name: &str,
        domain_id: u32,
        body: &[u8],
    ) -> Result<BackendResult, BackendError>;

    /// `PUT /dds/rest1/applications/{app}/domain_participants/{n}` —
    /// Application::update_participant (Spec §7.4.3.2). `body` is the
    /// same `participantObjectRepresentation` a create takes: its QoS
    /// is applied (`DomainParticipant::set_qos`), and its contained
    /// `Publisher`/`Subscriber` list is reconciled against what this
    /// participant already holds — one already existing (by `<name>`)
    /// has its own QoS/contained-entity list reconciled the same way,
    /// one absent from `body` is deleted, one named that does not yet
    /// exist is created.
    ///
    /// # Errors
    /// `NotFound` if the participant does not exist. `BadRequest` if
    /// `body`'s QoS does not parse, or a contained `DataWriter`/
    /// `DataReader` names a topic this participant does not have.
    fn update_participant(
        &mut self,
        app_name: &str,
        participant: &str,
        body: &[u8],
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

    /// `GET /dds/rest1/applications/{app}/domain_participants`.
    ///
    /// # Errors
    /// `NotFound` / `Internal`.
    fn list_participants(&self, app_name: &str) -> Result<BackendResult, BackendError>;

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

    /// `PUT .../topics/{t}` — DomainParticipant::update_topic (Spec
    /// §7.4.4.4): "It changes the QoS of the DDS::Topic." `body` is the
    /// `topicObjectRepresentation`; only its `<qos>` (or `qos_profile`)
    /// is read.
    ///
    /// # Errors
    /// `NotFound` if the topic does not exist. `BadRequest` if `body`'s
    /// QoS does not parse.
    fn update_topic(
        &mut self,
        app_name: &str,
        participant: &str,
        topic: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError>;

    /// `DELETE .../topics/{t}`.
    ///
    /// # Errors
    /// `NotFound`.
    fn delete_topic(
        &mut self,
        app_name: &str,
        participant: &str,
        topic: &str,
    ) -> Result<BackendResult, BackendError>;

    /// `GET .../topics`.
    ///
    /// # Errors
    /// `NotFound` / `Internal`.
    fn list_topics(&self, app_name: &str, participant: &str)
    -> Result<BackendResult, BackendError>;

    // ---- Publisher / Subscriber (Spec §7.4.4 + §7.4.5) --------------

    /// `POST .../publishers` — Participant::create_publisher (Spec
    /// §7.4.4.7). `body` is the `publisherObjectRepresentation`:
    /// optionally a QoS/QosProfile, and optionally contained
    /// `DataWriter`s (each naming an already-existing `Topic`).
    ///
    /// # Errors
    /// `NotFound` / `Conflict`. `BadRequest` if `body`'s QoS does not
    /// parse, or a contained `DataWriter` names an unknown topic.
    fn create_publisher(
        &mut self,
        app_name: &str,
        participant: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError>;

    /// `PUT .../publishers/{p}` — Participant::update_publisher (Spec
    /// §7.4.4.8). `body` is the `publisherObjectRepresentation`: its
    /// QoS is applied, and its contained `DataWriter` list is
    /// reconciled against what this publisher already holds (by
    /// `<name>`) — added, re-QoS'd, or deleted.
    ///
    /// # Errors
    /// `NotFound` if the publisher does not exist. `BadRequest` if
    /// `body`'s QoS does not parse, or a contained `DataWriter` names
    /// an unknown topic.
    fn update_publisher(
        &mut self,
        app_name: &str,
        participant: &str,
        publisher: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError>;

    /// `DELETE .../publishers/{p}`.
    ///
    /// # Errors
    /// `NotFound`.
    fn delete_publisher(
        &mut self,
        app_name: &str,
        participant: &str,
        publisher: &str,
    ) -> Result<BackendResult, BackendError>;

    /// `GET .../publishers`.
    ///
    /// # Errors
    /// `NotFound` / `Internal`.
    fn list_publishers(
        &self,
        app_name: &str,
        participant: &str,
    ) -> Result<BackendResult, BackendError>;

    /// `POST .../subscribers` — Participant::create_subscriber (Spec
    /// §7.4.4.11). `body` is the `subscriberObjectRepresentation`:
    /// optionally a QoS/QosProfile, and optionally contained
    /// `DataReader`s (each naming an already-existing `Topic`).
    ///
    /// # Errors
    /// `NotFound` / `Conflict`. `BadRequest` if `body`'s QoS does not
    /// parse, or a contained `DataReader` names an unknown topic.
    fn create_subscriber(
        &mut self,
        app_name: &str,
        participant: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError>;

    /// `PUT .../subscribers/{s}` — Participant::update_subscriber (Spec
    /// §7.4.4.12). `body` is the `subscriberObjectRepresentation`: its
    /// QoS is applied, and its contained `DataReader` list is
    /// reconciled against what this subscriber already holds (by
    /// `<name>`) — added, re-QoS'd, or deleted — same rule as
    /// [`Self::update_publisher`].
    ///
    /// # Errors
    /// `NotFound` if the subscriber does not exist. `BadRequest` if
    /// `body`'s QoS does not parse, or a contained `DataReader` names
    /// an unknown topic.
    fn update_subscriber(
        &mut self,
        app_name: &str,
        participant: &str,
        subscriber: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError>;

    /// `DELETE .../subscribers/{s}`.
    ///
    /// # Errors
    /// `NotFound`.
    fn delete_subscriber(
        &mut self,
        app_name: &str,
        participant: &str,
        subscriber: &str,
    ) -> Result<BackendResult, BackendError>;

    /// `GET .../subscribers`.
    ///
    /// # Errors
    /// `NotFound` / `Internal`.
    fn list_subscribers(
        &self,
        app_name: &str,
        participant: &str,
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

    /// `PUT .../data_writers/{w}` — Publisher::update_datawriter (Spec
    /// §7.4.5.2): "This operation updates the QoS of an existing
    /// DDS::DataWriter." `body` is the `datawriterObjectRepresentation`;
    /// only its QoS is read.
    ///
    /// # Errors
    /// `NotFound` if the writer does not exist. `BadRequest` if
    /// `body`'s QoS does not parse or is incompatible
    /// (Changeable=NO post-enable — Spec §2.2.3).
    fn update_data_writer(
        &mut self,
        app_name: &str,
        participant: &str,
        publisher: &str,
        writer: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError>;

    /// `DELETE .../data_writers/{w}`.
    ///
    /// # Errors
    /// `NotFound`.
    fn delete_data_writer(
        &mut self,
        app_name: &str,
        participant: &str,
        publisher: &str,
        writer: &str,
    ) -> Result<BackendResult, BackendError>;

    /// `GET .../data_writers`.
    ///
    /// # Errors
    /// `NotFound` / `Internal`.
    fn list_data_writers(
        &self,
        app_name: &str,
        participant: &str,
        publisher: &str,
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

    /// `PUT .../data_readers/{r}` — Subscriber::update_datareader (Spec
    /// §7.4.6.2): "This operation updates the QoS of an existing
    /// DDS::DataReader." `body` is the `datareaderObjectRepresentation`;
    /// only its QoS is read.
    ///
    /// # Errors
    /// `NotFound` if the reader does not exist. `BadRequest` if
    /// `body`'s QoS does not parse or is incompatible
    /// (Changeable=NO post-enable — Spec §2.2.3).
    fn update_data_reader(
        &mut self,
        app_name: &str,
        participant: &str,
        subscriber: &str,
        reader: &str,
        body: &[u8],
    ) -> Result<BackendResult, BackendError>;

    /// `DELETE .../data_readers/{r}`.
    ///
    /// # Errors
    /// `NotFound`.
    fn delete_data_reader(
        &mut self,
        app_name: &str,
        participant: &str,
        subscriber: &str,
        reader: &str,
    ) -> Result<BackendResult, BackendError>;

    /// `GET .../data_readers`.
    ///
    /// # Errors
    /// `NotFound` / `Internal`.
    fn list_data_readers(
        &self,
        app_name: &str,
        participant: &str,
        subscriber: &str,
    ) -> Result<BackendResult, BackendError>;

    // ---- Sample IO (Spec §7.4.6.5 + §7.4.7.5) -----------------------

    /// `POST .../data_writers/{w}` — Spec §7.4.6.5 `DataWriter::write`.
    /// The body is the `write_sample_seq` object representation (Spec
    /// §8.3.4 Tab 6); the URI names the writer itself, not a separate
    /// `write_sample_seq` sub-resource (Spec §8.3.3 Tab 5's row for
    /// this operation has no such segment).
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

    /// `GET .../data_readers/{r}` — Spec §7.4.8 `DataReader::read`; the
    /// response body is the `read_sample_seq` object representation.
    /// `selector` is the raw `SampleSelector` BNF
    /// string (see [`crate::sample_selector`]), unparsed — a concrete
    /// backend applies it as a `QueryCondition` plus the metadata
    /// state masks.
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

    // ---- WaitSet (Spec §7.4.9) ---------------------------------------

    /// `POST /dds/rest1/applications/{app}/waitsets` —
    /// `Application::create_waitset`. `readers` names the
    /// `subscriber/data_reader` paths (as returned by
    /// [`BackendResult::Created`]) whose arriving data attaches a
    /// `ReadCondition` to the new WaitSet.
    ///
    /// # Errors
    /// `NotFound` if a named reader does not exist. `Conflict` if the
    /// name is already taken.
    fn create_waitset(
        &mut self,
        app_name: &str,
        waitset: &str,
        readers: &[String],
    ) -> Result<BackendResult, BackendError>;

    /// `PUT /dds/rest1/applications/{app}/waitsets/{w}` —
    /// `Application::update_waitset` (Spec §7.4.3.5): "It updates the
    /// conditions associated with the WebDDS::WaitSet." Unlike every
    /// other `update_*` here, this is not a QoS change at all — a
    /// `WaitSet` has none — `readers` fully replaces the condition set
    /// the same way [`Self::create_waitset`]'s did.
    ///
    /// # Errors
    /// `NotFound` if the waitset, or a named reader, does not exist.
    fn update_waitset(
        &mut self,
        app_name: &str,
        waitset: &str,
        readers: &[String],
    ) -> Result<BackendResult, BackendError>;

    /// `GET /dds/rest1/applications/{app}/waitsets/{w}` —
    /// `WaitSet::get`. Blocks (long-poll) up to `timeout` for a
    /// condition to trigger, then returns the reader paths that did.
    /// An empty [`BackendResult::List`] means the timeout elapsed with
    /// nothing triggered — not an error, per Spec §7.4.9.3.
    ///
    /// # Errors
    /// `NotFound` if the waitset does not exist.
    fn wait_waitset(
        &mut self,
        app_name: &str,
        waitset: &str,
        timeout: core::time::Duration,
    ) -> Result<BackendResult, BackendError>;

    /// `DELETE /dds/rest1/applications/{app}/waitsets/{w}`.
    ///
    /// # Errors
    /// `NotFound`.
    fn delete_waitset(
        &mut self,
        app_name: &str,
        waitset: &str,
    ) -> Result<BackendResult, BackendError>;

    /// `GET /dds/rest1/applications/{app}/waitsets`.
    ///
    /// # Errors
    /// `NotFound` / `Internal`.
    fn list_waitsets(&self, app_name: &str) -> Result<BackendResult, BackendError>;
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
        fn create_participant(
            &mut self,
            _: &str,
            _: u32,
            _: &[u8],
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Created("p".to_string()))
        }
        fn delete_participant(&mut self, _: &str, _: &str) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Deleted)
        }
        fn list_participants(&self, _: &str) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::List(Vec::new()))
        }
        fn update_participant(
            &mut self,
            _: &str,
            _: &str,
            _: &[u8],
        ) -> Result<BackendResult, BackendError> {
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
        fn delete_topic(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Deleted)
        }
        fn list_topics(&self, _: &str, _: &str) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::List(Vec::new()))
        }
        fn update_topic(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: &[u8],
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Deleted)
        }
        fn create_publisher(
            &mut self,
            _: &str,
            _: &str,
            _: &[u8],
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Created("pub".to_string()))
        }
        fn update_publisher(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: &[u8],
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Deleted)
        }
        fn delete_publisher(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Deleted)
        }
        fn list_publishers(&self, _: &str, _: &str) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::List(Vec::new()))
        }
        fn create_subscriber(
            &mut self,
            _: &str,
            _: &str,
            _: &[u8],
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Created("sub".to_string()))
        }
        fn update_subscriber(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: &[u8],
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Deleted)
        }
        fn delete_subscriber(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Deleted)
        }
        fn list_subscribers(&self, _: &str, _: &str) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::List(Vec::new()))
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
        fn update_data_writer(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
            _: &[u8],
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Deleted)
        }
        fn delete_data_writer(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Deleted)
        }
        fn list_data_writers(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::List(Vec::new()))
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
        fn update_data_reader(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
            _: &[u8],
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Deleted)
        }
        fn delete_data_reader(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Deleted)
        }
        fn list_data_readers(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::List(Vec::new()))
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
        fn create_waitset(
            &mut self,
            _: &str,
            _: &str,
            _: &[String],
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Created("ws".to_string()))
        }
        fn update_waitset(
            &mut self,
            _: &str,
            _: &str,
            _: &[String],
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Deleted)
        }
        fn wait_waitset(
            &mut self,
            _: &str,
            _: &str,
            _: core::time::Duration,
        ) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::List(Vec::new()))
        }
        fn delete_waitset(&mut self, _: &str, _: &str) -> Result<BackendResult, BackendError> {
            Ok(BackendResult::Deleted)
        }
        fn list_waitsets(&self, _: &str) -> Result<BackendResult, BackendError> {
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
