// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! ZeroDDS async DCPS API (zerodds-async-1.0).
//!
//! Crate `zerodds-dcps-async`. Safety classification: **STANDARD**.
//!
//! Runtime-agnostic async wrappers around the DCPS sync API. Newtypes
//! share the internal `Arc<...>` with the sync counterparts — no
//! state duplicate, no performance overhead.
//!
//! # Example
//!
//! ```ignore
//! use zerodds_dcps_async::AsyncDomainParticipantFactory;
//! use futures_core::Stream;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() {
//!     let factory = AsyncDomainParticipantFactory::instance();
//!     let participant = factory.create_participant_offline(0);
//!     // ... topic + reader + writer like sync.
//!     let writer = /* ... */;
//!     let reader = /* ... */;
//!
//!     // write & take run without thread block.
//!     writer.write(&sample).await.unwrap();
//!     let samples = reader.take(Duration::from_secs(1)).await.unwrap();
//! }
//! ```
//!
//! Spec: `docs/specs/zerodds-async-1.0.md`.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![forbid(unsafe_code)]

extern crate alloc;

mod factory;
mod participant;
mod publisher;
mod reader;
mod subscriber;
mod writer;

pub use factory::AsyncDomainParticipantFactory;
pub use participant::AsyncDomainParticipant;
pub use publisher::AsyncPublisher;
pub use reader::{
    AsyncDataReader, DataAvailableStream, PublicationMatchedStream, SampleInfoStream, SampleStream,
};
pub use subscriber::AsyncSubscriber;
pub use writer::AsyncDataWriter;

// Re-exports of the sync types the caller still needs.
pub use zerodds_dcps::status::SubscriptionMatchedStatus;
pub use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DdsType, DomainParticipantQos, InstanceHandle,
    PublisherQos, Result, SubscriberQos, Topic, TopicQos,
};

/// Runtime-agnostic sleep helper. Polls until deadline; wakeup
/// comes from a detached thread (no hard tokio dependency).
///
/// With `--features tokio-glue`, tokio::time::sleep is used,
/// where the wakeup comes from the Tokio reactor — no thread-spawn overhead.
#[cfg(feature = "std")]
pub(crate) async fn yield_for(d: core::time::Duration) {
    #[cfg(feature = "tokio-glue")]
    {
        tokio::time::sleep(d).await;
    }
    #[cfg(not(feature = "tokio-glue"))]
    {
        // Default path: detached-thread sleep + waker.
        SleepFuture::new(d).await
    }
}

#[cfg(all(feature = "std", not(feature = "tokio-glue")))]
struct SleepFuture {
    deadline: std::time::Instant,
    spawned: bool,
}

#[cfg(all(feature = "std", not(feature = "tokio-glue")))]
impl SleepFuture {
    fn new(d: core::time::Duration) -> Self {
        Self {
            deadline: std::time::Instant::now() + d,
            spawned: false,
        }
    }
}

#[cfg(all(feature = "std", not(feature = "tokio-glue")))]
impl core::future::Future for SleepFuture {
    type Output = ();

    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<()> {
        if std::time::Instant::now() >= self.deadline {
            return core::task::Poll::Ready(());
        }
        if !self.spawned {
            self.spawned = true;
            let waker = cx.waker().clone();
            let deadline = self.deadline;
            std::thread::spawn(move || {
                let now = std::time::Instant::now();
                if deadline > now {
                    std::thread::sleep(deadline - now);
                }
                waker.wake();
            });
        }
        core::task::Poll::Pending
    }
}
