// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! ZeroDDS async-DCPS-API (zerodds-async-1.0).
//!
//! Crate `zerodds-dcps-async`. Safety classification: **STANDARD**.
//!
//! Runtime-agnostische async-Wrappers um die DCPS-Sync-API. Newtypes
//! teilen den internen `Arc<...>` mit den Sync-Pendants — kein
//! State-Duplikat, kein Performance-Overhead.
//!
//! # Beispiel
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
//!     // ... topic + reader + writer wie sync.
//!     let writer = /* ... */;
//!     let reader = /* ... */;
//!
//!     // write & take laufen ohne Thread-Block.
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
pub use reader::{AsyncDataReader, DataAvailableStream, PublicationMatchedStream, SampleStream};
pub use subscriber::AsyncSubscriber;
pub use writer::AsyncDataWriter;

// Re-Exports der Sync-Types die der Caller weiterhin braucht.
pub use zerodds_dcps::status::SubscriptionMatchedStatus;
pub use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DdsType, DomainParticipantQos, InstanceHandle,
    PublisherQos, Result, SubscriberQos, Topic, TopicQos,
};

/// Runtime-agnostischer Sleep-Helper. Polled bis Deadline; Wakeup
/// kommt durch einen detached thread (kein tokio-Hard-Dep).
///
/// Mit `--features tokio-glue` wird tokio::time::sleep verwendet,
/// das Wakeup vom Tokio-Reactor kommt — kein Thread-Spawn-Overhead.
#[cfg(feature = "std")]
pub(crate) async fn yield_for(d: core::time::Duration) {
    #[cfg(feature = "tokio-glue")]
    {
        tokio::time::sleep(d).await;
    }
    #[cfg(not(feature = "tokio-glue"))]
    {
        // Default-Pfad: detached-thread-Sleep + Waker.
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
