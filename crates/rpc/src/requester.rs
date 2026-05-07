// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! [`Requester`] — Client-Seite eines DDS-RPC-Service (Spec §7.9).
//!
//! Threading-Modell der Foundation-Stufe (C6.1.C):
//!
//! * **Synchrones API**, kein async-Runtime-Zwang. `send_request_blocking`
//!   ruft intern [`Requester::tick`] in einem Polling-Loop bis Reply oder
//!   Timeout. Caller mit eigenem Event-Loop koennen `send_request_async`
//!   nutzen, das nur den Request schickt und einen `mpsc::Receiver`
//!   zurueckliefert; sie sind dann selbst dafuer zustaendig, regelmaessig
//!   `tick()` zu rufen.
//! * Korrelation: jeder Request bekommt eine eindeutige
//!   [`SampleIdentity`], die im [`RequestHeader`] auf die Wire geht. Der
//!   Replier setzt sie in `ReplyHeader::related_request_id`. `tick`
//!   liest neue Replies, sucht den passenden `mpsc::Sender` und routet die
//!   Antwort.
//!
//! Wire-Frame: siehe [`crate::wire_codec`].
//!
//! Spec-Korrelation: Die Spec verlangt zusaetzlich, dass der Reply-DATA
//! im Inline-QoS-Block den `PID_RELATED_SAMPLE_IDENTITY` traegt
//! ([`zerodds_rtps::inline_qos`]). Dieser Pfad wird in C6.1.D
//! aktiviert, wenn DCPS-DataWriter eine Inline-QoS-API exponiert. Bis
//! dahin reicht der Header-im-Payload-Pfad fuer Foundation-Tests.

extern crate alloc;

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::time::Duration;
use std::collections::HashMap;
use std::sync::{Mutex, mpsc};

use zerodds_dcps::dds_type::{DdsType, RawBytes};
use zerodds_dcps::participant::DomainParticipant;
use zerodds_dcps::publisher::DataWriter;
use zerodds_dcps::qos::{PublisherQos, SubscriberQos, TopicQos};
use zerodds_dcps::subscriber::DataReader;

use crate::common_types::{RemoteExceptionCode, RequestHeader, SampleIdentity};
use crate::error::{RpcError, RpcResult};
use crate::qos_profile::RpcQos;
use crate::topic_naming::ServiceTopicNames;
use crate::wire_codec::{decode_reply_frame, encode_request_frame};

// ---------------------------------------------------------------------
// Service-Instance-Registry — verhindert Duplikate auf einem Participant
// ---------------------------------------------------------------------

/// Rolle eines Endpoints in der Instance-Registry — Requester und Replier
/// fuer dasselbe `(service, instance)` koexistieren auf einem Participant
/// (Spec §7.6.2). Doppelt belegt ist nur dieselbe Rolle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum InstanceRole {
    Requester,
    Replier,
}

/// Schluessel: `(participant_pointer, role, service_name, instance_name)`.
type InstanceKey = (usize, InstanceRole, String, String);

fn instance_registry() -> &'static Mutex<std::collections::HashSet<InstanceKey>> {
    use std::sync::OnceLock;
    static REGISTRY: OnceLock<Mutex<std::collections::HashSet<InstanceKey>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

fn participant_addr(p: &DomainParticipant) -> usize {
    // Adresse des `DomainParticipant`-Wrappers reicht als Process-lokales
    // Token. `Arc`-Inhalt bleibt waehrend der Lebenszeit stabil.
    core::ptr::from_ref(p) as usize
}

pub(crate) fn try_claim_instance(
    p: &DomainParticipant,
    role: InstanceRole,
    service_name: &str,
    instance_name: &str,
) -> RpcResult<InstanceClaim> {
    if instance_name.is_empty() {
        // Anonyme Endpoints erlauben Mehrfach-Registrierung — Spec §7.6.2
        // unterscheidet Default-Instance nicht vom Single-Endpoint.
        return Ok(InstanceClaim::anonymous());
    }
    let key: InstanceKey = (
        participant_addr(p),
        role,
        service_name.into(),
        instance_name.into(),
    );
    let mut reg = instance_registry()
        .lock()
        .map_err(|_| RpcError::Dcps("instance-registry poisoned".into()))?;
    if !reg.insert(key.clone()) {
        return Err(RpcError::DuplicateInstanceName(instance_name.into()));
    }
    Ok(InstanceClaim::owned(key))
}

/// RAII-Slot, der einen `(participant, service, instance)`-Eintrag bei
/// Drop wieder freigibt. So bleibt die Registry sauber, wenn ein
/// Requester/Replier gedropt wird.
#[derive(Debug)]
pub(crate) struct InstanceClaim {
    key: Option<InstanceKey>,
}

impl InstanceClaim {
    fn anonymous() -> Self {
        Self { key: None }
    }
    fn owned(key: InstanceKey) -> Self {
        Self { key: Some(key) }
    }
}

impl Drop for InstanceClaim {
    fn drop(&mut self) {
        if let Some(key) = self.key.take() {
            if let Ok(mut reg) = instance_registry().lock() {
                reg.remove(&key);
            }
        }
    }
}

// ---------------------------------------------------------------------
// Requester
// ---------------------------------------------------------------------

/// Reply-Payload an den wartenden Caller. `Ok(bytes)` ⇒
/// `RemoteExceptionCode::Ok` mit user-payload-bytes; `Err(code)` ⇒
/// Server-Side-Exception ohne Payload.
pub type ReplyOutcome = Result<Vec<u8>, RemoteExceptionCode>;

/// Pending-Slot pro outstanding Request.
struct PendingSlot {
    sender: mpsc::Sender<ReplyOutcome>,
}

/// Client-Seite eines DDS-RPC-Service.
///
/// `TIn` ist der **User-Request-Payload-Typ** (z.B. `Calculator_AddRequest`),
/// `TOut` der **User-Reply-Payload-Typ**. Beide muessen `DdsType`
/// implementieren — encodet/decodet wird ueber [`DdsType::encode`] und
/// [`DdsType::decode`].
pub struct Requester<TIn: DdsType, TOut: DdsType> {
    service_name: String,
    instance_name: String,
    request_writer: DataWriter<RawBytes>,
    reply_reader: DataReader<RawBytes>,
    /// 16-byte Writer-GUID, der jeden Request markiert. Der Wert wird
    /// vom DataWriter-Layer aus dem DCPS-Runtime injiziert (RTPS-GUID
    /// des Request-Writers); `Requester::new` synthesisiert nur einen
    /// Initial-Wert fuer Tests, der vor dem ersten Send durch
    /// `set_writer_guid` ersetzt wird.
    writer_guid: [u8; 16],
    next_seq: Mutex<u64>,
    pending: Arc<Mutex<HashMap<SampleIdentity, PendingSlot>>>,
    qos: RpcQos,
    _claim: InstanceClaim,
    _phantom: PhantomData<fn() -> (TIn, TOut)>,
}

impl<TIn: DdsType, TOut: DdsType> core::fmt::Debug for Requester<TIn, TOut> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Requester")
            .field("service", &self.service_name)
            .field("instance", &self.instance_name)
            .finish_non_exhaustive()
    }
}

impl<TIn: DdsType + Send + 'static, TOut: DdsType + Send + 'static> Requester<TIn, TOut> {
    /// Legt einen neuen Requester gegen `service_name` an.
    ///
    /// Erzeugt zwei Topics — `<service>_Request` (Writer) und
    /// `<service>_Reply` (Reader) — und einen `Publisher`/`Subscriber`-
    /// Paerchen. `instance_name=""` heisst "Default-Instance, kein
    /// `PID_SERVICE_INSTANCE_NAME`".
    ///
    /// # Errors
    /// * `RpcError::InvalidServiceName` falls der Name leer/illegal ist.
    /// * `RpcError::Dcps` bei Topic/Writer/Reader-Anlegen-Fehlern.
    /// * `RpcError::DuplicateInstanceName` falls auf demselben
    ///   Participant bereits ein Requester/Replier mit gleichem
    ///   `(service, instance)`-Paar laeuft.
    pub fn new(
        participant: &DomainParticipant,
        service_name: &str,
        qos: &RpcQos,
    ) -> RpcResult<Self> {
        Self::with_instance(participant, service_name, "", qos)
    }

    /// Wie [`Self::new`], aber mit explizitem `service_instance_name`
    /// (Spec §7.8.2 PID 0x0080).
    ///
    /// # Errors
    /// Siehe [`Self::new`].
    pub fn with_instance(
        participant: &DomainParticipant,
        service_name: &str,
        instance_name: &str,
        qos: &RpcQos,
    ) -> RpcResult<Self> {
        let topics = ServiceTopicNames::new(service_name)?;
        let claim = try_claim_instance(
            participant,
            InstanceRole::Requester,
            service_name,
            instance_name,
        )?;
        let request_topic = participant
            .create_topic::<RawBytes>(&topics.request, TopicQos::default())
            .map_err(|e| RpcError::Dcps(alloc::format!("create_topic request: {e:?}")))?;
        let reply_topic = participant
            .create_topic::<RawBytes>(&topics.reply, TopicQos::default())
            .map_err(|e| RpcError::Dcps(alloc::format!("create_topic reply: {e:?}")))?;
        let publisher = participant.create_publisher(PublisherQos::default());
        let subscriber = participant.create_subscriber(SubscriberQos::default());
        let request_writer = publisher
            .create_datawriter::<RawBytes>(&request_topic, qos.request_writer_qos())
            .map_err(|e| RpcError::Dcps(alloc::format!("create_datawriter: {e:?}")))?;
        let reply_reader = subscriber
            .create_datareader::<RawBytes>(&reply_topic, qos.reply_reader_qos())
            .map_err(|e| RpcError::Dcps(alloc::format!("create_datareader: {e:?}")))?;
        // Synthetischer Writer-GUID — eindeutig pro Process-Run.
        let writer_guid = synthesize_writer_guid();
        Ok(Self {
            service_name: service_name.into(),
            instance_name: instance_name.into(),
            request_writer,
            reply_reader,
            writer_guid,
            next_seq: Mutex::new(1),
            pending: Arc::new(Mutex::new(HashMap::new())),
            qos: qos.clone(),
            _claim: claim,
            _phantom: PhantomData,
        })
    }

    /// Service-Name, gegen den dieser Requester arbeitet.
    #[must_use]
    pub fn service_name(&self) -> &str {
        &self.service_name
    }

    /// Service-Instance-Name (`""` falls Default-Instance).
    #[must_use]
    pub fn instance_name(&self) -> &str {
        &self.instance_name
    }

    /// Anzahl outstanding Requests.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.lock().map(|m| m.len()).unwrap_or(0)
    }

    /// Aktuelle Default-Timeout-Konfiguration.
    #[must_use]
    pub fn default_timeout(&self) -> Duration {
        self.qos.request_timeout
    }

    /// Schickt einen Request **ohne auf Reply zu warten**. Liefert
    /// einen `mpsc::Receiver`, ueber den der Caller spaeter mit
    /// [`mpsc::Receiver::recv`] (oder selbst-getriebener
    /// `tick()`-Schleife) den Reply abholt.
    ///
    /// # Errors
    /// `RpcError::Dcps` bei Encoder- oder Writer-Fehlern.
    pub fn send_request_async(
        &self,
        payload: &TIn,
    ) -> RpcResult<(SampleIdentity, mpsc::Receiver<ReplyOutcome>)> {
        let id = self.next_request_id()?;
        let header = RequestHeader::new(id, self.instance_name.clone());
        let mut user_buf = Vec::new();
        payload
            .encode(&mut user_buf)
            .map_err(|e| RpcError::Dcps(alloc::format!("encode TIn: {e}")))?;
        let frame = encode_request_frame(&header, &user_buf);
        let (tx, rx) = mpsc::channel();
        // Erst registrieren, dann senden — vermeidet Race, wenn der
        // Replier extrem schnell antwortet und unser tick vor dem
        // pending-Insert dran kaeme.
        {
            let mut pend = self
                .pending
                .lock()
                .map_err(|_| RpcError::Dcps("pending-table poisoned".into()))?;
            pend.insert(id, PendingSlot { sender: tx });
        }
        if let Err(e) = self.request_writer.write(&RawBytes::new(frame)) {
            // Slot wieder freigeben.
            if let Ok(mut pend) = self.pending.lock() {
                pend.remove(&id);
            }
            return Err(RpcError::Dcps(alloc::format!("write request: {e:?}")));
        }
        Ok((id, rx))
    }

    /// Sendet einen Oneway-Request — kein Reply erwartet, kein
    /// `pending`-Slot.
    ///
    /// # Errors
    /// `RpcError::Dcps` bei Encoder- oder Writer-Fehlern.
    pub fn send_oneway(&self, payload: &TIn) -> RpcResult<SampleIdentity> {
        let id = self.next_request_id()?;
        let header = RequestHeader::new(id, self.instance_name.clone());
        let mut user_buf = Vec::new();
        payload
            .encode(&mut user_buf)
            .map_err(|e| RpcError::Dcps(alloc::format!("encode TIn: {e}")))?;
        let frame = encode_request_frame(&header, &user_buf);
        self.request_writer
            .write(&RawBytes::new(frame))
            .map_err(|e| RpcError::Dcps(alloc::format!("write oneway: {e:?}")))?;
        Ok(id)
    }

    /// Sendet einen Request und blockiert bis Reply oder Timeout.
    ///
    /// `timeout=None` nutzt [`RpcQos::request_timeout`]; explizit ueber
    /// `Some(...)` ueberschreiben.
    ///
    /// # Errors
    /// * `RpcError::Timeout` wenn waehrend `timeout` kein Reply ankam.
    /// * `RpcError::RemoteException(code)` falls Server-Side eine
    ///   `RemoteExceptionCode != Ok` zurueckgemeldet hat.
    /// * `RpcError::Dcps` bei Encode/Decode/Writer-Fehlern.
    pub fn send_request_blocking(
        &self,
        payload: &TIn,
        timeout: Option<Duration>,
    ) -> RpcResult<TOut> {
        let timeout = timeout.unwrap_or(self.qos.request_timeout);
        let (_id, rx) = self.send_request_async(payload)?;
        let deadline = std::time::Instant::now() + timeout;
        let poll = Duration::from_millis(2);
        loop {
            // 1. Vor dem Receive-Versuch einmal Replies einsammeln.
            self.tick();
            match rx.try_recv() {
                Ok(Ok(bytes)) => {
                    let out = TOut::decode(&bytes)
                        .map_err(|e| RpcError::Dcps(alloc::format!("decode TOut: {e}")))?;
                    return Ok(out);
                }
                Ok(Err(code)) => return Err(RpcError::RemoteException(code.as_u32())),
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(RpcError::Dcps("reply channel disconnected".into()));
                }
            }
            if std::time::Instant::now() >= deadline {
                return Err(RpcError::Timeout);
            }
            std::thread::sleep(poll);
        }
    }

    /// Liest neue Replies aus dem Reader, korreliert via
    /// `related_request_id`, und feuert die zugeordneten
    /// `mpsc::Sender`. Idempotent (kein Reply ⇒ no-op).
    pub fn tick(&self) {
        let samples = match self.reply_reader.take() {
            Ok(s) => s,
            Err(_) => return,
        };
        if samples.is_empty() {
            return;
        }
        let mut pend = match self.pending.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
        for raw in samples {
            let bytes = raw.data;
            let (header, payload) = match decode_reply_frame(&bytes) {
                Ok(t) => t,
                Err(_) => continue, // Malformed reply ⇒ silent drop.
            };
            let Some(slot) = pend.remove(&header.related_request_id) else {
                // Reply, dem keine offene Request entspricht — z.B. duplicate
                // delivery, late-after-timeout. Verwerfen.
                continue;
            };
            let payload_owned = payload.to_vec();
            let result = if header.remote_ex == RemoteExceptionCode::Ok {
                Ok(payload_owned)
            } else {
                Err(header.remote_ex)
            };
            // `send` failt nur, wenn Receiver gedropt wurde — egal.
            let _ = slot.sender.send(result);
        }
    }

    fn next_request_id(&self) -> RpcResult<SampleIdentity> {
        let mut g = self
            .next_seq
            .lock()
            .map_err(|_| RpcError::Dcps("seq counter poisoned".into()))?;
        let sn = *g;
        *g = sn.checked_add(1).ok_or_else(|| {
            RpcError::Dcps("rpc sequence-number wrapped — ran out of u64 space".into())
        })?;
        Ok(SampleIdentity::new(self.writer_guid, sn))
    }

    /// Test-Helper: Drainiert die offline-gequeueten Request-Bytes des
    /// Writers — ermoeglicht es, die Bytes manuell in einen Replier-
    /// Reader zu pumpen (bypass der DCPS-Live-Runtime).
    #[doc(hidden)]
    #[must_use]
    pub fn __drain_request_writer(&self) -> Vec<Vec<u8>> {
        self.request_writer.__drain_pending()
    }

    /// Test-Helper: pusht einen Reply-Frame direkt in die Reader-Inbox.
    /// Nur fuer Offline-Tests gedacht.
    #[doc(hidden)]
    pub fn __push_reply_raw(&self, bytes: Vec<u8>) -> RpcResult<()> {
        self.reply_reader
            .__push_raw(bytes)
            .map_err(|e| RpcError::Dcps(alloc::format!("push raw: {e:?}")))
    }

    /// Test-Helper: liefert eine `Clone` der Writer-GUID. Nicht stabil,
    /// nur fuer Test-Inspektion.
    #[doc(hidden)]
    #[must_use]
    pub fn __writer_guid(&self) -> [u8; 16] {
        self.writer_guid
    }
}

// ---------------------------------------------------------------------
// Helper: synthetischer Writer-GUID
// ---------------------------------------------------------------------

fn synthesize_writer_guid() -> [u8; 16] {
    use std::sync::atomic::{AtomicU64, Ordering};
    // Pro Requester ein eigener counter-Suffix; 8 byte Process-Salt
    // + 8 byte Counter. Ueber Process-Boundaries ist das nicht
    // global-unique, aber fuer Foundation-Korrelations-Tests (sowie
    // fuer mehrere Requester pro Process) ausreichend.
    static SALT: std::sync::OnceLock<[u8; 8]> = std::sync::OnceLock::new();
    static CTR: AtomicU64 = AtomicU64::new(1);
    let salt = *SALT.get_or_init(|| {
        // Ohne externe rng-Crate: System-Zeit + Process-ID + Stack-Probe-
        // Address ueber-XOR. Nicht crypto-strong, aber fuer eine
        // Process-lokale Disambiguierung von Requestern reicht es.
        let probe: alloc::boxed::Box<u8> = alloc::boxed::Box::new(0u8);
        let addr = (&*probe as *const u8) as u64;
        drop(probe);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0xCAFE_BABE_DEAD_BEEF);
        let pid = std::process::id() as u64;
        let mix = addr ^ now ^ pid ^ 0xA5A5_A5A5_A5A5_A5A5;
        mix.to_le_bytes()
    });
    let counter = CTR.fetch_add(1, Ordering::Relaxed);
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&salt);
    out[8..].copy_from_slice(&counter.to_le_bytes());
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::common_types::ReplyHeader;
    use zerodds_dcps::factory::DomainParticipantFactory;
    use zerodds_dcps::qos::DomainParticipantQos;

    fn participant(domain: i32) -> DomainParticipant {
        DomainParticipantFactory::instance()
            .create_participant_offline(domain, DomainParticipantQos::default())
    }

    #[test]
    fn synthesize_writer_guid_is_unique_per_call() {
        let a = synthesize_writer_guid();
        let b = synthesize_writer_guid();
        assert_ne!(a, b);
        // Salt-Bytes (erste 8) sind fuer beide gleich, Counter-Bytes
        // unterscheiden sich.
        assert_eq!(&a[..8], &b[..8]);
        assert_ne!(&a[8..], &b[8..]);
    }

    #[test]
    fn requester_new_creates_topics_and_writer() {
        let p = participant(101);
        let q = RpcQos::default_basic();
        let r = Requester::<RawBytes, RawBytes>::new(&p, "Calc", &q).unwrap();
        assert_eq!(r.service_name(), "Calc");
        assert_eq!(r.instance_name(), "");
        assert_eq!(r.pending_count(), 0);
    }

    #[test]
    fn requester_invalid_service_name_rejected() {
        let p = participant(102);
        let q = RpcQos::default_basic();
        let err = Requester::<RawBytes, RawBytes>::new(&p, "", &q).unwrap_err();
        assert!(matches!(err, RpcError::InvalidServiceName(_)));
    }

    #[test]
    fn requester_uses_qos_default_timeout() {
        let p = participant(103);
        let mut q = RpcQos::default_basic();
        q.request_timeout = Duration::from_millis(7);
        let r = Requester::<RawBytes, RawBytes>::new(&p, "Calc", &q).unwrap();
        assert_eq!(r.default_timeout(), Duration::from_millis(7));
    }

    #[test]
    fn send_request_async_assigns_unique_sample_ids() {
        let p = participant(104);
        let q = RpcQos::default_basic();
        let r = Requester::<RawBytes, RawBytes>::new(&p, "Calc", &q).unwrap();
        let payload = RawBytes::new(alloc::vec![1, 2, 3]);
        let (id1, _rx1) = r.send_request_async(&payload).unwrap();
        let (id2, _rx2) = r.send_request_async(&payload).unwrap();
        assert_ne!(id1.sequence_number, id2.sequence_number);
        assert_eq!(id1.writer_guid, id2.writer_guid);
        assert_eq!(r.pending_count(), 2);
    }

    #[test]
    fn send_request_async_increments_seq_monotonically() {
        let p = participant(105);
        let q = RpcQos::default_basic();
        let r = Requester::<RawBytes, RawBytes>::new(&p, "Calc", &q).unwrap();
        let payload = RawBytes::new(alloc::vec![]);
        let (id1, _rx1) = r.send_request_async(&payload).unwrap();
        let (id2, _rx2) = r.send_request_async(&payload).unwrap();
        let (id3, _rx3) = r.send_request_async(&payload).unwrap();
        assert_eq!(id1.sequence_number + 1, id2.sequence_number);
        assert_eq!(id2.sequence_number + 1, id3.sequence_number);
    }

    #[test]
    fn send_oneway_does_not_register_pending_slot() {
        let p = participant(106);
        let q = RpcQos::default_basic();
        let r = Requester::<RawBytes, RawBytes>::new(&p, "Calc", &q).unwrap();
        let payload = RawBytes::new(alloc::vec![9]);
        let id = r.send_oneway(&payload).unwrap();
        assert!(id.sequence_number > 0);
        assert_eq!(r.pending_count(), 0);
    }

    #[test]
    fn send_request_blocking_times_out_when_no_reply() {
        let p = participant(107);
        let mut q = RpcQos::default_basic();
        q.request_timeout = Duration::from_millis(20);
        let r = Requester::<RawBytes, RawBytes>::new(&p, "Calc", &q).unwrap();
        let err = r
            .send_request_blocking(&RawBytes::new(alloc::vec![1]), None)
            .unwrap_err();
        assert!(matches!(err, RpcError::Timeout));
    }

    #[test]
    fn duplicate_instance_name_rejected_on_same_participant() {
        let p = participant(108);
        let q = RpcQos::default_basic();
        let _r1 = Requester::<RawBytes, RawBytes>::with_instance(&p, "Calc", "calc-A", &q).unwrap();
        let err =
            Requester::<RawBytes, RawBytes>::with_instance(&p, "Calc", "calc-A", &q).unwrap_err();
        assert!(matches!(err, RpcError::DuplicateInstanceName(ref n) if n == "calc-A"));
    }

    #[test]
    fn duplicate_instance_name_freed_after_drop() {
        let p = participant(109);
        let q = RpcQos::default_basic();
        {
            let _r1 =
                Requester::<RawBytes, RawBytes>::with_instance(&p, "Calc", "calc-X", &q).unwrap();
        }
        // Nach dem Drop muss der Slot wieder frei sein.
        let _r2 = Requester::<RawBytes, RawBytes>::with_instance(&p, "Calc", "calc-X", &q).unwrap();
    }

    #[test]
    fn anonymous_instance_name_allows_multiple_requesters() {
        let p = participant(110);
        let q = RpcQos::default_basic();
        let _r1 = Requester::<RawBytes, RawBytes>::new(&p, "Calc", &q).unwrap();
        let _r2 = Requester::<RawBytes, RawBytes>::new(&p, "Calc", &q).unwrap();
    }

    #[test]
    fn tick_correlates_reply_with_pending_slot() {
        let p = participant(111);
        let q = RpcQos::default_basic();
        let r = Requester::<RawBytes, RawBytes>::new(&p, "Calc", &q).unwrap();
        let (id, rx) = r
            .send_request_async(&RawBytes::new(alloc::vec![1]))
            .unwrap();
        // Manuell einen passenden Reply-Frame in den Reader pushen.
        let reply_header = ReplyHeader::new(id, RemoteExceptionCode::Ok);
        let frame = crate::wire_codec::encode_reply_frame(&reply_header, &[7u8, 8, 9]);
        r.__push_reply_raw(frame).unwrap();
        r.tick();
        let result = rx.try_recv().expect("reply expected after tick");
        let bytes = result.expect("ok reply");
        assert_eq!(bytes, alloc::vec![7u8, 8, 9]);
        assert_eq!(r.pending_count(), 0);
    }

    #[test]
    fn tick_drops_reply_without_matching_request() {
        let p = participant(112);
        let q = RpcQos::default_basic();
        let r = Requester::<RawBytes, RawBytes>::new(&p, "Calc", &q).unwrap();
        let bogus = SampleIdentity::new([0xFF; 16], 999);
        let frame = crate::wire_codec::encode_reply_frame(
            &ReplyHeader::new(bogus, RemoteExceptionCode::Ok),
            &[],
        );
        r.__push_reply_raw(frame).unwrap();
        r.tick();
        // Kein Pending-Slot, daher kein Effekt.
        assert_eq!(r.pending_count(), 0);
    }

    #[test]
    fn tick_propagates_remote_exception_to_caller() {
        let p = participant(113);
        let q = RpcQos::default_basic();
        let r = Requester::<RawBytes, RawBytes>::new(&p, "Calc", &q).unwrap();
        let (id, rx) = r.send_request_async(&RawBytes::new(alloc::vec![])).unwrap();
        let frame = crate::wire_codec::encode_reply_frame(
            &ReplyHeader::new(id, RemoteExceptionCode::InvalidArgument),
            &[],
        );
        r.__push_reply_raw(frame).unwrap();
        r.tick();
        let res = rx.try_recv().expect("reply expected");
        assert_eq!(res, Err(RemoteExceptionCode::InvalidArgument));
    }

    #[test]
    fn tick_handles_malformed_reply_silently() {
        let p = participant(114);
        let q = RpcQos::default_basic();
        let r = Requester::<RawBytes, RawBytes>::new(&p, "Calc", &q).unwrap();
        let (_id, _rx) = r.send_request_async(&RawBytes::new(alloc::vec![])).unwrap();
        r.__push_reply_raw(alloc::vec![0u8; 4]).unwrap(); // Truncated header.
        r.tick();
        // Pending bleibt unveraendert — malformed reply wird verworfen.
        assert_eq!(r.pending_count(), 1);
    }

    #[test]
    fn drain_request_writer_yields_encoded_frames() {
        let p = participant(115);
        let q = RpcQos::default_basic();
        let r = Requester::<RawBytes, RawBytes>::new(&p, "Calc", &q).unwrap();
        let _ = r
            .send_oneway(&RawBytes::new(alloc::vec![0xDE, 0xAD]))
            .unwrap();
        let frames = r.__drain_request_writer();
        assert_eq!(frames.len(), 1);
        let (header, payload) = crate::wire_codec::decode_request_frame(&frames[0]).unwrap();
        assert_eq!(payload, &[0xDE, 0xAD]);
        // Header instance_name=""
        assert_eq!(header.instance_name, "");
    }
}
