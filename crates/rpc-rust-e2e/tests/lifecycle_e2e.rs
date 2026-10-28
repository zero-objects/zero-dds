// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-RPC 1.0 §7.9.2.2 and §7.9.3 over a real `DcpsRuntime`: the
//! listeners are called as a request or a reply arrives, and deactivating
//! a service deletes its DataReader and DataWriter.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use zerodds_dcps::dds_type::RawBytes;
use zerodds_dcps::factory::DomainParticipantFactory;
use zerodds_dcps::participant::DomainParticipant;
use zerodds_dcps::qos::DomainParticipantQos;
use zerodds_rpc::common_types::RemoteExceptionCode;
use zerodds_rpc::endpoint_options::RpcEndpointOptions;
use zerodds_rpc::qos_profile::RpcQos;
use zerodds_rpc::replier::{AsyncReplierHandler, FnHandler, Replier, ReplierHandler};
use zerodds_rpc::request_identity::RequestIdentity;
use zerodds_rpc::requester::Requester;
use zerodds_rpc::topic_naming::{AnnotatedTopics, BindingStyle};

fn participant(domain: i32) -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant(domain, DomainParticipantQos::default())
        .expect("a real participant on a leased domain")
}

fn echo() -> Arc<dyn ReplierHandler<RawBytes, RawBytes>> {
    Arc::new(FnHandler::new(|request: RawBytes| {
        Ok::<_, RemoteExceptionCode>(request)
    }))
}

fn replier(
    p: &DomainParticipant,
    instance: &str,
    options: &RpcEndpointOptions,
    handler: Arc<dyn ReplierHandler<RawBytes, RawBytes>>,
) -> Replier<RawBytes, RawBytes> {
    Replier::with_options(
        p,
        "Life",
        instance,
        &RpcQos::default_basic(),
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
        "Life",
        instance,
        &RpcQos::default_basic(),
        BindingStyle::FunctionCall,
        &AnnotatedTopics::none(),
        options,
    )
    .expect("a requester")
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

/// §7.9.2.2 way 1: "a synchronous service listener callback […] is
/// invoked for each received request" — without the application ticking
/// the replier or the requester.
#[test]
fn a_call_is_answered_without_anyone_ticking() {
    let lease = zerodds_test_domains::lease_domain();
    let p = participant(lease.id());
    let options = RpcEndpointOptions::default();
    let service = Arc::new(replier(&p, "", &options, echo()));
    service.activate();
    let client = Arc::new(requester(&p, "", &options));
    client.activate();
    assert!(eventually(|| {
        client.request_writer_matched() && service.reply_writer_matched()
    }));

    let (_id, rx) = client
        .send_request_async(&RawBytes::new(vec![7, 8, 9]))
        .expect("the request is written");
    let reply = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the reply arrives")
        .expect("it is not a remote exception");
    assert_eq!(reply, vec![7, 8, 9]);
    assert_eq!(service.handled_count(), 1);
}

struct Counting {
    seen: AtomicU64,
}

impl AsyncReplierHandler<RawBytes, RawBytes> for Counting {
    fn on_request(
        &self,
        replier: &Replier<RawBytes, RawBytes>,
        request: RawBytes,
        identity: RequestIdentity,
    ) {
        self.seen.fetch_add(1, Ordering::AcqRel);
        replier
            .send_reply(&identity, request)
            .expect("the reply is written");
    }
}

/// §7.9.2.2 way 2: the asynchronous listener is called on arrival and
/// answers through the replier.
#[test]
fn the_asynchronous_listener_is_called_as_the_request_arrives() {
    let lease = zerodds_test_domains::lease_domain();
    let p = participant(lease.id());
    let options = RpcEndpointOptions::default();
    let service = Arc::new(replier(&p, "", &options, echo()));
    let listener = Arc::new(Counting {
        seen: AtomicU64::new(0),
    });
    service.activate_async(listener.clone());
    let client = Arc::new(requester(&p, "", &options));
    client.activate();
    assert!(eventually(|| {
        client.request_writer_matched() && service.reply_writer_matched()
    }));

    let (_id, rx) = client
        .send_request_async(&RawBytes::new(vec![1]))
        .expect("the request is written");
    let reply = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the reply arrives")
        .expect("ok");
    assert_eq!(reply, vec![1]);
    assert_eq!(listener.seen.load(Ordering::Acquire), 1);
}

/// §7.9.3: "Deactivation shall delete the underlying DDS entities (i.e.,
/// DataReader and DataWriter only)." A peer sees the service end, and
/// the instance name is free again.
#[test]
fn closing_a_service_deletes_its_entities() {
    let lease = zerodds_test_domains::lease_domain();
    let p = participant(lease.id());
    let options = RpcEndpointOptions::enhanced();
    let service = replier(&p, "calc-A", &options, echo());
    let client = requester(&p, "calc-A", &options);
    assert!(
        eventually(|| client.service_matched()),
        "the client discovers the service"
    );

    service.close();
    assert!(
        eventually(|| !client.service_matched()),
        "the client sees the service end: its reader and writer are gone"
    );

    // The instance name is free: a second replier of that name can be made.
    let again = replier(&p, "calc-A", &options, echo());
    assert!(eventually(|| client.service_matched()));
    again.close();

    client.close();
    // And so is the client's: creating a requester of the same name works.
    let _second = requester(&p, "calc-A", &options);
}
