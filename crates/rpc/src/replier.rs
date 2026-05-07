// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! [`Replier`] — Server-Seite eines DDS-RPC-Service (Spec §7.10).
//!
//! Threading-Modell: spiegelt [`crate::requester::Requester`]. `tick()`
//! liest pending Requests, ruft den User-Handler synchron, schreibt das
//! Reply mit `related_request_id` zurueck. Caller orchestriert den Tick
//! selbst — kein eigener Thread.
//!
//! Spec-Referenz: §7.10 (Replier-Behaviour). `related_request_id`
//! wandert in dieser Foundation-Stufe ueber den Wire-Frame im
//! [`ReplyHeader`]; die parallele Inline-QoS-Variante mit
//! `PID_RELATED_SAMPLE_IDENTITY` (Spec §7.8.2) ist C6.1.D-Scope.

extern crate alloc;

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::marker::PhantomData;

use zerodds_dcps::dds_type::{DdsType, RawBytes};
use zerodds_dcps::participant::DomainParticipant;
use zerodds_dcps::publisher::DataWriter;
use zerodds_dcps::qos::{PublisherQos, SubscriberQos, TopicQos};
use zerodds_dcps::subscriber::DataReader;

use crate::common_types::{RemoteExceptionCode, ReplyHeader};
use crate::error::{RpcError, RpcResult};
use crate::qos_profile::RpcQos;
use crate::requester::{InstanceClaim, InstanceRole, try_claim_instance};
use crate::topic_naming::ServiceTopicNames;
use crate::wire_codec::{decode_request_frame, encode_reply_frame};

// ---------------------------------------------------------------------
// ReplierHandler
// ---------------------------------------------------------------------

/// User-Hook, der einen einzelnen Request bearbeitet.
///
/// Sync-only — die Foundation-Stufe ruft den Handler direkt im
/// `tick()`-Pfad auf. Async-Variante ist C6.1.D.
pub trait ReplierHandler<TIn, TOut>: Send + Sync {
    /// Bearbeitet einen Request. `Ok(reply)` ⇒ erfolgreiches Reply mit
    /// `RemoteExceptionCode::Ok`. `Err(code)` ⇒ Reply mit dem
    /// gegebenen Exception-Code und leerer Payload.
    fn handle(&self, request: TIn) -> Result<TOut, RemoteExceptionCode>;
}

/// Convenience-Adapter, der eine `Fn(TIn) -> Result<TOut, RemoteExceptionCode>`-
/// Closure als [`ReplierHandler`] verfuegbar macht.
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
    /// Wrappt eine Closure.
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

// ---------------------------------------------------------------------
// Replier
// ---------------------------------------------------------------------

/// Server-Seite eines DDS-RPC-Service.
pub struct Replier<TIn: DdsType, TOut: DdsType> {
    service_name: String,
    instance_name: String,
    request_reader: DataReader<RawBytes>,
    reply_writer: DataWriter<RawBytes>,
    handler: Arc<dyn ReplierHandler<TIn, TOut>>,
    handled_count: std::sync::atomic::AtomicU64,
    error_count: std::sync::atomic::AtomicU64,
    _claim: InstanceClaim,
    _phantom: PhantomData<fn() -> (TIn, TOut)>,
}

impl<TIn: DdsType, TOut: DdsType> core::fmt::Debug for Replier<TIn, TOut> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Replier")
            .field("service", &self.service_name)
            .field("instance", &self.instance_name)
            .finish_non_exhaustive()
    }
}

impl<TIn: DdsType + Send + 'static, TOut: DdsType + Send + 'static> Replier<TIn, TOut> {
    /// Legt einen neuen Replier gegen `service_name` an.
    ///
    /// # Errors
    /// Siehe [`crate::requester::Requester::new`].
    pub fn new(
        participant: &DomainParticipant,
        service_name: &str,
        qos: &RpcQos,
        handler: Arc<dyn ReplierHandler<TIn, TOut>>,
    ) -> RpcResult<Self> {
        Self::with_instance(participant, service_name, "", qos, handler)
    }

    /// Wie [`Self::new`], mit explizitem Instance-Namen.
    ///
    /// # Errors
    /// Siehe [`crate::requester::Requester::with_instance`].
    pub fn with_instance(
        participant: &DomainParticipant,
        service_name: &str,
        instance_name: &str,
        qos: &RpcQos,
        handler: Arc<dyn ReplierHandler<TIn, TOut>>,
    ) -> RpcResult<Self> {
        let topics = ServiceTopicNames::new(service_name)?;
        let claim = try_claim_instance(
            participant,
            InstanceRole::Replier,
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
        let request_reader = subscriber
            .create_datareader::<RawBytes>(&request_topic, qos.request_reader_qos())
            .map_err(|e| RpcError::Dcps(alloc::format!("create_datareader: {e:?}")))?;
        let reply_writer = publisher
            .create_datawriter::<RawBytes>(&reply_topic, qos.reply_writer_qos())
            .map_err(|e| RpcError::Dcps(alloc::format!("create_datawriter: {e:?}")))?;
        Ok(Self {
            service_name: service_name.into(),
            instance_name: instance_name.into(),
            request_reader,
            reply_writer,
            handler,
            handled_count: std::sync::atomic::AtomicU64::new(0),
            error_count: std::sync::atomic::AtomicU64::new(0),
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

    /// Anzahl bisher erfolgreich verarbeiteter Requests.
    #[must_use]
    pub fn handled_count(&self) -> u64 {
        self.handled_count
            .load(std::sync::atomic::Ordering::Acquire)
    }

    /// Anzahl Requests, die der Handler mit `Err(_)` quittiert hat.
    #[must_use]
    pub fn error_count(&self) -> u64 {
        self.error_count.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Liest pending Requests, ruft den Handler, schreibt Reply zurueck.
    /// Liefert die Anzahl der in diesem Tick verarbeiteten Requests.
    pub fn tick(&self) -> usize {
        let samples = match self.request_reader.take() {
            Ok(s) => s,
            Err(_) => return 0,
        };
        let mut processed = 0;
        for raw in samples {
            let bytes = raw.data;
            let (header, payload) = match decode_request_frame(&bytes) {
                Ok(t) => t,
                Err(_) => continue,
            };
            // Optional: Instance-Name-Filter — wenn unser Replier ein
            // konkretes Instance-Name beansprucht, ignorieren wir
            // Requests, die fuer einen anderen Instance-Namen gemeint
            // sind (Spec §7.6.2 — Instance-Routing).
            if !self.instance_name.is_empty()
                && !header.instance_name.is_empty()
                && header.instance_name != self.instance_name
            {
                continue;
            }
            let request_id = header.request_id;
            // Decode user payload.
            let req = match TIn::decode(payload) {
                Ok(v) => v,
                Err(_) => {
                    self.send_error_reply(request_id, RemoteExceptionCode::InvalidArgument);
                    continue;
                }
            };
            match self.handler.handle(req) {
                Ok(reply) => {
                    let mut user_buf = Vec::new();
                    if reply.encode(&mut user_buf).is_err() {
                        self.send_error_reply(request_id, RemoteExceptionCode::OutOfResources);
                        continue;
                    }
                    let header = ReplyHeader::new(request_id, RemoteExceptionCode::Ok);
                    let frame = encode_reply_frame(&header, &user_buf);
                    if self.reply_writer.write(&RawBytes::new(frame)).is_err() {
                        // Reply konnte nicht gesendet werden — Counters
                        // bleiben konsistent: handler war ok, aber Reply
                        // verloren. Wir zaehlen das nicht als Error.
                        continue;
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

    fn send_error_reply(
        &self,
        request_id: crate::common_types::SampleIdentity,
        code: RemoteExceptionCode,
    ) {
        let header = ReplyHeader::new(request_id, code);
        let frame = encode_reply_frame(&header, &[]);
        let _ = self.reply_writer.write(&RawBytes::new(frame));
    }

    /// Test-Helper: Drainiert die offline-gequeueten Reply-Frames.
    #[doc(hidden)]
    #[must_use]
    pub fn __drain_reply_writer(&self) -> Vec<Vec<u8>> {
        self.reply_writer.__drain_pending()
    }

    /// Test-Helper: pusht einen Request-Frame direkt in die Reader-Inbox.
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
    use zerodds_dcps::dds_type::RawBytes;
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
        let req_frame = encode_request_frame(&req_header, &[7u8, 8, 9]);
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
        let frame = encode_request_frame(&RequestHeader::new(id, ""), &[1, 2]);
        r.__push_request_raw(frame).unwrap();
        assert_eq!(r.tick(), 1);
        assert_eq!(r.error_count(), 1);
        assert_eq!(r.handled_count(), 0);
        let replies = r.__drain_reply_writer();
        let (h, payload) = decode_reply_frame(&replies[0]).unwrap();
        assert_eq!(h.related_request_id, id);
        assert_eq!(h.remote_ex, RemoteExceptionCode::InvalidArgument);
        assert!(payload.is_empty());
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
        // Header instance_name="calc-B" — wir sind calc-A → ignorieren.
        let frame = encode_request_frame(&RequestHeader::new(id, "calc-B"), &[1]);
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
                encode_request_frame(&RequestHeader::new(id, ""), &[u8::try_from(i).unwrap()]);
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
}
