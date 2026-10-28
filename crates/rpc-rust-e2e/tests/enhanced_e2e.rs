// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Enhanced Service Mapping of DDS-RPC 1.0 over a real `DcpsRuntime`:
//! requests without a header identified by their sample identity (§7.5.1.2,
//! §7.8.2), replies correlated by `PID_RELATED_SAMPLE_IDENTITY`, the
//! discovery extension of §7.6.2.1 and the rule of §7.6.2.2.2 that the
//! client sends no request before it has discovered the service.
//!
//! Requester and replier run on one participant: the samples go through
//! the runtime's own delivery path, which carries the sequence number and
//! the related identity exactly as the wire path does.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use zerodds_dcps::dds_type::RawBytes;
use zerodds_dcps::factory::DomainParticipantFactory;
use zerodds_dcps::participant::DomainParticipant;
use zerodds_dcps::qos::DomainParticipantQos;
use zerodds_rpc::common_types::RemoteExceptionCode;
use zerodds_rpc::endpoint_options::{RpcEndpointOptions, ServiceMatching};
use zerodds_rpc::error::RpcError;
use zerodds_rpc::qos_profile::RpcQos;
use zerodds_rpc::replier::{FnHandler, Replier, ReplierHandler};
use zerodds_rpc::requester::Requester;
use zerodds_rpc::topic_naming::{AnnotatedTopics, BindingStyle};

fn participant(domain: i32) -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant(domain, DomainParticipantQos::default())
        .expect("a real participant on a leased domain")
}

fn replier(
    p: &DomainParticipant,
    instance: &str,
    options: &RpcEndpointOptions,
    handler: Arc<dyn ReplierHandler<RawBytes, RawBytes>>,
) -> Replier<RawBytes, RawBytes> {
    Replier::with_options(
        p,
        "Echo",
        instance,
        &RpcQos::default_enhanced(),
        BindingStyle::FunctionCall,
        &AnnotatedTopics::none(),
        options,
        handler,
    )
    .expect("a replier")
}

fn requester(
    p: &DomainParticipant,
    instance: &str,
    options: &RpcEndpointOptions,
) -> Requester<RawBytes, RawBytes> {
    Requester::with_options(
        p,
        "Echo",
        instance,
        &RpcQos::default_enhanced(),
        BindingStyle::FunctionCall,
        &AnnotatedTopics::none(),
        options,
    )
    .expect("a requester")
}

fn echo() -> Arc<dyn ReplierHandler<RawBytes, RawBytes>> {
    Arc::new(FnHandler::new(|request: RawBytes| {
        let mut data = request.data;
        data.reverse();
        Ok::<_, RemoteExceptionCode>(RawBytes::new(data))
    }))
}

/// Ticks `replier` in the background until dropped.
struct Ticking {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Ticking {
    fn spawn(replier: Replier<RawBytes, RawBytes>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            while !stop_for_thread.load(Ordering::Acquire) {
                let _ = replier.tick();
                thread::sleep(Duration::from_millis(2));
            }
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for Ticking {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn eventually(cond: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    cond()
}

/// §7.8.2: the request has no header, the reply is found by the identity
/// of the request it names.
#[test]
fn an_enhanced_call_is_answered_through_the_related_sample_identity() {
    let lease = zerodds_test_domains::lease_domain();
    let p = participant(lease.id());
    let options = RpcEndpointOptions::enhanced();
    let _service = Ticking::spawn(replier(&p, "", &options, echo()));
    let client = requester(&p, "", &options);

    let reply = client
        .send_request_blocking(
            &RawBytes::new(vec![1, 2, 3, 4]),
            Some(Duration::from_secs(10)),
        )
        .expect("the service answers");
    assert_eq!(reply.data, vec![4, 3, 2, 1]);
    assert_eq!(client.uncorrelated_replies(), 0);
    assert_eq!(client.pending_count(), 0);
}

/// §7.5.1.2.5 rule 5: a remote exception is the `remoteEx` member.
#[test]
fn an_enhanced_remote_exception_is_the_remote_ex_member() {
    let lease = zerodds_test_domains::lease_domain();
    let p = participant(lease.id());
    let options = RpcEndpointOptions::enhanced();
    let refuse: Arc<dyn ReplierHandler<RawBytes, RawBytes>> =
        Arc::new(FnHandler::new(|_request: RawBytes| {
            Err::<RawBytes, _>(RemoteExceptionCode::UnknownOperation)
        }));
    let _service = Ticking::spawn(replier(&p, "", &options, refuse));
    let client = requester(&p, "", &options);

    let err = client
        .send_request_blocking(&RawBytes::new(vec![1]), Some(Duration::from_secs(10)))
        .unwrap_err();
    assert_eq!(
        err,
        RpcError::RemoteException(RemoteExceptionCode::UnknownOperation.as_u32())
    );
}

/// §7.5.1.1.6 and §7.5.1.1.7: the topic types are `${I}_Request` and
/// `${I}_Reply`.
#[test]
fn the_topics_are_announced_with_the_interface_type_names() {
    let lease = zerodds_test_domains::lease_domain();
    let p = participant(lease.id());
    let client = requester(&p, "", &RpcEndpointOptions::default());
    assert_eq!(client.request_type_name(), "Echo_Request");
    assert_eq!(client.reply_type_name(), "Echo_Reply");
}

/// §7.6.2.2.2, Figure 7.4: the client is complete only against the
/// service instance it names; §7.6.2.1.1: the client announces no
/// instance name.
#[test]
fn the_client_waits_for_the_service_instance_it_names() {
    let lease = zerodds_test_domains::lease_domain();
    let p = participant(lease.id());
    let options = RpcEndpointOptions::enhanced();
    let _service = replier(&p, "calc-A", &options, echo());

    let wanted = requester(&p, "calc-A", &options);
    let other = requester(&p, "calc-B", &options);
    let unnamed = requester(&p, "", &options);
    assert!(
        eventually(|| wanted.service_matched()),
        "the client of calc-A discovers calc-A"
    );
    assert!(!other.service_matched(), "calc-B is not served");
    assert!(
        !unnamed.service_matched(),
        "the unnamed instance is not served"
    );

    let failing = RpcEndpointOptions {
        service_matching: ServiceMatching::Fail,
        ..RpcEndpointOptions::enhanced()
    };
    let strict = requester(&p, "calc-C", &failing);
    assert_eq!(
        strict.send_oneway(&RawBytes::new(vec![1])).unwrap_err(),
        RpcError::ServiceNotMatched
    );
}
