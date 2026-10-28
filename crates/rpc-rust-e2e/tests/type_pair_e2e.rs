// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A service defined by a pair of types that carry their own header
//! (DDS-RPC 1.0 §7.3.1.4, Basic Service Mapping), generated from IDL and
//! run over a real `DcpsRuntime`: the runtime puts the request's identity
//! in the header of the request type, the replier answers with the reply
//! type whose header names the request.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use zerodds_dcps::factory::DomainParticipantFactory;
use zerodds_dcps::qos::DomainParticipantQos;
use zerodds_rpc::common_types::RemoteExceptionCode;
use zerodds_rpc::qos_profile::RpcQos;
use zerodds_rpc::replier::{FnHandler, ReplierHandler};
use zerodds_rpc_rust_e2e::pair_bindings::robot::{MoveReply, MoveRequest};

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

#[test]
fn a_pair_of_types_with_headers_is_called_and_answered() {
    let lease = zerodds_test_domains::lease_domain();
    let participant = DomainParticipantFactory::instance()
        .create_participant(lease.id(), DomainParticipantQos::default())
        .expect("a real participant");
    let qos = RpcQos::default_basic();

    let handler: Arc<dyn ReplierHandler<MoveRequest, MoveReply>> =
        Arc::new(FnHandler::new(|request: MoveRequest| {
            if request.x < 0 {
                return Err(RemoteExceptionCode::InvalidArgument);
            }
            Ok(MoveReply {
                ok: request.speed > 1.0,
                ..MoveReply::default()
            })
        }));
    let service = Arc::new(MoveRequest::replier(&participant, &qos, handler).expect("replier"));
    let stop = Arc::new(AtomicBool::new(false));
    let ticking = {
        let (service, stop) = (Arc::clone(&service), Arc::clone(&stop));
        thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                service.tick();
                thread::sleep(Duration::from_millis(2));
            }
        })
    };
    let client = MoveRequest::requester(&participant, &qos).expect("requester");
    assert!(eventually(|| {
        client.request_writer_matched() && service.reply_writer_matched()
    }));

    // The topic types are the two structures, under their scoped names.
    assert_eq!(client.request_type_name(), "robot::MoveRequest");
    assert_eq!(client.reply_type_name(), "robot::MoveReply");
    // §7.4.1, request-reply style: no interface name in the topic name.
    assert_eq!(client.request_topic_name(), "Service_Request");

    let reply = client
        .send_request_blocking(
            &MoveRequest {
                x: 3,
                speed: 2.5,
                ..MoveRequest::default()
            },
            Some(Duration::from_secs(10)),
        )
        .expect("the service answers");
    assert!(reply.ok);

    let slow = client
        .send_request_blocking(
            &MoveRequest {
                x: 1,
                speed: 0.5,
                ..MoveRequest::default()
            },
            Some(Duration::from_secs(10)),
        )
        .expect("the service answers");
    assert!(!slow.ok);

    // A remote exception is the code in the header of the reply type.
    let refused = client.send_request_blocking(
        &MoveRequest {
            x: -1,
            ..MoveRequest::default()
        },
        Some(Duration::from_secs(10)),
    );
    assert_eq!(
        refused.unwrap_err(),
        zerodds_rpc::error::RpcError::RemoteException(
            RemoteExceptionCode::InvalidArgument.as_u32()
        )
    );

    stop.store(true, Ordering::Release);
    ticking.join().unwrap();
}
