// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE Agent — DDS-Participant-Wrapper mit Pull-Modell (Spec §7.3).
//!
//! Crate `zerodds-xrce-agent`.
//!
//! # Spec-Mapping
//!
//! OMG DDS-XRCE 1.0 §7.3: "XRCE Agent stellt XRCE Client im DDS Data-
//! Space dar; Client-Pull-Modell fuer disconnected Devices."
//!
//! Wir liefern eine in-process Agent-State-Machine [`XrceAgent`] mit:
//!
//! - `register_client(client_key)` — Client wird per ClientKey
//!   registriert; Agent legt einen Object-Slot pro Client an.
//! - `create_object(...)` — Spec §7.8.3 CREATE-Pfad.
//! - `delete_object(...)` — Spec §7.8.3 DELETE-Pfad.
//! - `submit_sample(reader, payload)` — DDS-Side-Push: legt ein
//!   Sample in die Pull-Queue eines DataReaders.
//! - `pull_sample(client_key, reader)` — Spec §7.3 Client-Pull-Modell.
//!
//! Der Agent selbst kapselt keinen DDS-Stack — das ist Aufgabe der
//! integrierenden Anwendung. Wir liefern nur die Object-Verwaltung +
//! Pull-Queue.
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
use zerodds_xrce::object_repr::ObjectVariant;
use zerodds_xrce::object_store::{CreateOutcome, CreationMode, ObjectStore};

/// BTreeMap-faehiger Key aus ClientKey (ClientKey selbst hat kein
/// `Ord`-Derive, weil ein Hash-Container im Original-Crate vorgesehen
/// war; wir mappen auf das underlying `[u8; CLIENT_KEY_LEN]`).
type ClientKeyOrd = [u8; CLIENT_KEY_LEN];

fn ord_of(key: ClientKey) -> ClientKeyOrd {
    key.0
}

/// Agent-spezifische Error-Klassen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentError {
    /// Client unbekannt — Caller muss vorher `register_client` rufen.
    UnknownClient,
    /// DataReader-Object existiert nicht.
    UnknownReader,
    /// Pull-Queue ist voll (DoS-Schutz).
    QueueFull,
    /// Wire-Layer hat die Operation abgelehnt (z.B. ObjectId.kind
    /// passt nicht zur Operation).
    WireRejected,
}

/// Trace-Event fuer Operation-Tracing (Spec §8.5).
///
/// Wird pro CREATE/DELETE/PULL-Operation generiert und kann via
/// [`XrceAgent::set_trace_sink`] an einen externen Logger geleitet
/// werden (`tracing`-Crate, structured logger, Test-Sink).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEvent {
    /// Operation-Name ("CREATE", "DELETE", "SUBMIT", "PULL").
    pub operation: String,
    /// Client-Key (8 bytes).
    pub client_key: ClientKeyOrd,
    /// Object-Id (2 bytes wire form).
    pub object_id: [u8; 2],
}

/// Trace-Sink-Trait. Eine Implementation logged jedes Event in ein
/// externes System (e.g. `tracing::info!`).
pub trait TraceSink {
    /// Loggt ein Trace-Event.
    fn record(&mut self, event: TraceEvent);
}

/// In-Memory XRCE-Agent.
pub struct XrceAgent {
    /// Object-Stores pro Client.
    clients: BTreeMap<ClientKeyOrd, ObjectStore>,
    /// Pull-Queue pro (Client, Reader). Spec §7.3 Pull-Modell.
    samples: BTreeMap<(ClientKeyOrd, [u8; 2]), VecDeque<Vec<u8>>>,
    /// DoS-Cap: maximal gepufferte Samples pro Reader.
    max_pending_samples: usize,
    /// Optionaler Trace-Sink (Spec §8.5 Operation-Tracing).
    trace_sink: Option<alloc::boxed::Box<dyn TraceSink + Send>>,
}

fn oid_key(oid: ObjectId) -> [u8; 2] {
    oid.to_bytes()
}

impl XrceAgent {
    /// Konstruktor mit Default-DoS-Caps (256 Samples pro Reader).
    #[must_use]
    pub fn new() -> Self {
        Self::with_max_pending_samples(256)
    }

    /// Konstruktor mit Custom-DoS-Cap.
    #[must_use]
    pub fn with_max_pending_samples(max: usize) -> Self {
        Self {
            clients: BTreeMap::new(),
            samples: BTreeMap::new(),
            max_pending_samples: max,
            trace_sink: None,
        }
    }

    /// Spec §8.5 — registriert einen Operation-Trace-Sink. Pro
    /// CREATE/DELETE/SUBMIT/PULL-Operation wird ein
    /// [`TraceEvent`] erzeugt und an den Sink delegiert.
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

    /// Registriert einen neuen Client. Idempotent — mehrfaches
    /// Registrieren mit derselben ClientKey ueberschreibt den
    /// existierenden Slot **nicht**.
    pub fn register_client(&mut self, client_key: ClientKey) {
        self.clients.entry(ord_of(client_key)).or_default();
    }

    /// `true` wenn Client registriert ist.
    #[must_use]
    pub fn has_client(&self, client_key: ClientKey) -> bool {
        self.clients.contains_key(&ord_of(client_key))
    }

    /// Anzahl registrierter Clients.
    #[must_use]
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Spec §7.8.3 CREATE — registriert ein Objekt im Slot des Clients.
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

    /// Spec §7.8.3 DELETE — entfernt ein Objekt + zugehoerige
    /// Pull-Queue.
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
        // Pull-Queue mit aufraeumen.
        self.samples.remove(&(ord, oid_key(object_id)));
        self.trace("DELETE", ord, oid_key(object_id));
        Ok(removed)
    }

    /// DDS-Side-Push: ein neues Sample wird einer Reader-Queue
    /// hinzugefuegt. Der Client holt es spaeter via [`pull_sample`].
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

    /// Spec §7.3 Client-Pull-Modell: liefert das aelteste gepufferte
    /// Sample fuer den (Client, Reader) ab. Liefert `Ok(None)` wenn
    /// keine Daten anliegen.
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

    /// Anzahl gepufferter Samples fuer (Client, Reader).
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

    /// Spec §8.4.6 — CREATE(OBJK_APPLICATION) als Top-Level-Container.
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
    /// Doc-Test der die durchschnittliche Operation-Latenz misst.
    /// 1000 CREATE+DELETE-Operations in < 100 ms (Spec-Performance-
    /// Floor; auf Standard-Hardware liegt der Wert weit darunter).
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
            "1000 CREATE+DELETE-Ops dauerten {} ms (Spec-Floor 100ms)",
            elapsed.as_millis()
        );
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
        // Client 2 sieht das Sample von Client 1 NICHT.
        assert_eq!(a.pending_samples(k2, reader), 0);
        assert_eq!(a.pending_samples(k1, reader), 1);
    }
}
