// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-rs` with `zerodds-dcps-async`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps-async` (`src/lib.rs`): the whole
//! crate, re-exported as `zerodds_rs::aio` (`pub use zerodds_dcps_async as
//! aio`).
//!
//! What it relies on: the re-exported items are the `zerodds-dcps-async`
//! items themselves; an offline participant keeps its domain and hands its
//! sync participant back (§1); a `take` without a writer ends with its
//! timeout, empty (§2.2), on any executor (§3).
//!
//! Oracles: the identity of the re-exported items (`TypeId`),
//! zerodds-async-1.0 §1 (each async type wraps its sync type, `as_sync`),
//! §2.2 (`take(timeout)` resolves when the timeout runs out) and §3 (no
//! runtime of its own: a thread-parking executor of `std` drives it). No
//! socket is opened.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use core::any::TypeId;
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake};
use std::thread::Thread;
use std::time::Duration;

use zerodds_rs::RawBytes;
use zerodds_rs::aio::{
    AsyncDomainParticipantFactory, DataReaderQos, PublisherQos, SubscriberQos, TopicQos,
};

#[test]
fn the_reexported_items_are_the_async_crate_s() {
    assert_eq!(
        TypeId::of::<zerodds_rs::aio::AsyncDomainParticipantFactory>(),
        TypeId::of::<zerodds_dcps_async::AsyncDomainParticipantFactory>()
    );
    assert_eq!(
        TypeId::of::<zerodds_rs::aio::AsyncDataWriter<RawBytes>>(),
        TypeId::of::<zerodds_dcps_async::AsyncDataWriter<RawBytes>>()
    );
}

/// zerodds-async-1.0 §1: an offline participant keeps its domain and
/// hands its sync participant back.
#[test]
fn async_1_the_sync_participant_behind_the_async_one() {
    let participant = AsyncDomainParticipantFactory::instance().create_participant_offline(7);
    assert_eq!(participant.domain_id(), 7);
    assert_eq!(participant.as_sync().domain_id(), 7);
}

/// zerodds-async-1.0 §2.2: `take(timeout)` with no writer resolves when the
/// timeout runs out, without a sample.
#[test]
fn async_2_2_a_take_without_a_writer_ends_with_its_timeout() {
    let participant = AsyncDomainParticipantFactory::instance().create_participant_offline(0);
    let topic = participant
        .create_topic::<RawBytes>("ContractAio", TopicQos::default())
        .unwrap();
    let _publisher = participant.create_publisher(PublisherQos::default());
    let reader = participant
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .unwrap();
    let started = std::time::Instant::now();
    let taken = block_on(reader.take(Duration::from_millis(100)));
    assert!(started.elapsed() >= Duration::from_millis(100));
    match taken {
        Ok(samples) => assert!(samples.is_empty()),
        Err(e) => assert!(matches!(e, zerodds_rs::aio::DdsError::Timeout), "{e:?}"),
    }
}

/// A `std`-only executor: polls the future, parks the thread until its
/// waker unparks it.
fn block_on<F: Future>(future: F) -> F::Output {
    struct Unpark(Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Arc::new(Unpark(std::thread::current())).into();
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::park();
    }
}
