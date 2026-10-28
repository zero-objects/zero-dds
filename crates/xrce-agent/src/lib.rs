// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE Agent state machine: client registry, object slots and the pull
//! queue of spec §7.3, without a DDS participant of its own.
//!
//! Crate `zerodds-xrce-agent`.
//!
//! # Spec mapping
//!
//! OMG DDS-XRCE 1.0 §7.3: "the XRCE Agent represents the XRCE client in the DDS data
//! space; client-pull model for disconnected devices."
//!
//! We provide an in-process agent state machine [`XrceAgent`] with:
//!
//! - `register_client(client_key)` — a client is registered by ClientKey;
//!   the agent creates one object slot per client.
//! - `create_object(...)` — spec §7.8.3 CREATE path.
//! - `delete_object(...)` — spec §7.8.3 DELETE path.
//! - `submit_sample(reader, payload)` — DDS-side push: places a
//!   sample into the pull queue of a DataReader.
//! - `pull_sample(client_key, reader)` — spec §7.3 client-pull model.
//!
//! The agent itself encapsulates no DDS stack — that is the job of the
//! integrating application. We only provide the object management +
//! pull queue.
//!
//! Safety classification: **STANDARD**.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;

use alloc::string::String;

use zerodds_xrce::header::{CLIENT_KEY_LEN, ClientKey};
use zerodds_xrce::object_id::ObjectId;
use zerodds_xrce::object_kind::ObjectKind;
use zerodds_xrce::object_repr::ObjectVariant;
use zerodds_xrce::object_store::{CreateOutcome, CreationMode, ObjectInstance, ObjectStore};

/// `zerodds-xrce-agentd`: the session engine, DCPS bridge, transports and
/// daemon wiring that turn this crate's state machine into a process XRCE
/// clients connect to. Gated behind `daemon` so the plain state machine
/// above stays embeddable (`alloc`-only, no DCPS stack).
#[cfg(feature = "daemon")]
pub mod daemon;

/// BTreeMap-capable key from ClientKey (ClientKey itself has no
/// `Ord` derive, because a hash container was envisaged in the original
/// crate; we map to the underlying `[u8; CLIENT_KEY_LEN]`).
type ClientKeyOrd = [u8; CLIENT_KEY_LEN];

fn ord_of(key: ClientKey) -> ClientKeyOrd {
    key.0
}

/// Agent-specific error classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentError {
    /// Client unknown — the caller must call `register_client` first.
    UnknownClient,
    /// The DataReader object does not exist.
    UnknownReader,
    /// The pull queue is full (DoS protection).
    QueueFull,
    /// The wire layer rejected the operation (e.g. ObjectId.kind
    /// does not match the operation).
    WireRejected,
}

/// Trace event for operation tracing (spec §8.5).
///
/// Generated per CREATE/DELETE/PULL operation and can be routed via
/// [`XrceAgent::set_trace_sink`] to an external logger
/// (`tracing` crate, structured logger, test sink).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEvent {
    /// Operation name ("CREATE", "DELETE", "SUBMIT", "PULL").
    pub operation: String,
    /// Client key (8 bytes).
    pub client_key: ClientKeyOrd,
    /// Object id (2 bytes wire form).
    pub object_id: [u8; 2],
}

/// Trace-sink trait. An implementation logs each event to an
/// external system (e.g. `tracing::info!`).
pub trait TraceSink {
    /// Logs a trace event.
    fn record(&mut self, event: TraceEvent);
}

/// In-memory XRCE agent.
pub struct XrceAgent {
    /// Object stores per client.
    clients: BTreeMap<ClientKeyOrd, ObjectStore>,
    /// Pull queue per (client, reader). Spec §7.3 pull model.
    samples: BTreeMap<(ClientKeyOrd, [u8; 2]), VecDeque<Vec<u8>>>,
    /// DoS cap: maximum buffered samples per reader.
    max_pending_samples: usize,
    /// Optional trace sink (spec §8.5 operation tracing).
    trace_sink: Option<alloc::boxed::Box<dyn TraceSink + Send>>,
}

fn oid_key(oid: ObjectId) -> [u8; 2] {
    oid.to_bytes()
}

impl XrceAgent {
    /// Constructor with default DoS caps (256 samples per reader).
    #[must_use]
    pub fn new() -> Self {
        Self::with_max_pending_samples(256)
    }

    /// Constructor with a custom DoS cap.
    #[must_use]
    pub fn with_max_pending_samples(max: usize) -> Self {
        Self {
            clients: BTreeMap::new(),
            samples: BTreeMap::new(),
            max_pending_samples: max,
            trace_sink: None,
        }
    }

    /// Spec §8.5 — registers an operation trace sink. Per
    /// CREATE/DELETE/SUBMIT/PULL operation a
    /// [`TraceEvent`] is created and delegated to the sink.
    pub fn set_trace_sink(&mut self, sink: alloc::boxed::Box<dyn TraceSink + Send>) {
        self.trace_sink = Some(sink);
    }

    fn trace(&mut self, op: &str, client_key: ClientKeyOrd, oid: [u8; 2]) {
        if let Some(sink) = self.trace_sink.as_mut() {
            sink.record(TraceEvent {
                operation: String::from(op),
                client_key,
                object_id: oid,
            });
        }
    }

    /// Registers a new client. Idempotent — registering multiple
    /// times with the same ClientKey does **not** overwrite the
    /// existing slot.
    pub fn register_client(&mut self, client_key: ClientKey) {
        self.clients.entry(ord_of(client_key)).or_default();
    }

    /// `true` if the client is registered.
    #[must_use]
    pub fn has_client(&self, client_key: ClientKey) -> bool {
        self.clients.contains_key(&ord_of(client_key))
    }

    /// Number of registered clients.
    #[must_use]
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Number of objects currently held in this client's object store
    /// (participant, topics, publishers, subscribers, writers, readers,
    /// types — spec §7.5). A daemon enforcing a `max_objects_per_client`
    /// config limit checks this before calling [`Self::create_object`],
    /// so the CREATE it refuses gets `STATUS_ERR_RESOURCES` back instead
    /// of silently failing inside the store.
    ///
    /// # Errors
    /// `UnknownClient` if `client_key` was never registered.
    pub fn object_count(&self, client_key: ClientKey) -> Result<usize, AgentError> {
        self.clients
            .get(&ord_of(client_key))
            .map(ObjectStore::len)
            .ok_or(AgentError::UnknownClient)
    }

    /// Removes a client entirely: its object store and every pending
    /// sample queued for it. Spec §8.4.9 (session DELETE) and the
    /// session-timeout path both end here — without it a timed-out
    /// session would leave its slot counted against `client_count()`
    /// forever, so a fleet that only ever reconnects would eventually
    /// be refused by a `max_clients` cap for sessions that are long gone.
    ///
    /// Returns `true` if the client was registered (and is now gone).
    pub fn unregister_client(&mut self, client_key: ClientKey) -> bool {
        let ord = ord_of(client_key);
        let existed = self.clients.remove(&ord).is_some();
        if existed {
            self.samples.retain(|(k, _), _| *k != ord);
            self.trace("UNREGISTER", ord, [0xFF, 0xFF]);
        }
        existed
    }

    /// Direct read access to one of a client's stored objects
    /// (participant, topic, writer, reader, type, ...). The daemon's
    /// `GET_INFO` handler uses this to answer with the object's stored
    /// `ObjectVariant`; `None` if the client or the object is unknown.
    #[must_use]
    pub fn get_object(
        &self,
        client_key: ClientKey,
        object_id: ObjectId,
    ) -> Option<&ObjectInstance> {
        self.clients.get(&ord_of(client_key))?.get(object_id)
    }

    /// The client's sole object of `kind`, or `None` if it has zero or
    /// more than one. The daemon's CREATE dispatch uses this to resolve
    /// an endpoint's/topic's implicit parent (a bare `CREATE(topic)`
    /// carries no explicit `participant_id` on the wire — spec §9.3
    /// Tab.15 nests it structurally instead): with exactly one
    /// participant already created, that is unambiguously the parent; with
    /// zero or several, the daemon reports `STATUS_ERR_UNKNOWN_REFERENCE`
    /// rather than guessing which one.
    #[must_use]
    pub fn only_object_of_kind(&self, client_key: ClientKey, kind: ObjectKind) -> Option<ObjectId> {
        let store = self.clients.get(&ord_of(client_key))?;
        let mut iter = store.iter_by_kind(kind);
        let first = iter.next()?.0;
        if iter.next().is_some() {
            None
        } else {
            Some(first)
        }
    }

    /// Spec §7.8.3 CREATE — registers an object in the client's slot.
    ///
    /// # Errors
    /// `UnknownClient`.
    pub fn create_object(
        &mut self,
        client_key: ClientKey,
        object_id: ObjectId,
        representation: ObjectVariant,
        mode: CreationMode,
    ) -> Result<CreateOutcome, AgentError> {
        let ord = ord_of(client_key);
        let store = self
            .clients
            .get_mut(&ord)
            .ok_or(AgentError::UnknownClient)?;
        let kind = object_id.kind().map_err(|_| AgentError::WireRejected)?;
        let outcome = store
            .create(object_id, kind, representation, mode)
            .map_err(|_| AgentError::WireRejected)?;
        self.trace("CREATE", ord, oid_key(object_id));
        Ok(outcome)
    }

    /// Spec §7.8.3 DELETE — removes an object + its associated
    /// pull queue.
    ///
    /// # Errors
    /// `UnknownClient`.
    pub fn delete_object(
        &mut self,
        client_key: ClientKey,
        object_id: ObjectId,
    ) -> Result<bool, AgentError> {
        let ord = ord_of(client_key);
        let store = self
            .clients
            .get_mut(&ord)
            .ok_or(AgentError::UnknownClient)?;
        let removed = store.delete(object_id);
        // Clean up the pull queue too.
        self.samples.remove(&(ord, oid_key(object_id)));
        self.trace("DELETE", ord, oid_key(object_id));
        Ok(removed)
    }

    /// DDS-side push: a new sample is added to a reader queue.
    /// The client fetches it later via `pull_sample`.
    ///
    /// # Errors
    /// `UnknownClient`, `UnknownReader`, `QueueFull`.
    pub fn submit_sample(
        &mut self,
        client_key: ClientKey,
        reader_id: ObjectId,
        payload: Vec<u8>,
    ) -> Result<(), AgentError> {
        let ord = ord_of(client_key);
        let store = self.clients.get(&ord).ok_or(AgentError::UnknownClient)?;
        if store.get(reader_id).is_none() {
            return Err(AgentError::UnknownReader);
        }
        let queue = self.samples.entry((ord, oid_key(reader_id))).or_default();
        if queue.len() >= self.max_pending_samples {
            return Err(AgentError::QueueFull);
        }
        queue.push_back(payload);
        self.trace("SUBMIT", ord, oid_key(reader_id));
        Ok(())
    }

    /// Spec §7.3 client-pull model: delivers the oldest buffered
    /// sample for the (client, reader). Returns `Ok(None)` if
    /// no data is pending.
    ///
    /// # Errors
    /// `UnknownClient`.
    pub fn pull_sample(
        &mut self,
        client_key: ClientKey,
        reader_id: ObjectId,
    ) -> Result<Option<Vec<u8>>, AgentError> {
        let ord = ord_of(client_key);
        if !self.clients.contains_key(&ord) {
            return Err(AgentError::UnknownClient);
        }
        let sample = self
            .samples
            .get_mut(&(ord, oid_key(reader_id)))
            .and_then(VecDeque::pop_front);
        if sample.is_some() {
            self.trace("PULL", ord, oid_key(reader_id));
        }
        Ok(sample)
    }

    /// Number of buffered samples for (client, reader).
    #[must_use]
    pub fn pending_samples(&self, client_key: ClientKey, reader_id: ObjectId) -> usize {
        self.samples
            .get(&(ord_of(client_key), oid_key(reader_id)))
            .map_or(0, VecDeque::len)
    }
}

impl Default for XrceAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_xrce::header::CLIENT_KEY_LEN;
    use zerodds_xrce::object_kind::{OBJK_DATAREADER, ObjectKind};

    fn key(b: u8) -> ClientKey {
        ClientKey([b; CLIENT_KEY_LEN])
    }

    fn reader_id(raw: u16) -> ObjectId {
        ObjectId::new(raw, ObjectKind::from_u8(OBJK_DATAREADER).unwrap()).unwrap()
    }

    #[test]
    fn agent_starts_empty() {
        let a = XrceAgent::new();
        assert_eq!(a.client_count(), 0);
    }

    #[test]
    fn register_client_idempotent() {
        let mut a = XrceAgent::new();
        let k = key(0x01);
        a.register_client(k);
        a.register_client(k);
        assert!(a.has_client(k));
        assert_eq!(a.client_count(), 1);
    }

    #[test]
    fn create_object_for_unknown_client_rejected() {
        let mut a = XrceAgent::new();
        let oid = reader_id(0x010);
        let err = a
            .create_object(
                key(0x99),
                oid,
                ObjectVariant::ByReference("r".into()),
                CreationMode::default(),
            )
            .expect_err("unknown client");
        assert_eq!(err, AgentError::UnknownClient);
    }

    #[test]
    fn pull_sample_unknown_client_rejected() {
        let mut a = XrceAgent::new();
        let oid = reader_id(0x010);
        let err = a.pull_sample(key(0x99), oid).expect_err("unknown");
        assert_eq!(err, AgentError::UnknownClient);
    }

    #[test]
    fn client_pull_empty_returns_none() {
        let mut a = XrceAgent::new();
        let k = key(0x01);
        a.register_client(k);
        let oid = reader_id(0x010);
        let s = a.pull_sample(k, oid).expect("ok");
        assert!(s.is_none());
    }

    #[test]
    fn after_submit_pull_returns_sample_in_fifo_order() {
        let mut a = XrceAgent::new();
        let k = key(0x01);
        a.register_client(k);
        let reader = reader_id(0x010);
        a.create_object(
            k,
            reader,
            ObjectVariant::ByReference("R".into()),
            CreationMode::default(),
        )
        .expect("create");
        a.submit_sample(k, reader, alloc::vec![1, 2, 3])
            .expect("s1");
        a.submit_sample(k, reader, alloc::vec![4, 5, 6])
            .expect("s2");
        assert_eq!(a.pending_samples(k, reader), 2);
        let p1 = a.pull_sample(k, reader).expect("ok1").expect("some1");
        assert_eq!(p1, alloc::vec![1, 2, 3]);
        let p2 = a.pull_sample(k, reader).expect("ok2").expect("some2");
        assert_eq!(p2, alloc::vec![4, 5, 6]);
        assert!(a.pull_sample(k, reader).expect("ok3").is_none());
    }

    #[test]
    fn submit_to_unknown_reader_rejected() {
        let mut a = XrceAgent::new();
        let k = key(0x01);
        a.register_client(k);
        let reader = reader_id(0x010);
        let err = a
            .submit_sample(k, reader, alloc::vec![0])
            .expect_err("unknown reader");
        assert_eq!(err, AgentError::UnknownReader);
    }

    #[test]
    fn dos_cap_max_pending_samples_enforced() {
        let mut a = XrceAgent::with_max_pending_samples(2);
        let k = key(0x01);
        a.register_client(k);
        let reader = reader_id(0x010);
        a.create_object(
            k,
            reader,
            ObjectVariant::ByReference("R".into()),
            CreationMode::default(),
        )
        .expect("create");
        a.submit_sample(k, reader, alloc::vec![1]).expect("s1");
        a.submit_sample(k, reader, alloc::vec![2]).expect("s2");
        let err = a
            .submit_sample(k, reader, alloc::vec![3])
            .expect_err("full");
        assert_eq!(err, AgentError::QueueFull);
    }

    #[test]
    fn delete_object_removes_pull_queue() {
        let mut a = XrceAgent::new();
        let k = key(0x01);
        a.register_client(k);
        let reader = reader_id(0x010);
        a.create_object(
            k,
            reader,
            ObjectVariant::ByReference("R".into()),
            CreationMode::default(),
        )
        .expect("create");
        a.submit_sample(k, reader, alloc::vec![1]).expect("s1");
        assert_eq!(a.pending_samples(k, reader), 1);
        let removed = a.delete_object(k, reader).expect("delete");
        assert!(removed);
        assert_eq!(a.pending_samples(k, reader), 0);
    }

    /// Spec §8.5 — Operation-Tracing.
    #[test]
    fn trace_sink_captures_create_delete_submit_pull() {
        use alloc::sync::Arc;
        use std::sync::Mutex;

        struct CaptureSink(Arc<Mutex<Vec<TraceEvent>>>);
        impl TraceSink for CaptureSink {
            fn record(&mut self, event: TraceEvent) {
                self.0.lock().unwrap().push(event);
            }
        }
        let log: Arc<Mutex<Vec<TraceEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let mut a = XrceAgent::new();
        a.set_trace_sink(alloc::boxed::Box::new(CaptureSink(Arc::clone(&log))));
        let k = key(0x01);
        a.register_client(k);
        let reader = reader_id(0x010);
        a.create_object(
            k,
            reader,
            ObjectVariant::ByReference("R".into()),
            CreationMode::default(),
        )
        .expect("create");
        a.submit_sample(k, reader, alloc::vec![1]).expect("submit");
        a.pull_sample(k, reader).expect("pull");
        a.delete_object(k, reader).expect("delete");

        let events = log.lock().unwrap();
        let ops: Vec<&str> = events.iter().map(|e| e.operation.as_str()).collect();
        assert_eq!(ops, vec!["CREATE", "SUBMIT", "PULL", "DELETE"]);
        for e in events.iter() {
            assert_eq!(e.client_key, k.0);
            assert_eq!(e.object_id, reader.to_bytes());
        }
    }

    /// Spec §8.4.6 — CREATE(OBJK_APPLICATION) as a top-level container.
    #[test]
    fn create_application_object_via_objk_application() {
        use zerodds_xrce::object_kind::{OBJK_APPLICATION, ObjectKind};
        let mut a = XrceAgent::new();
        let k = key(0x01);
        a.register_client(k);
        let app_oid = ObjectId::new(0x100, ObjectKind::from_u8(OBJK_APPLICATION).unwrap()).unwrap();
        let outcome = a
            .create_object(
                k,
                app_oid,
                ObjectVariant::ByXmlString("<application name=\"App1\"/>".into()),
                CreationMode::default(),
            )
            .expect("create app");
        assert_eq!(outcome, CreateOutcome::Created);
    }

    /// Spec §8.4.1 — Logical Actions Performance.
    /// Doc test that measures the average operation latency.
    /// 1000 CREATE+DELETE operations in < 100 ms (spec performance
    /// floor; on standard hardware the value is well below that).
    #[test]
    fn agent_create_delete_latency_under_spec_floor() {
        use std::time::Instant;
        use zerodds_xrce::object_kind::{OBJK_PARTICIPANT, ObjectKind};

        let mut a = XrceAgent::new();
        let k = key(0x01);
        a.register_client(k);
        let kind = ObjectKind::from_u8(OBJK_PARTICIPANT).unwrap();
        let start = Instant::now();
        for i in 0..1000u16 {
            let oid = ObjectId::new(i, kind).unwrap();
            a.create_object(
                k,
                oid,
                ObjectVariant::ByReference("p".into()),
                CreationMode::default(),
            )
            .expect("create");
            a.delete_object(k, oid).expect("delete");
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 100,
            "1000 CREATE+DELETE-Ops took {} ms (Spec-Floor 100ms)",
            elapsed.as_millis()
        );
    }

    #[test]
    fn object_count_tracks_creates_and_deletes() {
        let mut a = XrceAgent::new();
        let k = key(0x01);
        a.register_client(k);
        assert_eq!(a.object_count(k).expect("known client"), 0);
        let reader = reader_id(0x010);
        a.create_object(
            k,
            reader,
            ObjectVariant::ByReference("R".into()),
            CreationMode::default(),
        )
        .expect("create");
        assert_eq!(a.object_count(k).expect("known client"), 1);
        a.delete_object(k, reader).expect("delete");
        assert_eq!(a.object_count(k).expect("known client"), 0);
    }

    #[test]
    fn object_count_unknown_client_rejected() {
        let a = XrceAgent::new();
        assert_eq!(a.object_count(key(0x99)), Err(AgentError::UnknownClient));
    }

    #[test]
    fn unregister_client_drops_objects_and_samples() {
        let mut a = XrceAgent::new();
        let k = key(0x01);
        a.register_client(k);
        let reader = reader_id(0x010);
        a.create_object(
            k,
            reader,
            ObjectVariant::ByReference("R".into()),
            CreationMode::default(),
        )
        .expect("create");
        a.submit_sample(k, reader, alloc::vec![1]).expect("submit");
        assert_eq!(a.client_count(), 1);

        assert!(a.unregister_client(k));
        assert_eq!(a.client_count(), 0);
        assert!(!a.has_client(k));
        // A re-registered client under the same key starts from zero —
        // no leftover sample from the deleted session leaks into it.
        a.register_client(k);
        assert_eq!(a.pending_samples(k, reader), 0);
    }

    #[test]
    fn unregister_unknown_client_returns_false() {
        let mut a = XrceAgent::new();
        assert!(!a.unregister_client(key(0x99)));
    }

    #[test]
    fn multiple_clients_isolated() {
        let mut a = XrceAgent::new();
        let k1 = key(0x01);
        let k2 = key(0x02);
        a.register_client(k1);
        a.register_client(k2);
        let reader = reader_id(0x010);
        a.create_object(
            k1,
            reader,
            ObjectVariant::ByReference("R".into()),
            CreationMode::default(),
        )
        .expect("c1");
        a.create_object(
            k2,
            reader,
            ObjectVariant::ByReference("R".into()),
            CreationMode::default(),
        )
        .expect("c2");
        a.submit_sample(k1, reader, alloc::vec![100]).expect("s1");
        // Client 2 does NOT see client 1's sample.
        assert_eq!(a.pending_samples(k2, reader), 0);
        assert_eq!(a.pending_samples(k1, reader), 1);
    }

    #[test]
    fn get_object_returns_stored_variant() {
        let mut a = XrceAgent::new();
        let k = key(0x01);
        a.register_client(k);
        let topic = ObjectId::new(0x010, ObjectKind::Topic).unwrap();
        a.create_object(
            k,
            topic,
            ObjectVariant::ByReference("T".into()),
            CreationMode::default(),
        )
        .expect("create");
        let got = a.get_object(k, topic).expect("present");
        assert_eq!(got.variant, ObjectVariant::ByReference("T".into()));
        assert!(
            a.get_object(k, ObjectId::new(0x020, ObjectKind::Topic).unwrap())
                .is_none()
        );
        assert!(a.get_object(key(0x99), topic).is_none());
    }

    #[test]
    fn only_object_of_kind_none_when_zero_or_many() {
        let mut a = XrceAgent::new();
        let k = key(0x01);
        a.register_client(k);
        assert!(a.only_object_of_kind(k, ObjectKind::Participant).is_none());
        let p1 = ObjectId::new(0x010, ObjectKind::Participant).unwrap();
        a.create_object(
            k,
            p1,
            ObjectVariant::ByReference("P1".into()),
            CreationMode::default(),
        )
        .expect("create p1");
        assert_eq!(a.only_object_of_kind(k, ObjectKind::Participant), Some(p1));
        let p2 = ObjectId::new(0x020, ObjectKind::Participant).unwrap();
        a.create_object(
            k,
            p2,
            ObjectVariant::ByReference("P2".into()),
            CreationMode::default(),
        )
        .expect("create p2");
        assert!(a.only_object_of_kind(k, ObjectKind::Participant).is_none());
    }
}
