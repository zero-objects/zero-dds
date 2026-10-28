// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! [`Requester`] — client side of a DDS-RPC service (Spec §7.2.1, §7.9, §7.11.1.4.3).
//!
//! Threading model of the foundation stage (C6.1.C):
//!
//! * **Synchronous API**, no async-runtime requirement. `send_request_blocking`
//!   internally calls [`Requester::tick`] in a polling loop until reply or
//!   timeout. Callers with their own event loop can use `send_request_async`,
//!   which only sends the request and returns an `mpsc::Receiver`;
//!   they are then responsible themselves for regularly
//!   calling `tick()`.
//! * Correlation under the Basic mapping (§7.8.1): each request gets a
//!   unique [`SampleIdentity`] that goes onto the wire in the
//!   [`RequestHeader`]. The replier sets it in
//!   `ReplyHeader::related_request_id`. `tick` reads new replies, finds
//!   the matching `mpsc::Sender` and routes the response.
//! * Correlation under the Enhanced mapping (§7.8.2): the request is
//!   identified by the writer GUID and sequence number of its DATA and
//!   the reply carries them in `PID_RELATED_SAMPLE_IDENTITY`; there is no
//!   header (see [`crate::enhanced`]). The client also waits for the
//!   service to be discovered completely before it sends (§7.6.2.2.2).
//!
//! Wire frame of the Basic mapping: see [`crate::wire_codec`].

extern crate alloc;

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::time::Duration;
use std::collections::HashMap;
use std::sync::{Mutex, mpsc};

use zerodds_dcps::dds_type::{DdsType, RawBytes};
use zerodds_dcps::entity::Entity;
use zerodds_dcps::listener::DataReaderListener;
use zerodds_dcps::participant::DomainParticipant;
use zerodds_dcps::psm_constants::status;
use zerodds_dcps::publisher::DataWriter;
use zerodds_dcps::qos::{PublisherQos, SubscriberQos};
use zerodds_dcps::runtime::RpcDiscoveryExt;
use zerodds_dcps::subscriber::DataReader;

use crate::common_types::{RemoteExceptionCode, RequestHeader, SampleIdentity};
use crate::discovery_ext::service_matching_complete;
use crate::endpoint_options::{RpcEndpointOptions, RpcMapping, ServiceMatching};
use crate::enhanced::decode_remote_ex_reply;
use crate::error::{RpcError, RpcResult};
use crate::header_pair::RequestWithHeader;
use crate::qos_profile::RpcQos;
use crate::topic_naming::{AnnotatedTopics, BindingStyle, ServiceTopicNames};
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

/// A data-available listener that calls a closure.
struct ArrivalListener<F: Fn() + Send + Sync>(F);

impl<F: Fn() + Send + Sync> DataReaderListener for ArrivalListener<F> {
    fn on_data_available(&self, _reader: zerodds_dcps::instance_handle::InstanceHandle) {
        (self.0)();
    }
}

/// The request writer's QoS: the RPC QoS and, for classic-CDR Basic
/// samples, the XCDR1 data representation (§7.5.1.1: DDS 1.3).
fn writer_qos(qos: &RpcQos, options: &RpcEndpointOptions) -> zerodds_dcps::qos::DataWriterQos {
    let mut writer = qos.request_writer_qos();
    // The samples of the Enhanced mapping are XCDR2 (§7.5.1.2), those of the Basic
    // mapping the encoding the options name; the writer offers the one it writes.
    writer.data_representation = if options.xcdr1() {
        crate::endpoint_options::WireEncoding::Xcdr1.representation()
    } else {
        crate::endpoint_options::WireEncoding::Xcdr2.representation()
    };
    writer
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
    /// 16-byte writer GUID that marks every request. On a live runtime it is
    /// the RTPS GUID of the request [`DataWriter`]; the request header then
    /// carries the identity of the sample itself (Spec §7.2.1). `Requester`
    /// only synthesizes a GUID in offline mode, where the writer has no RTPS
    /// identity.
    writer_guid: [u8; 16],
    next_seq: Mutex<u64>,
    /// Serializes identity allocation and the write so the sequence number
    /// the runtime stamps on the sample matches the one the header names.
    send_lock: Mutex<()>,
    pending: Arc<Mutex<HashMap<SampleIdentity, PendingSlot>>>,
    qos: RpcQos,
    options: RpcEndpointOptions,
    /// For a service defined by a pair of types with their own header
    /// (§7.3.1.4): puts the request's identity into the request's header.
    pair_stamp: Option<fn(&TIn, &RequestHeader) -> RpcResult<TIn>>,
    /// Enhanced replies that carried no `PID_RELATED_SAMPLE_IDENTITY` and
    /// so could not be correlated with a request.
    uncorrelated_replies: std::sync::atomic::AtomicU64,
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
    /// (Spec §7.6.2.1.1, `PID_SERVICE_INSTANCE_NAME` 0x0080).
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
        Self::from_topics(
            participant,
            service_name,
            instance_name,
            qos,
            topics,
            &RpcEndpointOptions::default(),
            None,
        )
    }

    /// Like [`Self::with_instance`] under an explicit binding style
    /// (§7.2.2): the request-reply style does not capture
    /// `<interface_name>` (§7.4.1), the function-call style does.
    ///
    /// # Errors
    /// See [`Self::new`].
    pub fn with_instance_style(
        participant: &DomainParticipant,
        service_name: &str,
        instance_name: &str,
        qos: &RpcQos,
        style: BindingStyle,
    ) -> RpcResult<Self> {
        let topics = ServiceTopicNames::synthesize(
            style,
            service_name,
            &[],
            crate::topic_naming::DEFAULT_SERVICE_NAME,
            None,
            None,
            &crate::topic_naming::TopicNameOverrides::none(),
        )?;
        Self::from_topics(
            participant,
            service_name,
            instance_name,
            qos,
            topics,
            &RpcEndpointOptions::default(),
            None,
        )
    }

    /// Like [`Self::with_instance`], but composing the topic pair from
    /// DDS-RPC 1.0 §7.4's full rule instead of always defaulting to
    /// `<interface>_Service_Request`/`_Reply`: `topics` carries the
    /// interface's own `@service`/`@DDSRequestTopic`/`@DDSReplyTopic`
    /// annotation values (or [`AnnotatedTopics::none`] where it declares
    /// none) and, on top of them, any run-time override §7.4.2.3 admits.
    /// §7.9 defines the `Requester` over exactly these inputs a service
    /// declares — a generated binding that parses its own IDL's
    /// annotations reaches this constructor instead of always the
    /// default one.
    ///
    /// # Errors
    /// See [`Self::new`].
    pub fn with_topics(
        participant: &DomainParticipant,
        qualified_interface: &str,
        instance_name: &str,
        qos: &RpcQos,
        topics: &AnnotatedTopics,
    ) -> RpcResult<Self> {
        Self::with_topics_style(
            participant,
            qualified_interface,
            instance_name,
            qos,
            BindingStyle::FunctionCall,
            topics,
        )
    }

    /// Like [`Self::with_topics`] under an explicit binding style
    /// (§7.2.2, §7.4.1): the request-reply style drops the interface
    /// segment of the topic name.
    ///
    /// # Errors
    /// See [`Self::new`].
    pub fn with_topics_style(
        participant: &DomainParticipant,
        qualified_interface: &str,
        instance_name: &str,
        qos: &RpcQos,
        style: BindingStyle,
        topics: &AnnotatedTopics,
    ) -> RpcResult<Self> {
        Self::with_options(
            participant,
            qualified_interface,
            instance_name,
            qos,
            style,
            topics,
            &RpcEndpointOptions::default(),
        )
    }

    /// The general constructor: [`Self::with_topics_style`] with the
    /// service mapping, the DDS type names and the discovery rules of
    /// `options` (see [`RpcEndpointOptions`]).
    ///
    /// Under [`RpcMapping::Enhanced`] the endpoints announce the
    /// discovery extension of §7.6.2.1 — `related_datareader_key` /
    /// `related_datawriter_key` naming each other and the topic aliases of
    /// `options.parents`, but no `service_instance_name` on the client
    /// side — and a request is sent only when the service is discovered
    /// completely (§7.6.2.2.2).
    ///
    /// # Errors
    /// See [`Self::new`].
    pub fn with_options(
        participant: &DomainParticipant,
        qualified_interface: &str,
        instance_name: &str,
        qos: &RpcQos,
        style: BindingStyle,
        topics: &AnnotatedTopics,
        options: &RpcEndpointOptions,
    ) -> RpcResult<Self> {
        let parents: Vec<&str> = options.parents.iter().map(String::as_str).collect();
        let resolved = topics.resolve(style, qualified_interface, &parents)?;
        Self::from_topics(
            participant,
            qualified_interface,
            instance_name,
            qos,
            resolved,
            options,
            None,
        )
    }

    /// Shared tail of [`Self::with_instance`] and [`Self::with_topics`]:
    /// everything after the topic pair is already resolved.
    fn from_topics(
        participant: &DomainParticipant,
        service_name: &str,
        instance_name: &str,
        qos: &RpcQos,
        topics: ServiceTopicNames,
        options: &RpcEndpointOptions,
        pair_stamp: Option<fn(&TIn, &RequestHeader) -> RpcResult<TIn>>,
    ) -> RpcResult<Self> {
        let claim = try_claim_instance(
            participant,
            InstanceRole::Requester,
            service_name,
            instance_name,
        )?;
        // §7.5.1.1.6/7: the topic types are `${I}_Request` and
        // `${I}_Reply`, not the type of the byte buffer the samples are
        // carried in.
        let type_names = options.type_names_for(service_name);
        let request_topic = crate::endpoint_options::create_topic(
            participant,
            &topics.request,
            &type_names.request,
            options.type_information.as_ref().map(|i| &i.request),
        )?;
        let reply_topic = crate::endpoint_options::create_topic(
            participant,
            &topics.reply,
            &type_names.reply,
            options.type_information.as_ref().map(|i| &i.reply),
        )?;
        let publisher = participant.create_publisher(PublisherQos::default());
        let subscriber = participant.create_subscriber(SubscriberQos::default());
        let request_writer = publisher
            .create_datawriter::<RawBytes>(&request_topic, writer_qos(qos, options))
            .map_err(|e| RpcError::Dcps(alloc::format!("create_datawriter: {e:?}")))?;
        let reply_reader = subscriber
            .create_datareader::<RawBytes>(&reply_topic, qos.reply_reader_qos())
            .map_err(|e| RpcError::Dcps(alloc::format!("create_datareader: {e:?}")))?;
        if options.mapping == RpcMapping::Enhanced {
            // §7.6.2.1: the client-side endpoints name each other, carry
            // the topic aliases, and do not carry a service instance name.
            let aliases = |list: &[String]| (!list.is_empty()).then(|| list.to_vec());
            if let Some(reply_reader_guid) = reply_reader.rtps_guid() {
                request_writer
                    .set_rpc_discovery(&RpcDiscoveryExt {
                        service_instance_name: None,
                        related_entity_guid: Some(reply_reader_guid),
                        topic_aliases: aliases(&topics.request_aliases),
                    })
                    .map_err(|e| {
                        RpcError::Dcps(alloc::format!("announce request writer: {e:?}"))
                    })?;
            }
            if let Some(request_writer_guid) = request_writer.rtps_guid() {
                reply_reader
                    .set_rpc_discovery(&RpcDiscoveryExt {
                        service_instance_name: None,
                        related_entity_guid: Some(request_writer_guid),
                        topic_aliases: aliases(&topics.reply_aliases),
                    })
                    .map_err(|e| RpcError::Dcps(alloc::format!("announce reply reader: {e:?}")))?;
            }
        }
        // On a live runtime the request identity is the RTPS identity of the
        // request writer; offline there is none and a process-local GUID
        // stands in (Spec §7.2.1).
        let writer_guid = request_writer
            .rtps_guid()
            .unwrap_or_else(synthesize_writer_guid);
        Ok(Self {
            service_name: service_name.into(),
            instance_name: instance_name.into(),
            request_writer,
            reply_reader,
            writer_guid,
            next_seq: Mutex::new(1),
            send_lock: Mutex::new(()),
            pending: Arc::new(Mutex::new(HashMap::new())),
            qos: qos.clone(),
            options: options.clone(),
            pair_stamp,
            uncorrelated_replies: std::sync::atomic::AtomicU64::new(0),
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

    /// The request topic this requester actually writes to — §7.4.1's
    /// name, after annotations and run-time overrides
    /// ([`Self::with_topics`]) have been applied. Not necessarily
    /// `<service>_Service_Request`.
    #[must_use]
    pub fn request_topic_name(&self) -> &str {
        self.request_writer.topic().name()
    }

    /// The reply topic this requester actually reads from — the other
    /// half of [`Self::request_topic_name`].
    #[must_use]
    pub fn reply_topic_name(&self) -> &str {
        self.reply_reader.topic().name()
    }

    /// The DDS type name of the request topic: `${interfaceName}_Request`
    /// (§7.5.1.1.6 rule 1).
    #[must_use]
    pub fn request_type_name(&self) -> &str {
        self.request_writer.topic().type_name()
    }

    /// The DDS type name of the reply topic: `${interfaceName}_Reply`
    /// (§7.5.1.1.7 rule 1).
    #[must_use]
    pub fn reply_type_name(&self) -> &str {
        self.reply_reader.topic().type_name()
    }

    /// Whether the request writer is matched with a reader. Under the
    /// Basic mapping this is all there is to know about the service
    /// (§7.6.1); [`Self::service_matched`] is the Enhanced algorithm.
    #[must_use]
    pub fn request_writer_matched(&self) -> bool {
        self.request_writer.matched_subscription_count() > 0
    }

    /// Delivers the replies as they arrive: [`Self::tick`] runs when the
    /// reply reader receives data, so the receiver of
    /// [`Self::send_request_async`] is answered without the application
    /// ticking (§7.9.2). The reader's listener refers to the requester
    /// weakly: dropping the last `Arc` ends the listening.
    pub fn activate(self: &Arc<Self>)
    where
        TIn: 'static,
        TOut: 'static,
    {
        let weak = Arc::downgrade(self);
        let listener = Arc::new(ArrivalListener(move || {
            if let Some(requester) = weak.upgrade() {
                requester.tick();
            }
        }));
        self.reply_reader
            .set_listener(Some(listener.clone()), status::DATA_AVAILABLE);
        // A reply that arrived before the listener was set is waiting.
        (listener.0)();
    }

    /// Stops the listening [`Self::activate`] began; replies are then
    /// delivered by [`Self::tick`] again.
    pub fn deactivate_listener(&self) {
        self.reply_reader.set_listener(None, status::DATA_AVAILABLE);
    }

    /// §7.9.3: deactivates the client. The request writer and the reply
    /// reader are deleted, the writer announcing its end to the peers, and
    /// the instance name is free again.
    pub fn close(self) {
        drop(self);
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

    /// Change the mutable QoS policies of the request writer and reply
    /// reader at run time — §7.10.2: "The users may modify the mutable
    /// QoS policies at run-time."
    ///
    /// # Errors
    /// `RpcError::Dcps` when the middleware refuses a policy — an
    /// immutable one changed after `enable()`, or an inconsistent
    /// combination.
    pub fn set_qos(&self, qos: &RpcQos) -> RpcResult<()> {
        self.request_writer
            .set_qos(writer_qos(qos, &self.options))
            .map_err(|e| RpcError::Dcps(alloc::format!("set request writer qos: {e:?}")))?;
        self.reply_reader
            .set_qos(qos.reply_reader_qos())
            .map_err(|e| RpcError::Dcps(alloc::format!("set reply reader qos: {e:?}")))?;
        Ok(())
    }

    /// Test seam: the request writer's current QoS.
    #[doc(hidden)]
    #[must_use]
    pub fn __request_writer_qos(&self) -> zerodds_dcps::qos::DataWriterQos {
        self.request_writer.qos()
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
        // Enhanced: the request is not sent before the service is
        // discovered (§7.6.2.2.2). Waiting happens before the send lock is
        // taken, so a request that waits does not hold up the others.
        self.wait_for_service()?;
        // Hold the send lock across identity allocation and the write: on a
        // live runtime the header names the sequence number the writer is
        // about to stamp, so no other send may slip in between.
        let _send = self
            .send_lock
            .lock()
            .map_err(|_| RpcError::Dcps("send-lock poisoned".into()))?;
        let id = self.next_request_id()?;
        let frame = self.frame_request(id, payload)?;
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

    /// The sample for request `id`: the Basic mapping's header and `Call`
    /// union (§7.5.1.1.6), or the Enhanced mapping's `Request` structure
    /// alone (§7.5.1.2.4).
    fn frame_request(&self, id: SampleIdentity, payload: &TIn) -> RpcResult<Vec<u8>> {
        match self.options.mapping {
            RpcMapping::Basic if self.pair_stamp.is_some() => {
                // §7.3.1.4: the request type is the topic type and holds
                // its own header; the sample is the whole structure.
                let stamp = self
                    .pair_stamp
                    .ok_or_else(|| RpcError::codec("no pair stamp"))?;
                let header = RequestHeader::new(id, self.instance_name.clone());
                let stamped = stamp(payload, &header)?;
                let mut frame = Vec::new();
                stamped
                    .encode_continuing(&mut frame, self.options.xcdr1())
                    .map_err(|e| RpcError::Dcps(alloc::format!("encode TIn: {e}")))?;
                Ok(frame)
            }
            RpcMapping::Basic => {
                let header = RequestHeader::new(id, self.instance_name.clone());
                // The header, the padding to the union's alignment, and the
                // `Call` union behind them: its 8-octet members align to the
                // sample under XCDR1 (§7.5.1.1), so it is encoded where it
                // stands.
                let mut frame = encode_request_frame(&header, &[])?;
                payload
                    .encode_continuing(&mut frame, self.options.xcdr1())
                    .map_err(|e| RpcError::Dcps(alloc::format!("encode TIn: {e}")))?;
                Ok(frame)
            }
            RpcMapping::Enhanced => {
                let mut user_buf = Vec::new();
                payload
                    .encode(&mut user_buf)
                    .map_err(|e| RpcError::Dcps(alloc::format!("encode TIn: {e}")))?;
                Ok(user_buf)
            }
        }
    }

    /// Whether the service this requester addresses is discovered
    /// completely — Figure 7.4 of §7.6.2.2.2. Always `true` under the Basic
    /// mapping, which has no such rule (§7.6.1).
    #[must_use]
    pub fn service_matched(&self) -> bool {
        match self.options.mapping {
            RpcMapping::Basic => true,
            RpcMapping::Enhanced => service_matching_complete(
                &self.request_writer.matched_rpc_readers(),
                &self.reply_reader.matched_rpc_writers(),
                &self.instance_name,
                self.options.accept_unannounced,
            ),
        }
    }

    /// Holds the caller until [`Self::service_matched`], as
    /// [`ServiceMatching`] says.
    fn wait_for_service(&self) -> RpcResult<()> {
        if self.options.mapping == RpcMapping::Basic
            || self.options.service_matching == ServiceMatching::Off
            || self.service_matched()
        {
            return Ok(());
        }
        if self.options.service_matching == ServiceMatching::Fail {
            return Err(RpcError::ServiceNotMatched);
        }
        let deadline = std::time::Instant::now() + self.qos.request_timeout;
        while std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
            if self.service_matched() {
                return Ok(());
            }
        }
        Err(RpcError::ServiceNotMatched)
    }

    /// Number of Enhanced replies that carried no related sample identity
    /// and so were not handed to any request.
    #[must_use]
    pub fn uncorrelated_replies(&self) -> u64 {
        self.uncorrelated_replies
            .load(std::sync::atomic::Ordering::Acquire)
    }

    /// Sends a oneway request — no reply expected, no
    /// `pending` slot.
    ///
    /// # Errors
    /// `RpcError::Dcps` on encoder or writer errors.
    pub fn send_oneway(&self, payload: &TIn) -> RpcResult<SampleIdentity> {
        self.wait_for_service()?;
        let _send = self
            .send_lock
            .lock()
            .map_err(|_| RpcError::Dcps("send-lock poisoned".into()))?;
        let id = self.next_request_id()?;
        let frame = self.frame_request(id, payload)?;
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

    /// Reads new replies from the reader, correlates them with the
    /// outstanding requests and fires the associated `mpsc::Sender`.
    /// Idempotent (no reply ⇒ no-op).
    ///
    /// Basic mapping: by `ReplyHeader::related_request_id` (§7.8.1).
    /// Enhanced mapping: by the `PID_RELATED_SAMPLE_IDENTITY` of the reply
    /// DATA (§7.8.2); a reply without one is counted in
    /// [`Self::uncorrelated_replies`].
    pub fn tick(&self) {
        let samples = match self.reply_reader.take_with_wire() {
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
        for (raw, wire) in samples {
            let bytes = raw.data;
            let (related_request_id, result) = match self.options.mapping {
                RpcMapping::Basic => {
                    let (header, payload) = match decode_reply_frame(&bytes) {
                        Ok(t) => t,
                        Err(_) => continue, // Malformed reply ⇒ silent drop.
                    };
                    let result = if header.remote_ex == RemoteExceptionCode::Ok {
                        Ok(self.reply_payload(&bytes, payload))
                    } else {
                        Err(header.remote_ex)
                    };
                    (header.related_request_id, result)
                }
                RpcMapping::Enhanced => {
                    let Some(related) = wire.and_then(|w| w.related_sample_identity) else {
                        self.uncorrelated_replies
                            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                        continue;
                    };
                    let id = SampleIdentity::new(related.writer_guid, related.sequence_number);
                    let result =
                        match decode_remote_ex_reply(&bytes, &self.options.remote_ex_member) {
                            Some(code) if code != RemoteExceptionCode::Ok => Err(code),
                            _ => Ok(bytes),
                        };
                    (id, result)
                }
            };
            // A reply with no matching open request — a duplicate delivery,
            // a late reply after a timeout, or a reply to another client on
            // a shared reply topic (§7.2.1) — is not ours.
            let Some(slot) = pend.remove(&related_request_id) else {
                continue;
            };
            // `send` only fails if the receiver was dropped — irrelevant.
            let _ = slot.sender.send(result);
        }
    }

    /// The reply's `Return` union as a caller decodes it: standalone, with
    /// `TOut::decode`.
    ///
    /// Under classic CDR the union sits at an offset of the sample that
    /// its 8-octet members align to, so it is read where it stands and
    /// written out again on its own. If the type cannot be read there, the
    /// octets go on unchanged and the caller's own decode reports why.
    fn reply_payload(&self, frame: &[u8], payload: &[u8]) -> Vec<u8> {
        // A type with its own header is the whole sample (§7.3.1.4).
        let pair = self.pair_stamp.is_some();
        let unchanged = || {
            if pair {
                frame.to_vec()
            } else {
                payload.to_vec()
            }
        };
        if !self.options.xcdr1() {
            return unchanged();
        }
        let offset = if pair { 0 } else { frame.len() - payload.len() };
        let mut standalone = Vec::new();
        match TOut::decode_continuing(frame, offset, true)
            .ok()
            .and_then(|value| value.encode(&mut standalone).ok())
        {
            Some(()) => standalone,
            None => unchanged(),
        }
    }

    fn next_request_id(&self) -> RpcResult<SampleIdentity> {
        // On a live runtime the request identity is the identity of the
        // sample: the GUID of the request writer and the sequence number the
        // RTPS writer will assign (Spec §7.2.1, §7.8.1). The caller holds
        // `send_lock` across allocation and the write, so the number query
        // returns is the one the write uses. Offline there is no runtime
        // sequence; a process-local counter stands in.
        let sn = match self.request_writer.next_sequence_number() {
            Some(next) if next >= 1 => u64::try_from(next)
                .map_err(|_| RpcError::Dcps("rpc sequence number out of u64 range".into()))?,
            _ => {
                let mut g = self
                    .next_seq
                    .lock()
                    .map_err(|_| RpcError::Dcps("seq counter poisoned".into()))?;
                let sn = *g;
                *g = sn.checked_add(1).ok_or_else(|| {
                    RpcError::Dcps("rpc sequence-number wrapped — ran out of u64 space".into())
                })?;
                sn
            }
        };
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

    /// Test helper: pushes a reply directly into the reader inbox, with
    /// the writer GUID and sequence number of its DATA and the sample it
    /// answers. Intended only for offline tests.
    #[doc(hidden)]
    pub fn __push_reply_related(
        &self,
        bytes: Vec<u8>,
        related: Option<SampleIdentity>,
    ) -> RpcResult<()> {
        self.reply_reader
            .__push_raw_related(
                bytes,
                [0xEE; 16],
                1,
                related.map(|id| {
                    zerodds_rtps::inline_qos::SampleIdentityBytes::new(
                        id.writer_guid,
                        id.sequence_number,
                    )
                }),
            )
            .map_err(|e| RpcError::Dcps(alloc::format!("push raw: {e:?}")))
    }

    /// Test helper: returns a `Clone` of the writer GUID. Not stable,
    /// only for test inspection.
    #[doc(hidden)]
    #[must_use]
    pub fn __writer_guid(&self) -> [u8; 16] {
        self.writer_guid
    }

    /// Test helper: the request writer, for reading what it announced and
    /// what its matched readers announced (`matched_rpc_readers`). Not
    /// stable, only for test inspection.
    #[doc(hidden)]
    #[must_use]
    pub fn __request_writer(&self) -> &DataWriter<RawBytes> {
        &self.request_writer
    }

    /// Test helper: the reply reader, the other endpoint of the client
    /// (`matched_rpc_writers`). Not stable, only for test inspection.
    #[doc(hidden)]
    #[must_use]
    pub fn __reply_reader(&self) -> &DataReader<RawBytes> {
        &self.reply_reader
    }

    /// Test helper: the RTPS GUID of the request writer, if the requester
    /// runs on a live runtime. Not stable, only for test inspection.
    #[doc(hidden)]
    #[must_use]
    pub fn __writer_rtps_guid(&self) -> Option<[u8; 16]> {
        self.request_writer.rtps_guid()
    }
}

impl<TIn: RequestWithHeader + Send + 'static, TOut: DdsType + Send + 'static> Requester<TIn, TOut> {
    /// A requester of a service defined by a pair of types that carry their
    /// own `header` (§7.3.1.4): `TIn` is the request type and `TOut` the
    /// reply type, each the topic type and each holding its header. The
    /// runtime puts the request's identity and the instance name into the
    /// request's header and finds the reply by the reply's header. Basic
    /// Service Mapping only; the Enhanced mapping needs no header and takes
    /// [`Self::with_options`] with `RpcMapping::Enhanced`.
    ///
    /// # Errors
    /// As [`Self::with_options`]; `RpcError::Codec` under
    /// `RpcMapping::Enhanced`.
    pub fn with_pair(
        participant: &DomainParticipant,
        name: &str,
        instance_name: &str,
        qos: &RpcQos,
        style: BindingStyle,
        topics: &AnnotatedTopics,
        options: &RpcEndpointOptions,
    ) -> RpcResult<Self> {
        if options.mapping != RpcMapping::Basic {
            return Err(RpcError::codec(
                "a pair of types with a header is the Basic mapping",
            ));
        }
        let parents: Vec<&str> = options.parents.iter().map(String::as_str).collect();
        let resolved = topics.resolve(style, name, &parents)?;
        Self::from_topics(
            participant,
            name,
            instance_name,
            qos,
            resolved,
            options,
            Some(<TIn as RequestWithHeader>::with_request_header),
        )
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
    fn request_reply_style_drops_the_interface_segment() {
        // §7.4.1: "For the request-reply style language binding,
        // <interface_name> and the following underscore shall not be
        // captured automatically."
        let p = participant(116);
        let q = RpcQos::default_basic();
        let r = Requester::<RawBytes, RawBytes>::with_instance_style(
            &p,
            "Calculator",
            "",
            &q,
            BindingStyle::RequestReply,
        )
        .unwrap();
        assert_eq!(r.request_topic_name(), "Service_Request");
        assert_eq!(r.reply_topic_name(), "Service_Reply");
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

    #[test]
    fn the_topics_carry_the_interface_type_names() {
        let p = participant(160);
        let q = RpcQos::default_basic();
        let r = Requester::<RawBytes, RawBytes>::new(&p, "Calc", &q).unwrap();
        assert_eq!(r.request_type_name(), "Calc_Request");
        assert_eq!(r.reply_type_name(), "Calc_Reply");
    }

    fn enhanced_options(service_matching: ServiceMatching) -> RpcEndpointOptions {
        RpcEndpointOptions {
            service_matching,
            ..RpcEndpointOptions::enhanced()
        }
    }

    fn enhanced_requester(
        domain: i32,
        service_matching: ServiceMatching,
    ) -> (DomainParticipant, Requester<RawBytes, RawBytes>) {
        let p = participant(domain);
        let r = Requester::<RawBytes, RawBytes>::with_options(
            &p,
            "Calc",
            "",
            &RpcQos::default_enhanced(),
            BindingStyle::FunctionCall,
            &AnnotatedTopics::none(),
            &enhanced_options(service_matching),
        )
        .unwrap();
        (p, r)
    }

    /// §7.5.1.2.4: the Enhanced request is the `Request` structure alone,
    /// with no header in front of it.
    #[test]
    fn an_enhanced_request_has_no_header() {
        let (_p, r) = enhanced_requester(161, ServiceMatching::Off);
        let (id, _rx) = r
            .send_request_async(&RawBytes::new(alloc::vec![1, 2, 3, 4]))
            .unwrap();
        let sent = r.__drain_request_writer();
        assert_eq!(sent, [alloc::vec![1, 2, 3, 4]]);
        assert_eq!(id.writer_guid, r.__writer_guid());
    }

    /// §7.8.2: the reply is correlated by its related sample identity, and
    /// it names the identity of the request.
    #[test]
    fn an_enhanced_reply_is_correlated_by_its_related_identity() {
        let (_p, r) = enhanced_requester(162, ServiceMatching::Off);
        let (id, rx) = r.send_request_async(&RawBytes::new(alloc::vec![])).unwrap();
        // A reply to another client's request is not ours.
        r.__push_reply_related(alloc::vec![9], Some(SampleIdentity::new([7; 16], 1)))
            .unwrap();
        r.tick();
        assert!(rx.try_recv().is_err());
        r.__push_reply_related(alloc::vec![4, 5], Some(id)).unwrap();
        r.tick();
        assert_eq!(rx.try_recv().unwrap(), Ok(alloc::vec![4, 5]));
        assert_eq!(r.pending_count(), 0);
    }

    /// A reply with no related identity cannot be handed to a request. It
    /// is counted, not dropped without a trace.
    #[test]
    fn an_enhanced_reply_without_a_related_identity_is_counted() {
        let (_p, r) = enhanced_requester(163, ServiceMatching::Off);
        let (_id, rx) = r.send_request_async(&RawBytes::new(alloc::vec![])).unwrap();
        r.__push_reply_related(alloc::vec![1], None).unwrap();
        r.tick();
        assert!(rx.try_recv().is_err());
        assert_eq!(r.uncorrelated_replies(), 1);
    }

    /// §7.5.1.2.5 rule 5: `remoteEx` is the member that reports a remote
    /// exception.
    #[test]
    fn an_enhanced_remote_exception_reaches_the_caller() {
        let (_p, r) = enhanced_requester(164, ServiceMatching::Off);
        let (id, rx) = r.send_request_async(&RawBytes::new(alloc::vec![])).unwrap();
        let body = crate::enhanced::encode_remote_ex_reply(
            RemoteExceptionCode::UnknownOperation,
            "remoteEx",
        );
        r.__push_reply_related(body, Some(id)).unwrap();
        r.tick();
        assert_eq!(
            rx.try_recv().unwrap(),
            Err(RemoteExceptionCode::UnknownOperation)
        );
    }

    /// §7.6.2.2.2: "attempts to send requests would be queued" until the
    /// service is discovered; here none is, so the call waits for the
    /// timeout of the QoS and then fails without sending.
    #[test]
    fn an_enhanced_request_waits_for_the_service_and_fails_without_sending() {
        let p = participant(165);
        let mut qos = RpcQos::default_enhanced();
        qos.request_timeout = Duration::from_millis(60);
        let r = Requester::<RawBytes, RawBytes>::with_options(
            &p,
            "Calc",
            "",
            &qos,
            BindingStyle::FunctionCall,
            &AnnotatedTopics::none(),
            &enhanced_options(ServiceMatching::Queue),
        )
        .unwrap();
        assert!(!r.service_matched());
        let started = std::time::Instant::now();
        let err = r
            .send_request_async(&RawBytes::new(alloc::vec![1]))
            .unwrap_err();
        assert_eq!(err, RpcError::ServiceNotMatched);
        assert!(started.elapsed() >= Duration::from_millis(60));
        assert!(r.__drain_request_writer().is_empty(), "nothing was sent");
        assert_eq!(r.pending_count(), 0);
    }

    #[test]
    fn the_fail_setting_does_not_wait() {
        let (_p, r) = enhanced_requester(166, ServiceMatching::Fail);
        let started = std::time::Instant::now();
        assert_eq!(
            r.send_oneway(&RawBytes::new(alloc::vec![1])).unwrap_err(),
            RpcError::ServiceNotMatched
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    /// The Basic mapping has no service-matching rule (§7.6.1).
    #[test]
    fn the_basic_mapping_sends_without_a_service_match() {
        let p = participant(167);
        let r = Requester::<RawBytes, RawBytes>::new(&p, "Calc", &RpcQos::default_basic()).unwrap();
        assert!(r.service_matched());
        r.send_oneway(&RawBytes::new(alloc::vec![1])).unwrap();
    }

    use crate::test_types::Wide;

    fn wide_requester(
        domain: i32,
        instance: &str,
        encoding: crate::endpoint_options::WireEncoding,
    ) -> (DomainParticipant, Requester<Wide, Wide>) {
        let p = participant(domain);
        let options = RpcEndpointOptions {
            wire_encoding: encoding,
            ..RpcEndpointOptions::default()
        };
        let r = Requester::<Wide, Wide>::with_options(
            &p,
            "Calc",
            instance,
            &RpcQos::default_basic(),
            BindingStyle::FunctionCall,
            &AnnotatedTopics::none(),
            &options,
        )
        .unwrap();
        (p, r)
    }

    /// §7.5.1.1: the Basic Service Mapping is for DDS 1.3, whose CDR
    /// aligns an 8-octet member to 8 counted from the start of the
    /// sample. With no instance name the header is 24 + 4 + 1 = 29 octets
    /// and the `Call` union begins at 32; behind its 4-octet member the
    /// `unsigned long long` is at 40 under XCDR1 (four octets of padding)
    /// and at 36 under XCDR2, which aligns to 4 at most.
    #[test]
    fn classic_cdr_aligns_the_call_union_to_the_sample() {
        use crate::endpoint_options::WireEncoding;
        let value = Wide {
            a: 0x0102_0304,
            b: 0x1112_1314_1516_1718,
        };

        let (_p, xcdr1) = wide_requester(170, "", WireEncoding::Xcdr1);
        xcdr1.send_oneway(&value).unwrap();
        let frame = xcdr1.__drain_request_writer().remove(0);
        assert_eq!(frame.len(), 32 + 4 + 4 + 8, "{frame:02x?}");
        assert_eq!(frame[32..36], 0x0102_0304u32.to_le_bytes());
        assert_eq!(frame[36..40], [0; 4], "padding up to offset 40");
        assert_eq!(frame[40..48], 0x1112_1314_1516_1718u64.to_le_bytes());

        let (_p, xcdr2) = wide_requester(171, "", WireEncoding::Xcdr2);
        xcdr2.send_oneway(&value).unwrap();
        let frame = xcdr2.__drain_request_writer().remove(0);
        assert_eq!(frame.len(), 32 + 4 + 8, "XCDR2 aligns to 4 at most");
        assert_eq!(frame[36..44], 0x1112_1314_1516_1718u64.to_le_bytes());
    }

    #[test]
    fn classic_cdr_endpoints_offer_the_xcdr1_representation_only() {
        use crate::endpoint_options::WireEncoding;
        let (_p, xcdr1) = wide_requester(172, "", WireEncoding::Xcdr1);
        assert_eq!(
            xcdr1.__request_writer_qos().data_representation,
            Some(alloc::vec![0])
        );
        let (_p, xcdr2) = wide_requester(173, "", WireEncoding::Xcdr2);
        assert_eq!(
            xcdr2.__request_writer_qos().data_representation,
            Some(alloc::vec![2])
        );
        // The default is the spec's: classic CDR.
        let p = participant(174);
        let default =
            Requester::<RawBytes, RawBytes>::new(&p, "Calc", &RpcQos::default_basic()).unwrap();
        assert_eq!(
            default.__request_writer_qos().data_representation,
            Some(alloc::vec![0])
        );
    }

    /// The reply's `Return` union begins at 28, behind the `ReplyHeader`.
    /// Read where it stands, the `unsigned long long` is at 32 under XCDR1;
    /// the caller then decodes the standalone form.
    #[test]
    fn a_classic_cdr_reply_is_read_where_it_stands() {
        use crate::endpoint_options::WireEncoding;
        let (_p, r) = wide_requester(175, "", WireEncoding::Xcdr1);
        let (id, rx) = r.send_request_async(&Wide { a: 1, b: 2 }).unwrap();
        let mut frame = crate::wire_codec::encode_reply_frame(
            &crate::common_types::ReplyHeader::new(id, RemoteExceptionCode::Ok),
            &[],
        );
        frame.extend_from_slice(&7u32.to_le_bytes());
        frame.extend_from_slice(&[0; 0]);
        // 28 + 4 = 32, which is a multiple of 8.
        frame.extend_from_slice(&0x0A0B_0C0D_0E0F_1011u64.to_le_bytes());
        r.__push_reply_raw(frame).unwrap();
        r.tick();
        let bytes = rx.try_recv().unwrap().unwrap();
        assert_eq!(
            Wide::decode(&bytes).unwrap(),
            Wide {
                a: 7,
                b: 0x0A0B_0C0D_0E0F_1011
            },
            "the standalone form"
        );
    }

    /// §7.3.1.4: a request type with its own `header` is the topic type;
    /// the runtime puts the identity and the instance name in that header
    /// and writes the structure as it is.
    #[test]
    fn a_request_type_with_its_own_header_carries_the_identity_in_it() {
        use crate::test_types::{HeaderReply, HeaderRequest};
        use crate::wire_codec::decode_request_frame;
        let p = participant(180);
        let r = Requester::<HeaderRequest, HeaderReply>::with_pair(
            &p,
            "Calc",
            "abcd",
            &RpcQos::default_basic(),
            BindingStyle::RequestReply,
            &AnnotatedTopics::none(),
            &RpcEndpointOptions::default(),
        )
        .unwrap();
        let (id, _rx) = r
            .send_request_async(&HeaderRequest::new(7, 0x1112_1314_1516_1718))
            .unwrap();
        let frame = r.__drain_request_writer().remove(0);
        let (header, _) = decode_request_frame(&frame).unwrap();
        assert_eq!(header.request_id, id);
        assert_eq!(header.instance_name, "abcd");
        // 24 + 4 + 5 = 33 octets of header, the structure's next member at
        // 36 and its 8-octet member at 40 (classic CDR).
        assert_eq!(frame[36..40], 7u32.to_le_bytes());
        assert_eq!(frame[40..48], 0x1112_1314_1516_1718u64.to_le_bytes());
        assert_eq!(frame.len(), 48);
    }

    #[test]
    fn the_reply_of_a_type_with_its_own_header_is_found_by_that_header() {
        use crate::header_pair::ReplyWithHeader;
        use crate::test_types::{HeaderReply, HeaderRequest};
        let p = participant(181);
        let r = Requester::<HeaderRequest, HeaderReply>::with_pair(
            &p,
            "Calc",
            "",
            &RpcQos::default_basic(),
            BindingStyle::RequestReply,
            &AnnotatedTopics::none(),
            &RpcEndpointOptions::default(),
        )
        .unwrap();
        let (id, rx) = r.send_request_async(&HeaderRequest::new(1, 2)).unwrap();
        let reply = HeaderReply {
            header: crate::common_types::ReplyHeader::new(id, RemoteExceptionCode::Ok),
            r: 99,
        };
        let mut frame = Vec::new();
        reply
            .with_reply_header(&reply.header)
            .unwrap()
            .encode_continuing(&mut frame, true)
            .unwrap();
        r.__push_reply_raw(frame).unwrap();
        r.tick();
        let bytes = rx.try_recv().unwrap().unwrap();
        assert_eq!(
            HeaderReply::decode(&bytes).unwrap().r,
            99,
            "the standalone form of the whole reply"
        );
    }

    #[test]
    fn a_pair_of_types_with_a_header_is_the_basic_mapping() {
        use crate::test_types::{HeaderReply, HeaderRequest};
        let p = participant(182);
        let err = Requester::<HeaderRequest, HeaderReply>::with_pair(
            &p,
            "Calc",
            "",
            &RpcQos::default_basic(),
            BindingStyle::RequestReply,
            &AnnotatedTopics::none(),
            &RpcEndpointOptions::enhanced(),
        )
        .unwrap_err();
        assert!(matches!(err, RpcError::Codec(_)));
    }
}
