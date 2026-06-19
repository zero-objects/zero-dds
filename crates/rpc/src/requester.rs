// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! [`Requester`] — client side of a DDS-RPC service (Spec §7.9).
//!
//! Threading model of the foundation stage (C6.1.C):
//!
//! * **Synchronous API**, no async-runtime requirement. `send_request_blocking`
//!   internally calls [`Requester::tick`] in a polling loop until reply or
//!   timeout. Callers with their own event loop can use `send_request_async`,
//!   which only sends the request and returns an `mpsc::Receiver`;
//!   they are then responsible themselves for regularly
//!   calling `tick()`.
//! * Correlation: each request gets a unique
//!   [`SampleIdentity`] that goes onto the wire in the [`RequestHeader`]. The
//!   replier sets it in `ReplyHeader::related_request_id`. `tick`
//!   reads new replies, finds the matching `mpsc::Sender` and routes the
//!   response.
//!
//! Wire frame: see [`crate::wire_codec`].
//!
//! Spec correlation: the spec additionally requires that the reply DATA
//! carries `PID_RELATED_SAMPLE_IDENTITY` in the inline-QoS block
//! ([`zerodds_rtps::inline_qos`]). This path is activated in C6.1.D
//! when the DCPS DataWriter exposes an inline-QoS API. Until
//! then, the header-in-payload path suffices for foundation tests.

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
// Service instance registry — prevents duplicates on one participant
// ---------------------------------------------------------------------

/// Role of an endpoint in the instance registry — requester and replier
/// for the same `(service, instance)` coexist on one participant
/// (Spec §7.6.2). Only the same role is doubly occupied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum InstanceRole {
    Requester,
    Replier,
}

/// Key: `(participant_pointer, role, service_name, instance_name)`.
type InstanceKey = (usize, InstanceRole, String, String);

fn instance_registry() -> &'static Mutex<std::collections::HashSet<InstanceKey>> {
    use std::sync::OnceLock;
    static REGISTRY: OnceLock<Mutex<std::collections::HashSet<InstanceKey>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

fn participant_addr(p: &DomainParticipant) -> usize {
    // The address of the `DomainParticipant` wrapper suffices as a process-local
    // token. The `Arc` content stays stable over its lifetime.
    core::ptr::from_ref(p) as usize
}

pub(crate) fn try_claim_instance(
    p: &DomainParticipant,
    role: InstanceRole,
    service_name: &str,
    instance_name: &str,
) -> RpcResult<InstanceClaim> {
    if instance_name.is_empty() {
        // Anonymous endpoints allow multiple registration — Spec §7.6.2
        // does not distinguish the default instance from a single endpoint.
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

/// RAII slot that releases a `(participant, service, instance)` entry on
/// drop. This keeps the registry clean when a
/// requester/replier is dropped.
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

/// Reply payload to the waiting caller. `Ok(bytes)` ⇒
/// `RemoteExceptionCode::Ok` with user payload bytes; `Err(code)` ⇒
/// server-side exception without payload.
pub type ReplyOutcome = Result<Vec<u8>, RemoteExceptionCode>;

/// Pending slot per outstanding request.
struct PendingSlot {
    sender: mpsc::Sender<ReplyOutcome>,
}

/// Client side of a DDS-RPC service.
///
/// `TIn` is the **user request payload type** (e.g. `Calculator_AddRequest`),
/// `TOut` the **user reply payload type**. Both must implement `DdsType`
/// — encoding/decoding goes through [`DdsType::encode`] and
/// [`DdsType::decode`].
pub struct Requester<TIn: DdsType, TOut: DdsType> {
    service_name: String,
    instance_name: String,
    request_writer: DataWriter<RawBytes>,
    reply_reader: DataReader<RawBytes>,
    /// 16-byte writer GUID that marks every request. The value is
    /// injected by the DataWriter layer from the DCPS runtime (RTPS GUID
    /// of the request writer); `Requester::new` only synthesizes an
    /// initial value for tests, which is replaced before the first send by
    /// `set_writer_guid`.
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
    /// Creates a new requester against `service_name`.
    ///
    /// Produces two topics — `<service>_Request` (writer) and
    /// `<service>_Reply` (reader) — and a `Publisher`/`Subscriber`
    /// pair. `instance_name=""` means "default instance, no
    /// `PID_SERVICE_INSTANCE_NAME`".
    ///
    /// # Errors
    /// * `RpcError::InvalidServiceName` if the name is empty/illegal.
    /// * `RpcError::Dcps` on topic/writer/reader creation errors.
    /// * `RpcError::DuplicateInstanceName` if a requester/replier with
    ///   the same `(service, instance)` pair already runs on the same
    ///   participant.
    pub fn new(
        participant: &DomainParticipant,
        service_name: &str,
        qos: &RpcQos,
    ) -> RpcResult<Self> {
        Self::with_instance(participant, service_name, "", qos)
    }

    /// Like [`Self::new`], but with an explicit `service_instance_name`
    /// (Spec §7.8.2 PID 0x0080).
    ///
    /// # Errors
    /// See [`Self::new`].
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

    /// Service name this requester works against.
    #[must_use]
    pub fn service_name(&self) -> &str {
        &self.service_name
    }

    /// Service instance name (`""` if default instance).
    #[must_use]
    pub fn instance_name(&self) -> &str {
        &self.instance_name
    }

    /// Number of outstanding requests.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.lock().map(|m| m.len()).unwrap_or(0)
    }

    /// Current default-timeout configuration.
    #[must_use]
    pub fn default_timeout(&self) -> Duration {
        self.qos.request_timeout
    }

    /// Sends a request **without waiting for a reply**. Returns
    /// an `mpsc::Receiver` through which the caller later picks up the
    /// reply with [`mpsc::Receiver::recv`] (or a self-driven
    /// `tick()` loop).
    ///
    /// # Errors
    /// `RpcError::Dcps` on encoder or writer errors.
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
        // Register first, then send — avoids a race if the
        // replier answers extremely fast and our tick would run before the
        // pending insert.
        {
            let mut pend = self
                .pending
                .lock()
                .map_err(|_| RpcError::Dcps("pending-table poisoned".into()))?;
            pend.insert(id, PendingSlot { sender: tx });
        }
        if let Err(e) = self.request_writer.write(&RawBytes::new(frame)) {
            // Release the slot again.
            if let Ok(mut pend) = self.pending.lock() {
                pend.remove(&id);
            }
            return Err(RpcError::Dcps(alloc::format!("write request: {e:?}")));
        }
        Ok((id, rx))
    }

    /// Sends a oneway request — no reply expected, no
    /// `pending` slot.
    ///
    /// # Errors
    /// `RpcError::Dcps` on encoder or writer errors.
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

    /// Sends a request and blocks until reply or timeout.
    ///
    /// `timeout=None` uses [`RpcQos::request_timeout`]; override explicitly
    /// via `Some(...)`.
    ///
    /// # Errors
    /// * `RpcError::Timeout` if no reply arrived during `timeout`.
    /// * `RpcError::RemoteException(code)` if the server side reported a
    ///   `RemoteExceptionCode != Ok`.
    /// * `RpcError::Dcps` on encode/decode/writer errors.
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
            // 1. Collect replies once before the receive attempt.
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

    /// Reads new replies from the reader, correlates via
    /// `related_request_id`, and fires the associated
    /// `mpsc::Sender`. Idempotent (no reply ⇒ no-op).
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
                // A reply with no matching open request — e.g. duplicate
                // delivery, late-after-timeout. Discard.
                continue;
            };
            let payload_owned = payload.to_vec();
            let result = if header.remote_ex == RemoteExceptionCode::Ok {
                Ok(payload_owned)
            } else {
                Err(header.remote_ex)
            };
            // `send` only fails if the receiver was dropped — irrelevant.
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

    /// Test helper: drains the offline-queued request bytes of the
    /// writer — lets you manually pump the bytes into a replier
    /// reader (bypassing the DCPS live runtime).
    #[doc(hidden)]
    #[must_use]
    pub fn __drain_request_writer(&self) -> Vec<Vec<u8>> {
        self.request_writer.__drain_pending()
    }

    /// Test helper: pushes a reply frame directly into the reader inbox.
    /// Intended only for offline tests.
    #[doc(hidden)]
    pub fn __push_reply_raw(&self, bytes: Vec<u8>) -> RpcResult<()> {
        self.reply_reader
            .__push_raw(bytes)
            .map_err(|e| RpcError::Dcps(alloc::format!("push raw: {e:?}")))
    }

    /// Test helper: returns a `Clone` of the writer GUID. Not stable,
    /// only for test inspection.
    #[doc(hidden)]
    #[must_use]
    pub fn __writer_guid(&self) -> [u8; 16] {
        self.writer_guid
    }
}

// ---------------------------------------------------------------------
// Helper: synthetic writer GUID
// ---------------------------------------------------------------------

fn synthesize_writer_guid() -> [u8; 16] {
    use std::sync::atomic::{AtomicU64, Ordering};
    // A dedicated counter suffix per requester; 8 bytes process salt
    // + 8 bytes counter. Across process boundaries this is not
    // globally unique, but for foundation correlation tests (as well as
    // for multiple requesters per process) it suffices.
    static SALT: std::sync::OnceLock<[u8; 8]> = std::sync::OnceLock::new();
    static CTR: AtomicU64 = AtomicU64::new(1);
    let salt = *SALT.get_or_init(|| {
        // Without an external rng crate: system time + process id + stack-probe
        // address XOR'd together. Not crypto-strong, but for a
        // process-local disambiguation of requesters it suffices.
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
        // The salt bytes (first 8) are the same for both, the counter bytes
        // differ.
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
        // After the drop the slot must be free again.
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
        // Manually push a matching reply frame into the reader.
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
        // No pending slot, hence no effect.
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
        // Pending stays unchanged — a malformed reply is discarded.
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
