// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! [`Replier`] — server side of a DDS-RPC service (Spec §7.2.1, §7.9, §7.11.1.4.5).
//!
//! Threading model: mirrors [`crate::requester::Requester`]. The caller
//! orchestrates the polling itself — no dedicated thread.
//!
//! §7.9.2.2 names three mutually exclusive ways to process requests, and
//! all three are here:
//!
//! 1. [`ReplierHandler`] — the synchronous listener, called by
//!    [`Replier::tick`], which returns the reply.
//! 2. [`AsyncReplierHandler`] — the asynchronous listener, called by
//!    [`Replier::dispatch_async`], which receives the replier and
//!    answers through the Replier API.
//! 3. [`Replier::receive_requests`] with [`Replier::send_reply`] /
//!    [`Replier::send_exception`] — the polling API.
//!
//! In all three the request identity of §7.8.2.1 is available: ways 2
//! and 3 carry it as [`RequestIdentity`], and the reply is correlated by
//! it.
//!
//! Under the Basic mapping `related_request_id` travels in the
//! [`ReplyHeader`] (§7.8.1). Under the Enhanced mapping the request is
//! identified by the writer GUID and sequence number of its DATA and the
//! reply carries them in `PID_RELATED_SAMPLE_IDENTITY` (§7.8.2); there is
//! no header, a remote exception is the `remoteEx` member of the reply
//! structure, and a request from a client the service has not discovered
//! completely is held back until it has (§7.6.2.2.1).

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::marker::PhantomData;

use zerodds_dcps::dds_type::{DdsType, RawBytes};
use zerodds_dcps::entity::Entity;
use zerodds_dcps::listener::DataReaderListener;
use zerodds_dcps::participant::DomainParticipant;
use zerodds_dcps::psm_constants::status;
use zerodds_dcps::publisher::DataWriter;
use zerodds_dcps::qos::{PublisherQos, SubscriberQos};
use zerodds_dcps::runtime::RpcDiscoveryExt;
use zerodds_dcps::sample_bytes::SampleWire;
use zerodds_dcps::subscriber::DataReader;

use crate::common_types::{RemoteExceptionCode, ReplyHeader, SampleIdentity};
use crate::discovery_ext::client_matching_complete;
use crate::endpoint_options::{ClientMatching, RpcEndpointOptions, RpcMapping};
use crate::enhanced::encode_remote_ex_reply;
use crate::error::{RpcError, RpcResult};
use crate::header_pair::ReplyWithHeader;
use crate::qos_profile::RpcQos;
use crate::request_identity::RequestIdentity;
use crate::requester::{InstanceClaim, InstanceRole, try_claim_instance};
use crate::topic_naming::{AnnotatedTopics, BindingStyle, ServiceTopicNames};
use crate::wire_codec::{ERROR_REPLY_DATA, decode_request_frame, encode_reply_frame};

// ---------------------------------------------------------------------
// ReplierHandler
// ---------------------------------------------------------------------

/// User hook that handles a single request — §7.9.2.2 way 1, the
/// synchronous listener. The reply is the return value.
///
/// The asynchronous listener is [`AsyncReplierHandler`] and the polling
/// API is [`Replier::receive_requests`] with [`Replier::send_reply`].
pub trait ReplierHandler<TIn, TOut>: Send + Sync {
    /// Handles a request. `Ok(reply)` ⇒ successful reply with
    /// `RemoteExceptionCode::Ok`. `Err(code)` ⇒ reply with the
    /// given exception code and an empty payload.
    fn handle(&self, request: TIn) -> Result<TOut, RemoteExceptionCode>;
}

/// Convenience adapter that makes an `Fn(TIn) -> Result<TOut, RemoteExceptionCode>`
/// closure available as a [`ReplierHandler`].
pub struct FnHandler<F, TIn, TOut>
where
    F: Fn(TIn) -> Result<TOut, RemoteExceptionCode> + Send + Sync,
{
    f: F,
    _phantom: PhantomData<fn() -> (TIn, TOut)>,
}

impl<F, TIn, TOut> FnHandler<F, TIn, TOut>
where
    F: Fn(TIn) -> Result<TOut, RemoteExceptionCode> + Send + Sync,
{
    /// Wraps a closure.
    pub fn new(f: F) -> Self {
        Self {
            f,
            _phantom: PhantomData,
        }
    }
}

impl<F, TIn, TOut> ReplierHandler<TIn, TOut> for FnHandler<F, TIn, TOut>
where
    F: Fn(TIn) -> Result<TOut, RemoteExceptionCode> + Send + Sync,
{
    fn handle(&self, request: TIn) -> Result<TOut, RemoteExceptionCode> {
        (self.f)(request)
    }
}

/// §7.9.2.2 way 2: an asynchronous service listener.
///
/// The callback does not return the reply. It receives a handle to the
/// replier and the request's identity (§7.8.2.1), and answers through
/// [`Replier::send_reply`] or [`Replier::send_exception`], which is what
/// decouples request reception from request processing.
pub trait AsyncReplierHandler<TIn: DdsType, TOut: DdsType>: Send + Sync {
    /// Handle one request.
    fn on_request(&self, replier: &Replier<TIn, TOut>, request: TIn, identity: RequestIdentity);
}

/// A request taken from the wire by [`Replier::receive_requests`], with
/// the identity §7.8.2.1 lets the service retrieve and answer by.
#[derive(Debug, Clone)]
pub struct PendingRequest<TIn> {
    /// The decoded request payload.
    pub request: TIn,
    /// The identity of the request sample (§7.8.2.1).
    pub identity: RequestIdentity,
}

impl<TIn: DdsType + Send + 'static, TOut: ReplyWithHeader + Send + 'static> Replier<TIn, TOut> {
    /// A replier of a service defined by a pair of types that carry their
    /// own `header` (§7.3.1.4); see
    /// [`crate::requester::Requester::with_pair`]. The reply the handler
    /// returns is given its header (the request it answers and
    /// `REMOTE_EX_OK`); a reply that reports a remote exception is the
    /// type's default with that header.
    ///
    /// # Errors
    /// As [`Self::with_options`]; `RpcError::Codec` under
    /// `RpcMapping::Enhanced`.
    #[allow(clippy::too_many_arguments)]
    pub fn with_pair(
        participant: &DomainParticipant,
        name: &str,
        instance_name: &str,
        qos: &RpcQos,
        style: BindingStyle,
        topics: &AnnotatedTopics,
        options: &RpcEndpointOptions,
        handler: Arc<dyn ReplierHandler<TIn, TOut>>,
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
            Some(PairReply {
                stamp: <TOut as ReplyWithHeader>::with_reply_header,
                error: |header| TOut::default().with_reply_header(header),
            }),
            handler,
        )
    }
}

/// The reply writer's QoS: the RPC QoS and, for classic-CDR Basic
/// samples, the XCDR1 data representation (§7.5.1.1: DDS 1.3).
fn writer_qos(qos: &RpcQos, options: &RpcEndpointOptions) -> zerodds_dcps::qos::DataWriterQos {
    let mut writer = qos.reply_writer_qos();
    // The samples of the Enhanced mapping are XCDR2 (§7.5.1.2), those of the Basic
    // mapping the encoding the options name; the writer offers the one it writes.
    writer.data_representation = if options.xcdr1() {
        crate::endpoint_options::WireEncoding::Xcdr1.representation()
    } else {
        crate::endpoint_options::WireEncoding::Xcdr2.representation()
    };
    writer
}

/// A data-available listener that calls a closure: the arrival of a
/// request is the event §7.9.2.2's listeners are called on.
struct ArrivalListener<F: Fn() + Send + Sync>(F);

impl<F: Fn() + Send + Sync> DataReaderListener for ArrivalListener<F> {
    fn on_data_available(&self, _reader: zerodds_dcps::instance_handle::InstanceHandle) {
        (self.0)();
    }
}

// ---------------------------------------------------------------------
// Replier
// ---------------------------------------------------------------------

/// Server side of a DDS-RPC service.
pub struct Replier<TIn: DdsType, TOut: DdsType> {
    service_name: String,
    instance_name: String,
    request_reader: DataReader<RawBytes>,
    reply_writer: DataWriter<RawBytes>,
    handler: Arc<dyn ReplierHandler<TIn, TOut>>,
    handled_count: std::sync::atomic::AtomicU64,
    error_count: std::sync::atomic::AtomicU64,
    options: RpcEndpointOptions,
    /// For a service defined by a pair of types with their own header
    /// (§7.3.1.4): the reply with its header set to the reply to a request,
    /// and the reply that reports a remote exception.
    pair: Option<PairReply<TOut>>,
    /// Enhanced requests of clients the service has not discovered
    /// completely, oldest first.
    held: std::sync::Mutex<alloc::collections::VecDeque<RawRequest>>,
    /// Requests dropped because the held queue was full.
    held_overflow: std::sync::atomic::AtomicU64,
    /// Requests that carried no identity to answer to.
    unidentified_requests: std::sync::atomic::AtomicU64,
    /// Replies the writer refused.
    reply_write_failures: std::sync::atomic::AtomicU64,
    /// Whether the listener of [`Self::activate`] or [`Self::activate_async`]
    /// takes the requests.
    listening: std::sync::atomic::AtomicBool,
    /// Whether the thread of [`Self::watch_held`] runs.
    watching_held: std::sync::atomic::AtomicBool,
    _claim: InstanceClaim,
    _phantom: PhantomData<fn() -> (TIn, TOut)>,
}

/// How a reply type with its own header is given its header.
struct PairReply<TOut> {
    stamp: fn(&TOut, &ReplyHeader) -> RpcResult<TOut>,
    /// The type's default with the header set.
    error: fn(&ReplyHeader) -> RpcResult<TOut>,
}

impl<TOut> Clone for PairReply<TOut> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<TOut> Copy for PairReply<TOut> {}

/// A request taken from the reader, before its payload is decoded.
#[derive(Debug, Clone)]
struct RawRequest {
    identity: SampleIdentity,
    /// The whole sample.
    frame: Vec<u8>,
    /// Where the `Call` union begins in it (Basic: behind the header and
    /// its padding), or `0` for the `Request` structure (Enhanced).
    offset: usize,
}

/// How many Enhanced requests the service holds for clients it has not
/// discovered completely. Beyond it a request is dropped and counted in
/// [`Replier::held_overflow`].
const HELD_REQUESTS_MAX: usize = 4096;

/// How often a replier called on arrival looks at the requests it holds
/// again; see [`Replier::watch_held`].
const HELD_RECHECK: core::time::Duration = core::time::Duration::from_millis(5);

impl<TIn: DdsType, TOut: DdsType> core::fmt::Debug for Replier<TIn, TOut> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Replier")
            .field("service", &self.service_name)
            .field("instance", &self.instance_name)
            .finish_non_exhaustive()
    }
}

impl<TIn: DdsType + Send + 'static, TOut: DdsType + Send + 'static> Replier<TIn, TOut> {
    /// Creates a new replier against `service_name`.
    ///
    /// # Errors
    /// See [`crate::requester::Requester::new`].
    pub fn new(
        participant: &DomainParticipant,
        service_name: &str,
        qos: &RpcQos,
        handler: Arc<dyn ReplierHandler<TIn, TOut>>,
    ) -> RpcResult<Self> {
        Self::with_instance(participant, service_name, "", qos, handler)
    }

    /// Like [`Self::new`], with an explicit instance name.
    ///
    /// # Errors
    /// See [`crate::requester::Requester::with_instance`].
    pub fn with_instance(
        participant: &DomainParticipant,
        service_name: &str,
        instance_name: &str,
        qos: &RpcQos,
        handler: Arc<dyn ReplierHandler<TIn, TOut>>,
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
            handler,
        )
    }

    /// Like [`Self::with_instance`] under an explicit binding style
    /// (§7.2.2, §7.4.1).
    ///
    /// # Errors
    /// See [`crate::requester::Requester::with_instance`].
    pub fn with_instance_style(
        participant: &DomainParticipant,
        service_name: &str,
        instance_name: &str,
        qos: &RpcQos,
        style: BindingStyle,
        handler: Arc<dyn ReplierHandler<TIn, TOut>>,
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
            handler,
        )
    }

    /// Like [`Self::with_instance`], but composing the topic pair from
    /// DDS-RPC 1.0 §7.4's full rule instead of always defaulting to
    /// `<interface>_Service_Request`/`_Reply`: `topics` carries the
    /// interface's own `@service`/`@DDSRequestTopic`/`@DDSReplyTopic`
    /// annotation values (or [`AnnotatedTopics::none`] where it declares
    /// none) and, on top of them, any run-time override §7.4.2.3 admits.
    /// §7.10 defines the `Replier` over exactly these inputs a service
    /// declares — a generated dispatcher that parses its own IDL's
    /// annotations reaches this constructor instead of always the
    /// default one.
    ///
    /// # Errors
    /// See [`crate::requester::Requester::with_instance`].
    pub fn with_topics(
        participant: &DomainParticipant,
        qualified_interface: &str,
        instance_name: &str,
        qos: &RpcQos,
        topics: &AnnotatedTopics,
        handler: Arc<dyn ReplierHandler<TIn, TOut>>,
    ) -> RpcResult<Self> {
        Self::with_topics_style(
            participant,
            qualified_interface,
            instance_name,
            qos,
            BindingStyle::FunctionCall,
            topics,
            handler,
        )
    }

    /// Like [`Self::with_topics`] under an explicit binding style
    /// (§7.2.2, §7.4.1).
    ///
    /// # Errors
    /// See [`crate::requester::Requester::with_instance`].
    pub fn with_topics_style(
        participant: &DomainParticipant,
        qualified_interface: &str,
        instance_name: &str,
        qos: &RpcQos,
        style: BindingStyle,
        topics: &AnnotatedTopics,
        handler: Arc<dyn ReplierHandler<TIn, TOut>>,
    ) -> RpcResult<Self> {
        Self::with_options(
            participant,
            qualified_interface,
            instance_name,
            qos,
            style,
            topics,
            &RpcEndpointOptions::default(),
            handler,
        )
    }

    /// The general constructor: [`Self::with_topics_style`] with the
    /// service mapping, the DDS type names and the discovery rules of
    /// `options` (see [`RpcEndpointOptions`]).
    ///
    /// Under [`RpcMapping::Enhanced`] the endpoints announce the
    /// discovery extension of §7.6.2.1 — the instance name on both,
    /// `related_datareader_key` / `related_datawriter_key` naming each
    /// other, and the topic aliases of `options.parents` — and a request
    /// of a client whose endpoints the service has not discovered is held
    /// back (§7.6.2.2.1).
    ///
    /// # Errors
    /// See [`crate::requester::Requester::with_instance`].
    #[allow(clippy::too_many_arguments)]
    pub fn with_options(
        participant: &DomainParticipant,
        qualified_interface: &str,
        instance_name: &str,
        qos: &RpcQos,
        style: BindingStyle,
        topics: &AnnotatedTopics,
        options: &RpcEndpointOptions,
        handler: Arc<dyn ReplierHandler<TIn, TOut>>,
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
            handler,
        )
    }

    /// Shared tail of [`Self::with_instance`] and [`Self::with_topics`]:
    /// everything after the topic pair is already resolved.
    #[allow(clippy::too_many_arguments)]
    fn from_topics(
        participant: &DomainParticipant,
        service_name: &str,
        instance_name: &str,
        qos: &RpcQos,
        topics: ServiceTopicNames,
        options: &RpcEndpointOptions,
        pair: Option<PairReply<TOut>>,
        handler: Arc<dyn ReplierHandler<TIn, TOut>>,
    ) -> RpcResult<Self> {
        let claim = try_claim_instance(
            participant,
            InstanceRole::Replier,
            service_name,
            instance_name,
        )?;
        // §7.5.1.1.6/7: `${I}_Request` and `${I}_Reply`.
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
        let request_reader = subscriber
            .create_datareader::<RawBytes>(&request_topic, qos.request_reader_qos())
            .map_err(|e| RpcError::Dcps(alloc::format!("create_datareader: {e:?}")))?;
        let reply_writer = publisher
            .create_datawriter::<RawBytes>(&reply_topic, writer_qos(qos, options))
            .map_err(|e| RpcError::Dcps(alloc::format!("create_datawriter: {e:?}")))?;
        if options.mapping == RpcMapping::Enhanced {
            // §7.6.2.1: the service-side endpoints name each other, carry
            // the instance name "specified by the user at the service
            // side" (none if none) and the topic aliases.
            let name = (!instance_name.is_empty()).then(|| instance_name.to_string());
            let aliases = |list: &[String]| (!list.is_empty()).then(|| list.to_vec());
            if let Some(request_reader_guid) = request_reader.rtps_guid() {
                reply_writer
                    .set_rpc_discovery(&RpcDiscoveryExt {
                        service_instance_name: name.clone(),
                        related_entity_guid: Some(request_reader_guid),
                        topic_aliases: aliases(&topics.reply_aliases),
                    })
                    .map_err(|e| RpcError::Dcps(alloc::format!("announce reply writer: {e:?}")))?;
            }
            if let Some(reply_writer_guid) = reply_writer.rtps_guid() {
                request_reader
                    .set_rpc_discovery(&RpcDiscoveryExt {
                        service_instance_name: name,
                        related_entity_guid: Some(reply_writer_guid),
                        topic_aliases: aliases(&topics.request_aliases),
                    })
                    .map_err(|e| {
                        RpcError::Dcps(alloc::format!("announce request reader: {e:?}"))
                    })?;
            }
        }
        Ok(Self {
            service_name: service_name.into(),
            instance_name: instance_name.into(),
            request_reader,
            reply_writer,
            handler,
            handled_count: std::sync::atomic::AtomicU64::new(0),
            error_count: std::sync::atomic::AtomicU64::new(0),
            options: options.clone(),
            pair,
            held: std::sync::Mutex::new(alloc::collections::VecDeque::new()),
            held_overflow: std::sync::atomic::AtomicU64::new(0),
            unidentified_requests: std::sync::atomic::AtomicU64::new(0),
            reply_write_failures: std::sync::atomic::AtomicU64::new(0),
            listening: std::sync::atomic::AtomicBool::new(false),
            watching_held: std::sync::atomic::AtomicBool::new(false),
            _claim: claim,
            _phantom: PhantomData,
        })
    }

    /// Service-Name.
    #[must_use]
    pub fn service_name(&self) -> &str {
        &self.service_name
    }

    /// Service-Instance-Name.
    #[must_use]
    pub fn instance_name(&self) -> &str {
        &self.instance_name
    }

    /// The request topic this replier actually reads from — §7.4.1's
    /// name, after annotations and run-time overrides
    /// ([`Self::with_topics`]) have been applied. Not necessarily
    /// `<service>_Service_Request`.
    #[must_use]
    pub fn request_topic_name(&self) -> &str {
        self.request_reader.topic().name()
    }

    /// The reply topic this replier actually writes to — the other half
    /// of [`Self::request_topic_name`].
    #[must_use]
    pub fn reply_topic_name(&self) -> &str {
        self.reply_writer.topic().name()
    }

    /// The DDS type name of the request topic: `${interfaceName}_Request`
    /// (§7.5.1.1.6 rule 1).
    #[must_use]
    pub fn request_type_name(&self) -> &str {
        self.request_reader.topic().type_name()
    }

    /// The DDS type name of the reply topic: `${interfaceName}_Reply`
    /// (§7.5.1.1.7 rule 1).
    #[must_use]
    pub fn reply_type_name(&self) -> &str {
        self.reply_writer.topic().type_name()
    }

    /// Number of requests successfully processed so far.
    #[must_use]
    pub fn handled_count(&self) -> u64 {
        self.handled_count
            .load(std::sync::atomic::Ordering::Acquire)
    }

    /// Number of requests the handler acknowledged with `Err(_)`.
    #[must_use]
    pub fn error_count(&self) -> u64 {
        self.error_count.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Change the mutable QoS policies of the request reader and reply
    /// writer at run time — §7.10.2: "The users may modify the mutable
    /// QoS policies at run-time."
    ///
    /// # Errors
    /// `RpcError::Dcps` when the middleware refuses a policy — an
    /// immutable one changed after `enable()`, or an inconsistent
    /// combination.
    pub fn set_qos(&self, qos: &RpcQos) -> RpcResult<()> {
        self.request_reader
            .set_qos(qos.request_reader_qos())
            .map_err(|e| RpcError::Dcps(alloc::format!("set request reader qos: {e:?}")))?;
        self.reply_writer
            .set_qos(writer_qos(qos, &self.options))
            .map_err(|e| RpcError::Dcps(alloc::format!("set reply writer qos: {e:?}")))?;
        Ok(())
    }

    /// Test seam: the request reader's current QoS.
    #[doc(hidden)]
    #[must_use]
    pub fn __request_reader_qos(&self) -> zerodds_dcps::qos::DataReaderQos {
        self.request_reader.qos()
    }

    /// The request value of a sample: the `Call` union read where it
    /// stands in the sample, so that classic CDR aligns its 8-octet
    /// members to the sample (§7.5.1.1).
    fn decode_request(&self, raw: &RawRequest) -> Result<TIn, zerodds_dcps::dds_type::DecodeError> {
        TIn::decode_continuing(&raw.frame, raw.offset, self.options.xcdr1())
    }

    /// Whether the reply writer is matched with a reader: a reply written
    /// before that has no one to go to (the discovery race of §7.6.1).
    #[must_use]
    pub fn reply_writer_matched(&self) -> bool {
        self.reply_writer.matched_subscription_count() > 0
    }

    /// §7.9.2.2 way 1, on arrival: the synchronous listener this replier
    /// was created with is called as each request arrives, without the
    /// application calling [`Self::tick`]. The replier is the one the
    /// reader's listener refers to weakly: dropping the last `Arc` ends
    /// the listening. A request that arrived before is processed at once.
    pub fn activate(self: &Arc<Self>)
    where
        TIn: 'static,
        TOut: 'static,
    {
        let weak = Arc::downgrade(self);
        let pass: Arc<dyn Fn(&Self) + Send + Sync> = Arc::new(|replier: &Self| {
            replier.tick();
        });
        self.listen(move || {
            if let Some(replier) = weak.upgrade() {
                replier.tick();
                replier.watch_held(Arc::clone(&pass));
            }
        })
    }

    /// §7.9.2.2 way 2, on arrival: `handler` is called as each request
    /// arrives and answers through the replier it receives. A request that
    /// arrived before is handed over at once.
    pub fn activate_async(self: &Arc<Self>, handler: Arc<dyn AsyncReplierHandler<TIn, TOut>>)
    where
        TIn: 'static,
        TOut: 'static,
    {
        let weak = Arc::downgrade(self);
        let on_held = Arc::clone(&handler);
        let pass: Arc<dyn Fn(&Self) + Send + Sync> = Arc::new(move |replier: &Self| {
            replier.dispatch_async(on_held.as_ref());
        });
        self.listen(move || {
            if let Some(replier) = weak.upgrade() {
                replier.dispatch_async(handler.as_ref());
                replier.watch_held(Arc::clone(&pass));
            }
        })
    }

    /// §7.6.2.2.1, Figure 7.3: "When the service-side DataWriter completes
    /// the discovery of a client DataReader, the service DataReader is
    /// ready to accept requests from that specific client." The discovery
    /// that completes it delivers no request, so the listener of the
    /// request reader alone would keep the requests held for that client.
    /// While some are held, this thread runs `pass` every
    /// [`HELD_RECHECK`]; it ends when none is held, the listening stopped,
    /// or the replier is gone. A replier driven by [`Self::tick`] looks at
    /// them at every tick.
    fn watch_held(self: &Arc<Self>, pass: Arc<dyn Fn(&Self) + Send + Sync>)
    where
        TIn: 'static,
        TOut: 'static,
    {
        use std::sync::atomic::Ordering;
        if self.held_requests() == 0 || self.watching_held.swap(true, Ordering::AcqRel) {
            return;
        }
        let weak = Arc::downgrade(self);
        let spawned = std::thread::Builder::new()
            .name(alloc::format!("zerodds-rpc-held-{}", self.service_name))
            .spawn(move || {
                loop {
                    std::thread::sleep(HELD_RECHECK);
                    let Some(replier) = weak.upgrade() else {
                        return;
                    };
                    if !replier.listening.load(Ordering::Acquire) {
                        replier.watching_held.store(false, Ordering::Release);
                        return;
                    }
                    if replier.held_requests() == 0 {
                        replier.watching_held.store(false, Ordering::Release);
                        // A request held after the look and before the
                        // store would find the flag still set: look again.
                        if replier.held_requests() == 0
                            || replier.watching_held.swap(true, Ordering::AcqRel)
                        {
                            return;
                        }
                        continue;
                    }
                    pass(&replier);
                }
            });
        if spawned.is_err() {
            // No thread: the held requests wait for the next arrival, as
            // before this watch existed.
            self.watching_held.store(false, Ordering::Release);
        }
    }

    /// Stops the listening [`Self::activate`] or [`Self::activate_async`]
    /// began; requests are then taken by [`Self::tick`] and
    /// [`Self::receive_requests`] again.
    pub fn deactivate_listener(&self) {
        self.listening
            .store(false, std::sync::atomic::Ordering::Release);
        self.request_reader
            .set_listener(None, status::DATA_AVAILABLE);
    }

    fn listen(&self, on_data: impl Fn() + Send + Sync + 'static) {
        self.listening
            .store(true, std::sync::atomic::Ordering::Release);
        let listener = Arc::new(ArrivalListener(on_data));
        self.request_reader
            .set_listener(Some(listener.clone()), status::DATA_AVAILABLE);
        // A request that arrived before the listener was set is waiting.
        (listener.0)();
    }

    /// §7.9.3: deactivates the service. Deactivation "shall delete the
    /// underlying DDS entities (i.e., DataReader and DataWriter only)":
    /// the request reader and the reply writer are deleted, the writer
    /// announcing its end to the peers, and the instance name is free
    /// again.
    pub fn close(self) {
        drop(self);
    }

    /// Takes the requests that are ready to be processed: the fresh ones
    /// and, under the Enhanced mapping, the held ones whose client the
    /// service has discovered completely by now.
    fn take_requests(&self) -> Vec<RawRequest> {
        let samples = self.request_reader.take_with_wire().unwrap_or_default();
        match self.options.mapping {
            RpcMapping::Basic => samples
                .into_iter()
                .filter_map(|(raw, _)| {
                    let Ok((header, payload)) = decode_request_frame(&raw.data) else {
                        // No header, no identity to answer to: counted.
                        self.unidentified_requests
                            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                        return None;
                    };
                    // Instance binding (Spec §7.6.2, §7.11.1.4.3): a
                    // replier answers only requests bound to its own
                    // instance. The empty name is the default instance and
                    // matches only itself; a named replier does not answer
                    // a request that names no instance, and vice versa.
                    (header.instance_name == self.instance_name).then(|| RawRequest {
                        identity: header.request_id,
                        // A type with its own header is the whole sample.
                        offset: if self.pair.is_some() {
                            0
                        } else {
                            raw.data.len() - payload.len()
                        },
                        frame: raw.data.clone(),
                    })
                })
                .collect(),
            RpcMapping::Enhanced => self.take_enhanced_requests(samples),
        }
    }

    /// The Enhanced request path: identity from the DATA (§7.8.2), the
    /// client matching rule of §7.6.2.2.1.
    fn take_enhanced_requests(
        &self,
        samples: Vec<(RawBytes, Option<SampleWire>)>,
    ) -> Vec<RawRequest> {
        let Ok(mut held) = self.held.lock() else {
            return Vec::new();
        };
        for (raw, wire) in samples {
            let identity = wire.and_then(|w| {
                u64::try_from(w.sequence_number)
                    .ok()
                    .map(|sn| SampleIdentity::new(w.writer_guid, sn))
            });
            match identity {
                Some(identity) => held.push_back(RawRequest {
                    identity,
                    frame: raw.data,
                    offset: 0,
                }),
                None => {
                    self.unidentified_requests
                        .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                }
            }
        }
        if held.is_empty() {
            return Vec::new();
        }
        let ready_all = self.options.client_matching == ClientMatching::Off;
        let writers = self.request_reader.matched_rpc_writers();
        let readers = self.reply_writer.matched_rpc_readers();
        let accept = self.options.accept_unannounced;
        let mut ready = Vec::new();
        let mut still_held = alloc::collections::VecDeque::new();
        for request in held.drain(..) {
            let complete = ready_all
                || match writers
                    .iter()
                    .find(|peer| peer.key == request.identity.writer_guid)
                {
                    Some(writer) => client_matching_complete(writer, &readers, accept),
                    None => accept,
                };
            if complete {
                ready.push(request);
            } else {
                still_held.push_back(request);
            }
        }
        if still_held.len() > HELD_REQUESTS_MAX {
            let dropped = still_held.len() - HELD_REQUESTS_MAX;
            still_held.truncate(HELD_REQUESTS_MAX);
            self.held_overflow
                .fetch_add(dropped as u64, std::sync::atomic::Ordering::AcqRel);
        }
        *held = still_held;
        ready
    }

    /// Number of Enhanced requests held back because their client is not
    /// discovered completely (§7.6.2.2.1).
    #[must_use]
    pub fn held_requests(&self) -> usize {
        self.held.lock().map(|held| held.len()).unwrap_or(0)
    }

    /// Number of held requests dropped because more than the service can
    /// hold waited for their client.
    #[must_use]
    pub fn held_overflow(&self) -> u64 {
        self.held_overflow
            .load(std::sync::atomic::Ordering::Acquire)
    }

    /// Number of requests that carried no identity to answer to — under
    /// the Basic mapping a sample whose header does not decode, under the
    /// Enhanced mapping a DATA without writer identity.
    #[must_use]
    pub fn unidentified_requests(&self) -> u64 {
        self.unidentified_requests
            .load(std::sync::atomic::Ordering::Acquire)
    }

    /// §7.9.2.2 way 3: take the requests without answering them, each
    /// with the identity of §7.8.2.1 so the application can answer
    /// through [`Self::send_reply`] or [`Self::send_exception`].
    ///
    /// A request bound to another instance is skipped (§7.11.1.4.3).
    /// A request whose payload does not decode is not handed out: it is
    /// answered with `REMOTE_EX_INVALID_ARGUMENT` (§7.5.2), as
    /// [`Self::tick`] answers it, since the polling caller has no request
    /// to answer.
    #[must_use]
    pub fn receive_requests(&self) -> Vec<PendingRequest<TIn>> {
        let mut out = Vec::new();
        for raw in self.take_requests() {
            let Ok(request) = self.decode_request(&raw) else {
                self.send_error_reply(raw.identity, RemoteExceptionCode::InvalidArgument);
                continue;
            };
            out.push(PendingRequest {
                request,
                identity: match self.options.mapping {
                    RpcMapping::Basic => RequestIdentity::from_header(raw.identity),
                    RpcMapping::Enhanced => RequestIdentity::from_inline_qos(raw.identity),
                },
            });
        }
        out
    }

    /// §7.9.2.2 way 3: answer one request taken by
    /// [`Self::receive_requests`] with a successful reply.
    ///
    /// # Errors
    /// `RpcError::Dcps` if the reply cannot be encoded or written.
    pub fn send_reply(&self, identity: &RequestIdentity, reply: TOut) -> RpcResult<()> {
        self.write_reply(
            &identity.sample_identity,
            RemoteExceptionCode::Ok,
            Some(&reply),
        )?;
        self.handled_count
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Ok(())
    }

    /// §7.9.2.2 way 3: answer one request with a remote exception code.
    ///
    /// # Errors
    /// `RpcError::Dcps` if the reply cannot be written.
    pub fn send_exception(
        &self,
        identity: &RequestIdentity,
        code: RemoteExceptionCode,
    ) -> RpcResult<()> {
        self.write_reply(&identity.sample_identity, code, None)?;
        self.error_count
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Ok(())
    }

    /// §7.9.2.2 way 2: hand every pending request to an asynchronous
    /// listener. Returns how many were handed out.
    pub fn dispatch_async(&self, handler: &dyn AsyncReplierHandler<TIn, TOut>) -> usize {
        let pending = self.receive_requests();
        let count = pending.len();
        for entry in pending {
            handler.on_request(self, entry.request, entry.identity);
        }
        count
    }

    /// §7.9.2.2 way 1: reads pending requests, calls the synchronous
    /// handler, writes the reply back. Returns the number of requests
    /// processed in this tick.
    pub fn tick(&self) -> usize {
        let mut processed = 0;
        for raw in self.take_requests() {
            let request_id = raw.identity;
            // Decode user payload.
            let req = match self.decode_request(&raw) {
                Ok(v) => v,
                Err(_) => {
                    self.send_error_reply(request_id, RemoteExceptionCode::InvalidArgument);
                    continue;
                }
            };
            match self.handler.handle(req) {
                Ok(reply) => {
                    match self.write_reply(&request_id, RemoteExceptionCode::Ok, Some(&reply)) {
                        Ok(()) => {}
                        Err(RpcError::Codec(_)) => {
                            // The service could not produce the reply its
                            // own type declares. §7.5.2 reserves
                            // REMOTE_EX_OUT_OF_RESOURCES for resource
                            // exhaustion and REMOTE_EX_UNKNOWN_EXCEPTION for
                            // a generic service-side failure; the latter
                            // fits.
                            self.send_error_reply(
                                request_id,
                                RemoteExceptionCode::UnknownException,
                            );
                            continue;
                        }
                        Err(_) => {
                            // The reply could not be sent: the handler was
                            // ok but the reply was lost, which is not a
                            // handler error; it is counted where the caller
                            // can see it.
                            self.reply_write_failures
                                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                            continue;
                        }
                    }
                    self.handled_count
                        .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                }
                Err(code) => {
                    self.send_error_reply(request_id, code);
                    self.error_count
                        .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                }
            }
            processed += 1;
        }
        processed
    }

    /// Number of replies that could not be written.
    #[must_use]
    pub fn reply_write_failures(&self) -> u64 {
        self.reply_write_failures
            .load(std::sync::atomic::Ordering::Acquire)
    }

    fn send_error_reply(&self, request_id: SampleIdentity, code: RemoteExceptionCode) {
        if self.write_reply(&request_id, code, None).is_err() {
            self.reply_write_failures
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        }
    }

    /// Writes the reply to request `request_id`.
    ///
    /// Basic: `ReplyHeader` naming the request, then the `Return` union —
    /// `body` for `Ok`, the union's `default` case otherwise (§7.5.1.1.7
    /// rule 3). The union is written where it stands in the sample, so
    /// classic CDR aligns its 8-octet members to the sample. Enhanced: the
    /// `Reply` structure as it is, or the one that carries `remoteEx`, with
    /// the identity of the request in `PID_RELATED_SAMPLE_IDENTITY`
    /// (§7.8.2).
    ///
    /// # Errors
    /// `RpcError::Codec` when `body` cannot be encoded; `RpcError::Dcps`
    /// when the writer refuses the sample.
    fn write_reply(
        &self,
        request_id: &SampleIdentity,
        code: RemoteExceptionCode,
        body: Option<&TOut>,
    ) -> RpcResult<()> {
        match self.options.mapping {
            RpcMapping::Basic if self.pair.is_some() => {
                // §7.3.1.4: the reply type is the topic type and holds its
                // own header: the sample is the whole structure, whose
                // header names the request and carries the code.
                let pair = self
                    .pair
                    .ok_or_else(|| RpcError::Codec("no pair stamp".into()))?;
                let header = ReplyHeader::new(*request_id, code);
                let value = match body.filter(|_| code == RemoteExceptionCode::Ok) {
                    Some(reply) => (pair.stamp)(reply, &header)?,
                    // A reply that reports a remote exception has no
                    // `data` of the service's; the type's default stands.
                    None => (pair.error)(&header)?,
                };
                let mut frame = Vec::new();
                value
                    .encode_continuing(&mut frame, self.options.xcdr1())
                    .map_err(|e| RpcError::Codec(alloc::format!("encode TOut: {e}")))?;
                self.reply_writer
                    .write(&RawBytes::new(frame))
                    .map_err(|e| RpcError::Dcps(alloc::format!("write reply: {e:?}")))
            }
            RpcMapping::Basic => {
                let header = ReplyHeader::new(*request_id, code);
                let mut frame = encode_reply_frame(&header, &[]);
                match body.filter(|_| code == RemoteExceptionCode::Ok) {
                    Some(reply) => reply
                        .encode_continuing(&mut frame, self.options.xcdr1())
                        .map_err(|e| RpcError::Codec(alloc::format!("encode TOut: {e}")))?,
                    None => frame.extend_from_slice(&ERROR_REPLY_DATA),
                }
                self.reply_writer
                    .write(&RawBytes::new(frame))
                    .map_err(|e| RpcError::Dcps(alloc::format!("write reply: {e:?}")))
            }
            RpcMapping::Enhanced => {
                let bytes = match body.filter(|_| code == RemoteExceptionCode::Ok) {
                    Some(reply) => {
                        let mut buf = Vec::new();
                        reply
                            .encode(&mut buf)
                            .map_err(|e| RpcError::Codec(alloc::format!("encode TOut: {e}")))?;
                        buf
                    }
                    None => encode_remote_ex_reply(code, &self.options.remote_ex_member),
                };
                self.reply_writer
                    .write_related(
                        &RawBytes::new(bytes),
                        zerodds_rtps::inline_qos::SampleIdentityBytes::new(
                            request_id.writer_guid,
                            request_id.sequence_number,
                        ),
                    )
                    .map(|_sequence_number| ())
                    .map_err(|e| RpcError::Dcps(alloc::format!("write reply: {e:?}")))
            }
        }
    }

    /// Test helper: the request reader, for reading what it announced and
    /// what its matched writers announced (`matched_rpc_writers`). Not
    /// stable, only for test inspection.
    #[doc(hidden)]
    #[must_use]
    pub fn __request_reader(&self) -> &DataReader<RawBytes> {
        &self.request_reader
    }

    /// Test helper: the reply writer, the other endpoint of the service
    /// (`matched_rpc_readers`). Not stable, only for test inspection.
    #[doc(hidden)]
    #[must_use]
    pub fn __reply_writer(&self) -> &DataWriter<RawBytes> {
        &self.reply_writer
    }

    /// Test helper: drains the offline-queued reply frames.
    #[doc(hidden)]
    #[must_use]
    pub fn __drain_reply_writer(&self) -> Vec<Vec<u8>> {
        self.reply_writer.__drain_pending()
    }

    /// Test helper: pushes a request directly into the reader inbox with
    /// the writer GUID and sequence number of its DATA. Intended only for
    /// offline tests.
    #[doc(hidden)]
    pub fn __push_request_from(
        &self,
        bytes: Vec<u8>,
        writer_guid: [u8; 16],
        sequence_number: i64,
    ) -> RpcResult<()> {
        self.request_reader
            .__push_raw_related(bytes, writer_guid, sequence_number, None)
            .map_err(|e| RpcError::Dcps(alloc::format!("push raw: {e:?}")))
    }

    /// Test helper: pushes a request frame directly into the reader inbox.
    #[doc(hidden)]
    pub fn __push_request_raw(&self, bytes: Vec<u8>) -> RpcResult<()> {
        self.request_reader
            .__push_raw(bytes)
            .map_err(|e| RpcError::Dcps(alloc::format!("push raw: {e:?}")))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::common_types::{RequestHeader, SampleIdentity};
    use crate::wire_codec::{decode_reply_frame, encode_request_frame};
    use zerodds_dcps::dds_type::{DdsType, RawBytes};
    use zerodds_dcps::factory::DomainParticipantFactory;
    use zerodds_dcps::qos::DomainParticipantQos;

    fn participant(domain: i32) -> DomainParticipant {
        DomainParticipantFactory::instance()
            .create_participant_offline(domain, DomainParticipantQos::default())
    }

    fn echo_handler() -> Arc<dyn ReplierHandler<RawBytes, RawBytes>> {
        Arc::new(FnHandler::new(|req: RawBytes| -> Result<RawBytes, _> {
            Ok(req)
        }))
    }

    fn err_handler(code: RemoteExceptionCode) -> Arc<dyn ReplierHandler<RawBytes, RawBytes>> {
        Arc::new(FnHandler::new(move |_req: RawBytes| Err(code)))
    }

    #[test]
    fn replier_new_creates_endpoints() {
        let p = participant(201);
        let q = RpcQos::default_basic();
        let r = Replier::<RawBytes, RawBytes>::new(&p, "Calc", &q, echo_handler()).unwrap();
        assert_eq!(r.service_name(), "Calc");
        assert_eq!(r.instance_name(), "");
        assert_eq!(r.handled_count(), 0);
    }

    #[test]
    fn replier_invalid_service_name_rejected() {
        let p = participant(202);
        let q = RpcQos::default_basic();
        let err = Replier::<RawBytes, RawBytes>::new(&p, "", &q, echo_handler()).unwrap_err();
        assert!(matches!(err, RpcError::InvalidServiceName(_)));
    }

    #[test]
    fn replier_duplicate_instance_name_rejected() {
        let p = participant(203);
        let q = RpcQos::default_basic();
        let _r1 =
            Replier::<RawBytes, RawBytes>::with_instance(&p, "Calc", "calc-A", &q, echo_handler())
                .unwrap();
        let err =
            Replier::<RawBytes, RawBytes>::with_instance(&p, "Calc", "calc-A", &q, echo_handler())
                .unwrap_err();
        assert!(matches!(err, RpcError::DuplicateInstanceName(_)));
    }

    #[test]
    fn tick_with_no_requests_is_noop() {
        let p = participant(204);
        let q = RpcQos::default_basic();
        let r = Replier::<RawBytes, RawBytes>::new(&p, "Calc", &q, echo_handler()).unwrap();
        assert_eq!(r.tick(), 0);
        assert_eq!(r.handled_count(), 0);
    }

    #[test]
    fn tick_processes_request_and_writes_reply() {
        let p = participant(205);
        let q = RpcQos::default_basic();
        let r = Replier::<RawBytes, RawBytes>::new(&p, "Calc", &q, echo_handler()).unwrap();
        let id = SampleIdentity::new([1u8; 16], 42);
        let req_header = RequestHeader::new(id, "");
        let req_frame = encode_request_frame(&req_header, &[7u8, 8, 9]).unwrap();
        r.__push_request_raw(req_frame).unwrap();
        assert_eq!(r.tick(), 1);
        assert_eq!(r.handled_count(), 1);
        let frames = r.__drain_reply_writer();
        assert_eq!(frames.len(), 1);
        let (reply_header, payload) = decode_reply_frame(&frames[0]).unwrap();
        assert_eq!(reply_header.related_request_id, id);
        assert_eq!(reply_header.remote_ex, RemoteExceptionCode::Ok);
        assert_eq!(payload, &[7u8, 8, 9]);
    }

    #[test]
    fn tick_propagates_handler_error_into_reply() {
        let p = participant(206);
        let q = RpcQos::default_basic();
        let r = Replier::<RawBytes, RawBytes>::new(
            &p,
            "Calc",
            &q,
            err_handler(RemoteExceptionCode::InvalidArgument),
        )
        .unwrap();
        let id = SampleIdentity::new([2u8; 16], 7);
        let frame = encode_request_frame(&RequestHeader::new(id, ""), &[1, 2]).unwrap();
        r.__push_request_raw(frame).unwrap();
        assert_eq!(r.tick(), 1);
        assert_eq!(r.error_count(), 1);
        assert_eq!(r.handled_count(), 0);
        let replies = r.__drain_reply_writer();
        let (h, payload) = decode_reply_frame(&replies[0]).unwrap();
        assert_eq!(h.related_request_id, id);
        assert_eq!(h.remote_ex, RemoteExceptionCode::InvalidArgument);
        assert_eq!(payload, ERROR_REPLY_DATA, "the Return union's default case");
    }

    /// A TIn whose decoder refuses every payload.
    struct Refusing;

    impl DdsType for Refusing {
        const TYPE_NAME: &'static str = "test::Refusing";

        fn encode(&self, _out: &mut Vec<u8>) -> Result<(), zerodds_dcps::dds_type::EncodeError> {
            Ok(())
        }

        fn decode(_bytes: &[u8]) -> Result<Self, zerodds_dcps::dds_type::DecodeError> {
            Err(zerodds_dcps::dds_type::DecodeError::Invalid {
                what: "the test type refuses every payload",
            })
        }
    }

    fn refusing_replier(domain: i32) -> Replier<Refusing, RawBytes> {
        let p = participant(domain);
        let handler: Arc<dyn ReplierHandler<Refusing, RawBytes>> =
            Arc::new(FnHandler::new(|_req: Refusing| {
                Ok::<_, RemoteExceptionCode>(RawBytes::new(alloc::vec![1]))
            }));
        Replier::<Refusing, RawBytes>::new(&p, "Calc", &RpcQos::default_basic(), handler).unwrap()
    }

    /// §7.5.2 REMOTE_EX_INVALID_ARGUMENT: "The value of the parameter
    /// passed has an illegal value". A payload the request type cannot
    /// decode is answered with that code; the handler is not called.
    #[test]
    fn tick_answers_an_undecodable_payload_with_invalid_argument() {
        let r = refusing_replier(214);
        let id = SampleIdentity::new([8u8; 16], 11);
        let frame = encode_request_frame(&RequestHeader::new(id, ""), &[1, 2, 3]).unwrap();
        r.__push_request_raw(frame).unwrap();
        assert_eq!(r.tick(), 0, "no request reached the handler");
        assert_eq!(r.handled_count(), 0);
        let replies = r.__drain_reply_writer();
        assert_eq!(replies.len(), 1);
        let (header, payload) = decode_reply_frame(&replies[0]).unwrap();
        assert_eq!(header.related_request_id, id);
        assert_eq!(header.remote_ex, RemoteExceptionCode::InvalidArgument);
        assert_eq!(payload, ERROR_REPLY_DATA);
    }

    /// The polling API answers the same way instead of dropping the
    /// request without a word.
    #[test]
    fn receive_requests_answers_an_undecodable_payload_with_invalid_argument() {
        let r = refusing_replier(215);
        let id = SampleIdentity::new([9u8; 16], 12);
        let frame = encode_request_frame(&RequestHeader::new(id, ""), &[4]).unwrap();
        r.__push_request_raw(frame).unwrap();
        assert!(r.receive_requests().is_empty());
        let replies = r.__drain_reply_writer();
        assert_eq!(replies.len(), 1);
        let (header, _) = decode_reply_frame(&replies[0]).unwrap();
        assert_eq!(header.related_request_id, id);
        assert_eq!(header.remote_ex, RemoteExceptionCode::InvalidArgument);
    }

    /// §7.5.2 REMOTE_EX_OUT_OF_RESOURCES: a handler that reports it is
    /// answered with that code and the default case of the `Return`
    /// union.
    #[test]
    fn tick_answers_out_of_resources() {
        let p = participant(216);
        let q = RpcQos::default_basic();
        let r = Replier::<RawBytes, RawBytes>::new(
            &p,
            "Calc",
            &q,
            err_handler(RemoteExceptionCode::OutOfResources),
        )
        .unwrap();
        let id = SampleIdentity::new([10u8; 16], 13);
        let frame = encode_request_frame(&RequestHeader::new(id, ""), &[1]).unwrap();
        r.__push_request_raw(frame).unwrap();
        assert_eq!(r.tick(), 1);
        assert_eq!(r.error_count(), 1);
        let replies = r.__drain_reply_writer();
        let (header, payload) = decode_reply_frame(&replies[0]).unwrap();
        assert_eq!(header.related_request_id, id);
        assert_eq!(header.remote_ex, RemoteExceptionCode::OutOfResources);
        assert_eq!(payload, ERROR_REPLY_DATA);
    }

    #[test]
    fn tick_drops_malformed_request_silently() {
        let p = participant(207);
        let q = RpcQos::default_basic();
        let r = Replier::<RawBytes, RawBytes>::new(&p, "Calc", &q, echo_handler()).unwrap();
        r.__push_request_raw(alloc::vec![0u8; 5]).unwrap(); // Truncated header.
        assert_eq!(r.tick(), 0);
        assert_eq!(r.handled_count(), 0);
        assert!(r.__drain_reply_writer().is_empty());
    }

    #[test]
    fn tick_filters_requests_for_other_instance_name() {
        let p = participant(208);
        let q = RpcQos::default_basic();
        let r =
            Replier::<RawBytes, RawBytes>::with_instance(&p, "Calc", "calc-A", &q, echo_handler())
                .unwrap();
        let id = SampleIdentity::new([3u8; 16], 1);
        // Header instance_name="calc-B" — we are calc-A → ignore.
        let frame = encode_request_frame(&RequestHeader::new(id, "calc-B"), &[1]).unwrap();
        r.__push_request_raw(frame).unwrap();
        assert_eq!(r.tick(), 0);
        assert_eq!(r.handled_count(), 0);
    }

    #[test]
    fn a_named_replier_does_not_answer_a_request_without_an_instance() {
        // §7.11.1.4.3: the request is bound to an instance. The empty name is
        // the default instance; a replier of "calc-A" is not it and must not
        // answer.
        let p = participant(210);
        let q = RpcQos::default_basic();
        let r =
            Replier::<RawBytes, RawBytes>::with_instance(&p, "Calc", "calc-A", &q, echo_handler())
                .unwrap();
        let id = SampleIdentity::new([4u8; 16], 1);
        let frame = encode_request_frame(&RequestHeader::new(id, ""), &[1]).unwrap();
        r.__push_request_raw(frame).unwrap();
        assert_eq!(r.tick(), 0);
        assert_eq!(r.handled_count(), 0);
    }

    #[test]
    fn tick_handles_multiple_requests_in_one_call() {
        let p = participant(209);
        let q = RpcQos::default_basic();
        let r = Replier::<RawBytes, RawBytes>::new(&p, "Calc", &q, echo_handler()).unwrap();
        for i in 1..=5u64 {
            let id = SampleIdentity::new([0xAB; 16], i);
            let frame =
                encode_request_frame(&RequestHeader::new(id, ""), &[u8::try_from(i).unwrap()])
                    .unwrap();
            r.__push_request_raw(frame).unwrap();
        }
        assert_eq!(r.tick(), 5);
        assert_eq!(r.handled_count(), 5);
        let replies = r.__drain_reply_writer();
        assert_eq!(replies.len(), 5);
    }

    #[test]
    fn fn_handler_passthrough_works() {
        let h = FnHandler::new(|x: RawBytes| Ok::<RawBytes, RemoteExceptionCode>(x));
        let res = h.handle(RawBytes::new(alloc::vec![1, 2])).unwrap();
        assert_eq!(res.data, alloc::vec![1, 2]);
    }

    #[test]
    fn receive_requests_and_send_reply_expose_the_identity() {
        // §7.9.2.2 way 3 and §7.8.2.1: the application polls the
        // requests and answers them, and it can read the request
        // identity.
        let p = participant(211);
        let q = RpcQos::default_basic();
        let r = Replier::<RawBytes, RawBytes>::new(&p, "Calc", &q, echo_handler()).unwrap();
        let id = SampleIdentity::new([5u8; 16], 9);
        let frame = encode_request_frame(&RequestHeader::new(id, ""), &[1, 2]).unwrap();
        r.__push_request_raw(frame).unwrap();
        let pending = r.receive_requests();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].identity.sample_identity, id);
        assert!(!pending[0].identity.via_inline_qos);
        r.send_reply(&pending[0].identity, RawBytes::new(alloc::vec![3, 4]))
            .unwrap();
        let replies = r.__drain_reply_writer();
        let (header, payload) = decode_reply_frame(&replies[0]).unwrap();
        assert_eq!(header.related_request_id, id);
        assert_eq!(payload, &[3, 4]);
        assert_eq!(r.handled_count(), 1);
    }

    #[test]
    fn send_exception_answers_with_the_code() {
        let p = participant(212);
        let q = RpcQos::default_basic();
        let r = Replier::<RawBytes, RawBytes>::new(&p, "Calc", &q, echo_handler()).unwrap();
        let id = SampleIdentity::new([6u8; 16], 2);
        let frame = encode_request_frame(&RequestHeader::new(id, ""), &[1]).unwrap();
        r.__push_request_raw(frame).unwrap();
        let pending = r.receive_requests();
        r.send_exception(&pending[0].identity, RemoteExceptionCode::Unsupported)
            .unwrap();
        let replies = r.__drain_reply_writer();
        let (header, payload) = decode_reply_frame(&replies[0]).unwrap();
        assert_eq!(header.remote_ex, RemoteExceptionCode::Unsupported);
        assert_eq!(header.related_request_id, id);
        assert_eq!(payload, ERROR_REPLY_DATA, "the Return union's default case");
        assert_eq!(r.error_count(), 1);
    }

    #[test]
    fn dispatch_async_hands_the_replier_and_the_identity_to_the_listener() {
        // §7.9.2.2 way 2: the asynchronous listener receives the replier
        // handle and answers through the Replier API.
        struct Listener {
            seen: std::sync::atomic::AtomicU64,
        }
        impl AsyncReplierHandler<RawBytes, RawBytes> for Listener {
            fn on_request(
                &self,
                replier: &Replier<RawBytes, RawBytes>,
                request: RawBytes,
                identity: RequestIdentity,
            ) {
                self.seen.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                replier.send_reply(&identity, request).unwrap();
            }
        }
        let p = participant(213);
        let q = RpcQos::default_basic();
        let r = Replier::<RawBytes, RawBytes>::new(&p, "Calc", &q, echo_handler()).unwrap();
        let id = SampleIdentity::new([7u8; 16], 3);
        let frame = encode_request_frame(&RequestHeader::new(id, ""), &[9]).unwrap();
        r.__push_request_raw(frame).unwrap();
        let listener = Listener {
            seen: std::sync::atomic::AtomicU64::new(0),
        };
        assert_eq!(r.dispatch_async(&listener), 1);
        assert_eq!(listener.seen.load(std::sync::atomic::Ordering::Acquire), 1);
        let replies = r.__drain_reply_writer();
        let (header, payload) = decode_reply_frame(&replies[0]).unwrap();
        assert_eq!(header.related_request_id, id);
        assert_eq!(payload, &[9]);
    }

    #[test]
    fn the_topics_carry_the_interface_type_names() {
        let p = participant(230);
        let q = RpcQos::default_basic();
        let r = Replier::<RawBytes, RawBytes>::new(&p, "Calc", &q, echo_handler()).unwrap();
        assert_eq!(r.request_type_name(), "Calc_Request");
        assert_eq!(r.reply_type_name(), "Calc_Reply");
    }

    /// §7.4.1: "For the request-reply style language binding,
    /// <interface_name> and the following underscore shall not be
    /// captured automatically." The replier's endpoints carry the same
    /// names as the requester's.
    #[test]
    fn request_reply_style_drops_the_interface_segment() {
        let p = participant(232);
        let q = RpcQos::default_basic();
        let r = Replier::<RawBytes, RawBytes>::with_instance_style(
            &p,
            "Calculator",
            "",
            &q,
            BindingStyle::RequestReply,
            echo_handler(),
        )
        .unwrap();
        assert_eq!(r.request_topic_name(), "Service_Request");
        assert_eq!(r.reply_topic_name(), "Service_Reply");
        // The function-call style names the interface.
        let called = Replier::<RawBytes, RawBytes>::with_instance_style(
            &p,
            "Calculator",
            "other",
            &q,
            BindingStyle::FunctionCall,
            echo_handler(),
        )
        .unwrap();
        assert_eq!(called.request_topic_name(), "Calculator_Service_Request");
        assert_eq!(called.reply_topic_name(), "Calculator_Service_Reply");
    }

    #[test]
    fn the_options_name_the_topic_types() {
        let p = participant(231);
        let q = RpcQos::default_basic();
        let options = RpcEndpointOptions {
            type_names: Some(crate::endpoint_options::TopicTypeNames::for_interface(
                "robot", "Calc",
            )),
            ..RpcEndpointOptions::default()
        };
        let r = Replier::<RawBytes, RawBytes>::with_options(
            &p,
            "robot_Calc",
            "",
            &q,
            BindingStyle::FunctionCall,
            &AnnotatedTopics::none(),
            &options,
            echo_handler(),
        )
        .unwrap();
        assert_eq!(r.request_type_name(), "robot::Calc_Request");
        assert_eq!(r.reply_type_name(), "robot::Calc_Reply");
    }

    fn enhanced(client_matching: ClientMatching, accept_unannounced: bool) -> RpcEndpointOptions {
        RpcEndpointOptions {
            client_matching,
            accept_unannounced,
            ..RpcEndpointOptions::enhanced()
        }
    }

    fn enhanced_replier(
        domain: i32,
        options: &RpcEndpointOptions,
    ) -> (DomainParticipant, Replier<RawBytes, RawBytes>) {
        let p = participant(domain);
        let r = Replier::<RawBytes, RawBytes>::with_options(
            &p,
            "Calc",
            "",
            &RpcQos::default_enhanced(),
            BindingStyle::FunctionCall,
            &AnnotatedTopics::none(),
            options,
            echo_handler(),
        )
        .unwrap();
        (p, r)
    }

    /// §7.8.2: under the Enhanced mapping the request is the structure
    /// alone, identified by the writer GUID and sequence number of its
    /// DATA.
    #[test]
    fn an_enhanced_request_is_identified_by_its_data() {
        let (_p, r) = enhanced_replier(232, &enhanced(ClientMatching::Off, false));
        r.__push_request_from(alloc::vec![5, 6, 7], [9; 16], 44)
            .unwrap();
        let pending = r.receive_requests();
        assert_eq!(pending.len(), 1);
        assert_eq!(
            pending[0].request.data,
            [5, 6, 7],
            "no header in the sample"
        );
        assert_eq!(
            pending[0].identity.sample_identity,
            SampleIdentity::new([9; 16], 44)
        );
        assert!(pending[0].identity.via_inline_qos);
    }

    /// §7.6.2.2.1: the service "shall reject requests" of a client it has
    /// not discovered completely. The request is held, not lost.
    #[test]
    fn an_enhanced_request_of_an_undiscovered_client_is_held() {
        let (_p, r) = enhanced_replier(233, &enhanced(ClientMatching::Hold, false));
        r.__push_request_from(alloc::vec![1], [9; 16], 1).unwrap();
        r.__push_request_from(alloc::vec![2], [9; 16], 2).unwrap();
        assert!(r.receive_requests().is_empty());
        assert_eq!(r.held_requests(), 2);
        assert_eq!(r.held_overflow(), 0);
        assert!(r.__drain_reply_writer().is_empty(), "nothing was answered");
    }

    #[test]
    fn the_named_alternatives_release_a_request_of_an_undiscovered_client() {
        for (domain, options) in [
            (234, enhanced(ClientMatching::Off, false)),
            (235, enhanced(ClientMatching::Hold, true)),
        ] {
            let (_p, r) = enhanced_replier(domain, &options);
            r.__push_request_from(alloc::vec![1], [9; 16], 1).unwrap();
            assert_eq!(r.receive_requests().len(), 1, "{options:?}");
            assert_eq!(r.held_requests(), 0);
        }
    }

    /// A DATA without writer identity cannot be answered; it is counted.
    #[test]
    fn an_enhanced_request_without_a_wire_identity_is_counted() {
        let (_p, r) = enhanced_replier(236, &enhanced(ClientMatching::Off, false));
        r.__push_request_raw(alloc::vec![1, 2, 3]).unwrap();
        assert!(r.receive_requests().is_empty());
        assert_eq!(r.unidentified_requests(), 1);
    }

    /// Enhanced replies need the inline QoS of a live runtime; on a writer
    /// without one the reply is refused and counted, not sent without the
    /// identity.
    #[test]
    fn an_enhanced_reply_without_a_runtime_is_refused() {
        let (_p, r) = enhanced_replier(237, &enhanced(ClientMatching::Off, false));
        r.__push_request_from(alloc::vec![1], [9; 16], 1).unwrap();
        assert_eq!(r.tick(), 0, "the request was not answered");
        assert_eq!(r.reply_write_failures(), 1);
        assert_eq!(r.handled_count(), 0);
        assert!(r.__drain_reply_writer().is_empty());
    }

    fn wide_replier(
        domain: i32,
        encoding: crate::endpoint_options::WireEncoding,
    ) -> (
        DomainParticipant,
        Replier<crate::test_types::Wide, crate::test_types::Wide>,
    ) {
        let p = participant(domain);
        let options = RpcEndpointOptions {
            wire_encoding: encoding,
            ..RpcEndpointOptions::default()
        };
        let handler: Arc<dyn ReplierHandler<_, _>> =
            Arc::new(FnHandler::new(|request: crate::test_types::Wide| {
                Ok::<_, RemoteExceptionCode>(request)
            }));
        let r = Replier::with_options(
            &p,
            "Calc",
            "",
            &RpcQos::default_basic(),
            BindingStyle::FunctionCall,
            &AnnotatedTopics::none(),
            &options,
            handler,
        )
        .unwrap();
        (p, r)
    }

    /// §7.5.1.1: under classic CDR the `unsigned long long` of the `Call`
    /// union at offset 32 sits at 40; the replier reads it there, and
    /// answers with a `Return` union that begins at 28, behind the
    /// `ReplyHeader`.
    #[test]
    fn a_classic_cdr_request_is_read_where_it_stands() {
        use crate::endpoint_options::WireEncoding;
        use crate::test_types::Wide;
        let id = SampleIdentity::new([5u8; 16], 3);
        let mut frame = encode_request_frame(&RequestHeader::new(id, ""), &[]).unwrap();
        assert_eq!(frame.len(), 32);
        frame.extend_from_slice(&0x0102_0304u32.to_le_bytes());
        frame.extend_from_slice(&[0; 4]);
        frame.extend_from_slice(&0x1112_1314_1516_1718u64.to_le_bytes());

        let (_p, r) = wide_replier(240, WireEncoding::Xcdr1);
        r.__push_request_raw(frame.clone()).unwrap();
        assert_eq!(r.tick(), 1);
        let replies = r.__drain_reply_writer();
        let (header, payload) = decode_reply_frame(&replies[0]).unwrap();
        assert_eq!(header.related_request_id, id);
        // Behind the 28-octet header the union is a 4-octet member, then the
        // 8-octet one at 32.
        assert_eq!(payload.len(), 4 + 8);
        assert_eq!(payload[..4], 0x0102_0304u32.to_le_bytes());
        assert_eq!(payload[4..], 0x1112_1314_1516_1718u64.to_le_bytes());

        // The same octets read as XCDR2 put the 8-octet member at 36.
        let (_p, r) = wide_replier(241, WireEncoding::Xcdr2);
        r.__push_request_raw(frame).unwrap();
        let pending = r.receive_requests();
        assert_eq!(
            pending[0].request,
            Wide {
                a: 0x0102_0304,
                b: 0x1516_1718_0000_0000
            }
        );
    }

    fn header_replier(
        domain: i32,
        handler: Arc<
            dyn ReplierHandler<crate::test_types::HeaderRequest, crate::test_types::HeaderReply>,
        >,
    ) -> (
        DomainParticipant,
        Replier<crate::test_types::HeaderRequest, crate::test_types::HeaderReply>,
    ) {
        let p = participant(domain);
        let r = Replier::with_pair(
            &p,
            "Calc",
            "",
            &RpcQos::default_basic(),
            BindingStyle::RequestReply,
            &AnnotatedTopics::none(),
            &RpcEndpointOptions::default(),
            handler,
        )
        .unwrap();
        (p, r)
    }

    fn header_request_frame(id: SampleIdentity) -> Vec<u8> {
        use crate::header_pair::RequestWithHeader;
        let mut frame = Vec::new();
        crate::test_types::HeaderRequest::new(5, 0x0A0B_0C0D_0E0F_1011)
            .with_request_header(&RequestHeader::new(id, ""))
            .unwrap()
            .encode_continuing(&mut frame, true)
            .unwrap();
        frame
    }

    /// §7.3.1.4: the replier reads the request's identity from the header
    /// the request type carries, hands over the whole structure, and writes
    /// the reply type with its own header naming the request.
    #[test]
    fn a_type_pair_with_headers_is_answered_by_the_header() {
        use crate::test_types::{HeaderReply, HeaderRequest};
        let id = SampleIdentity::new([4u8; 16], 11);
        let handler: Arc<dyn ReplierHandler<HeaderRequest, HeaderReply>> =
            Arc::new(FnHandler::new(|request: HeaderRequest| {
                assert_eq!(request.a, 5);
                Ok::<_, RemoteExceptionCode>(HeaderReply {
                    r: request.b,
                    ..HeaderReply::default()
                })
            }));
        let (_p, r) = header_replier(250, handler);
        r.__push_request_raw(header_request_frame(id)).unwrap();
        assert_eq!(r.tick(), 1);
        let frames = r.__drain_reply_writer();
        let (header, _) = decode_reply_frame(&frames[0]).unwrap();
        assert_eq!(header.related_request_id, id);
        assert_eq!(header.remote_ex, RemoteExceptionCode::Ok);
        // The 28-octet header, then the 8-octet member at 32 (classic CDR).
        assert_eq!(frames[0][32..40], 0x0A0B_0C0D_0E0F_1011u64.to_le_bytes());
    }

    #[test]
    fn a_remote_exception_of_a_type_pair_is_the_header_with_a_default_body() {
        use crate::test_types::{HeaderReply, HeaderRequest};
        let id = SampleIdentity::new([4u8; 16], 12);
        let handler: Arc<dyn ReplierHandler<HeaderRequest, HeaderReply>> =
            Arc::new(FnHandler::new(|_request: HeaderRequest| {
                Err::<HeaderReply, _>(RemoteExceptionCode::Unsupported)
            }));
        let (_p, r) = header_replier(251, handler);
        r.__push_request_raw(header_request_frame(id)).unwrap();
        assert_eq!(r.tick(), 1);
        let frames = r.__drain_reply_writer();
        let (header, _) = decode_reply_frame(&frames[0]).unwrap();
        assert_eq!(header.related_request_id, id);
        assert_eq!(header.remote_ex, RemoteExceptionCode::Unsupported);
        assert_eq!(frames[0][32..40], [0; 8]);
    }
}
