// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Iceoryx2 bridge — parallel pub/sub API for
//! [Eclipse iceoryx2](https://github.com/eclipse-iceoryx/iceoryx2)
//! (Bosch / Apex.AI), as the optional feature `iceoryx2-bridge`.
//!
//! # Why a separate API
//!
//! `zerodds-flatdata-1.0` §8/§9 specifies a
//! `loan/commit/receive` pub/sub form. The built-in
//! [`SlotBackend`](crate::SlotBackend) trait for the in-memory
//! and POSIX backends exposes a **random-access slot-pool**
//! form; iceoryx2, by contrast, is FIFO pub/sub with internal
//! refcount management and has no random slot reads. The two
//! models do not run on the same trait, so the
//! bridge is its own API:
//!
//! - [`Iceoryx2Publisher<T>`] — `loan` + `send` (maps directly to
//!   `publisher.loan_slice_uninit` + `sample.send`).
//! - [`Iceoryx2Subscriber<T>`] — `receive` (maps to
//!   `subscriber.receive`).
//!
//! The spec §6.1 type-hash cross-validation is carried through at
//! service build time (the service name encodes the hash).
//!
//! # Example
//!
//! ```no_run
//! # #[cfg(feature = "iceoryx2-bridge")] {
//! use zerodds_flatdata::{FlatStruct, Iceoryx2Publisher, Iceoryx2Subscriber};
//!
//! #[derive(Copy, Clone, Debug, PartialEq, Default)]
//! #[repr(C)]
//! struct Pose { x: f64, y: f64, z: f64 }
//! unsafe impl FlatStruct for Pose {
//!     const TYPE_HASH: [u8; 16] = [0xAA; 16];
//! }
//!
//! let pubr = Iceoryx2Publisher::<Pose>::create("zerodds/Pose").unwrap();
//! let subr = Iceoryx2Subscriber::<Pose>::create("zerodds/Pose").unwrap();
//!
//! pubr.send(&Pose { x: 1.0, y: 2.0, z: 3.0 }).unwrap();
//! while let Some(p) = subr.receive().unwrap() {
//!     assert_eq!(p, Pose { x: 1.0, y: 2.0, z: 3.0 });
//!     break;
//! }
//! # }
//! ```

use core::marker::PhantomData;

use iceoryx2::node::{Node, NodeBuilder};
use iceoryx2::port::listener::Listener;
use iceoryx2::port::notifier::Notifier;
use iceoryx2::port::publisher::Publisher;
use iceoryx2::port::subscriber::Subscriber;
use iceoryx2::service::ipc::Service;
use iceoryx2::service::port_factory::publish_subscribe::PortFactory;

/// Thread-safe cross-process service. The `Raw*` ports below use this so they
/// are `Send + Sync` and can live in a process-global registry (the C-API
/// `Iceoryx` delivery mode keys them by `(runtime, eid)`). The typed
/// `Iceoryx2Publisher`/`Subscriber` keep `ipc::Service` (single-thread ports).
use iceoryx2::service::ipc_threadsafe::Service as TsService;

use crate::FlatStruct;

/// Error class for the iceoryx2 bridge.
#[derive(Debug)]
#[non_exhaustive]
pub enum Iceoryx2Error {
    /// Service build / connect error.
    Service(alloc::string::String),
    /// Publisher build error.
    Publisher(alloc::string::String),
    /// Subscriber build error.
    Subscriber(alloc::string::String),
    /// Loan / send error.
    Send(alloc::string::String),
    /// Receive error.
    Receive(alloc::string::String),
    /// Type-hash mismatch between publisher and subscriber (spec §6.1).
    TypeHashMismatch,
    /// Service name empty.
    EmptyServiceName,
}

impl core::fmt::Display for Iceoryx2Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Service(e) => write!(f, "iceoryx2 service: {e}"),
            Self::Publisher(e) => write!(f, "iceoryx2 publisher: {e}"),
            Self::Subscriber(e) => write!(f, "iceoryx2 subscriber: {e}"),
            Self::Send(e) => write!(f, "iceoryx2 send: {e}"),
            Self::Receive(e) => write!(f, "iceoryx2 receive: {e}"),
            Self::TypeHashMismatch => f.write_str("iceoryx2 service type-hash mismatch"),
            Self::EmptyServiceName => f.write_str("iceoryx2 service name must be non-empty"),
        }
    }
}

impl std::error::Error for Iceoryx2Error {}

/// The service name is combined with `T::TYPE_HASH` into a
/// collision-safe identifier, so that schema drift
/// (pub and sub having different `T` layouts) is rejected on the
/// iceoryx2 side in the service match (spec §6.1).
fn build_service_name<T: FlatStruct>(base: &str) -> alloc::string::String {
    use core::fmt::Write;
    let mut s = alloc::string::String::with_capacity(base.len() + 1 + 32);
    s.push_str(base);
    s.push('#');
    for b in T::TYPE_HASH.iter() {
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

/// Iceoryx2 pub/sub service wrapper. Holds the Node + ServiceFactory;
/// the lifetime applies for the lifetime of the `Iceoryx2Publisher`/
/// `Iceoryx2Subscriber` owner object.
struct ServiceCtx<T: FlatStruct> {
    _node: Node<Service>,
    factory: PortFactory<Service, [u8], ()>,
    _t: PhantomData<fn() -> T>,
}

impl<T: FlatStruct> ServiceCtx<T> {
    fn open_or_create(base: &str) -> Result<Self, Iceoryx2Error> {
        if base.is_empty() {
            return Err(Iceoryx2Error::EmptyServiceName);
        }
        let node = NodeBuilder::new()
            .create::<Service>()
            .map_err(|e| Iceoryx2Error::Service(alloc::format!("{e:?}")))?;
        let svc_name = build_service_name::<T>(base);
        let svc_id = svc_name
            .as_str()
            .try_into()
            .map_err(|e| Iceoryx2Error::Service(alloc::format!("invalid service name: {e:?}")))?;
        let factory = node
            .service_builder(&svc_id)
            .publish_subscribe::<[u8]>()
            .open_or_create()
            .map_err(|e| Iceoryx2Error::Service(alloc::format!("{e:?}")))?;
        Ok(Self {
            _node: node,
            factory,
            _t: PhantomData,
        })
    }
}

/// Iceoryx2 publisher for a `FlatStruct` type. Writes samples
/// directly into iceoryx2 SHM via `loan_slice_uninit` + `send` (spec
/// §8.1 zerodds-flatdata-1.0).
pub struct Iceoryx2Publisher<T: FlatStruct> {
    ctx: ServiceCtx<T>,
    publisher: Publisher<Service, [u8], ()>,
}

impl<T: FlatStruct> Iceoryx2Publisher<T> {
    /// Creates a publisher for the given service name.
    /// The iceoryx2 service identifier is assembled internally from
    /// `service_base#<hex(TYPE_HASH)>` — subscribers
    /// must use the same `T`, otherwise the iceoryx2
    /// service match does not take effect.
    ///
    /// # Errors
    /// `Service` on a service-build error; `Publisher` on a
    /// publisher-build error; `EmptyServiceName` on an empty name.
    pub fn create(service_name: &str) -> Result<Self, Iceoryx2Error> {
        let ctx = ServiceCtx::<T>::open_or_create(service_name)?;
        let publisher = ctx
            .factory
            .publisher_builder()
            .initial_max_slice_len(T::WIRE_SIZE)
            .create()
            .map_err(|e| Iceoryx2Error::Publisher(alloc::format!("{e:?}")))?;
        Ok(Self { ctx, publisher })
    }

    /// Spec §8.1 `write_flat` — copies the sample into a loaned
    /// iceoryx2 slot and sends it. Zero-copy with respect to the IPC path
    /// (no `memcpy` to the kernel and back), a single
    /// `core::ptr::copy_nonoverlapping` from stack/heap into the SHM.
    ///
    /// # Errors
    /// `Send` on loan/send errors (e.g. when the subscriber buffer
    /// is full and `UnableToDeliverStrategy::Block` would be active — the
    /// default is `DiscardData`, since the spec §4.2 cross-host
    /// path covers us in parallel).
    pub fn send(&self, sample: &T) -> Result<(), Iceoryx2Error> {
        let bytes = sample.as_bytes();
        let loan = self
            .publisher
            .loan_slice_uninit(bytes.len())
            .map_err(|e| Iceoryx2Error::Send(alloc::format!("loan: {e:?}")))?;
        let initialized = loan.write_from_slice(bytes);
        initialized
            .send()
            .map_err(|e| Iceoryx2Error::Send(alloc::format!("send: {e:?}")))?;
        Ok(())
    }

    /// Service name identifier (with `TYPE_HASH` suffix), e.g. for
    /// diagnostics or logging.
    #[must_use]
    pub fn service_name(&self) -> alloc::string::String {
        // Can only be reproduced — iceoryx2 does not return the name
        // directly. We cannot separate TYPE_HASH from the caller base,
        // so we return a reproduced form.
        // Sufficient for diagnostic purposes.
        let _ = &self.ctx;
        alloc::format!("zerodds-flatdata#TYPE_HASH={:02x?}", T::TYPE_HASH)
    }
}

/// Byte-oriented iceoryx2 publisher — no `FlatStruct` type parameter.
///
/// The service name is used **verbatim** (no `TYPE_HASH` suffix): cross-stack
/// peers must agree on the name and on the payload layout out of band. This
/// powers the C-API `Iceoryx` delivery mode, where the FFI is byte-oriented and
/// has no Rust type to derive a hash from. For typed, hash-namespaced services
/// use [`Iceoryx2Publisher`] instead.
pub struct RawIceoryx2Publisher {
    _node: Node<TsService>,
    publisher: Publisher<TsService, [u8], ()>,
    /// Event notifier on the same service name — pinged on every `send` so a
    /// blocking subscriber ([`RawIceoryx2Subscriber::wait`]) wakes event-driven.
    notifier: Notifier<TsService>,
}

impl RawIceoryx2Publisher {
    /// Creates a publisher on `service_name` with a maximum payload of
    /// `max_len` bytes per sample.
    ///
    /// # Errors
    /// `EmptyServiceName` on an empty name; `Service`/`Publisher` on build
    /// errors.
    pub fn create(service_name: &str, max_len: usize) -> Result<Self, Iceoryx2Error> {
        if service_name.is_empty() {
            return Err(Iceoryx2Error::EmptyServiceName);
        }
        let node = NodeBuilder::new()
            .create::<TsService>()
            .map_err(|e| Iceoryx2Error::Service(alloc::format!("{e:?}")))?;
        let svc_id = service_name
            .try_into()
            .map_err(|e| Iceoryx2Error::Service(alloc::format!("invalid service name: {e:?}")))?;
        let factory = node
            .service_builder(&svc_id)
            .publish_subscribe::<[u8]>()
            .open_or_create()
            .map_err(|e| Iceoryx2Error::Service(alloc::format!("{e:?}")))?;
        let publisher = factory
            .publisher_builder()
            .initial_max_slice_len(max_len)
            .create()
            .map_err(|e| Iceoryx2Error::Publisher(alloc::format!("{e:?}")))?;
        // Event service on the same name (a service can host both the
        // publish_subscribe and the event messaging pattern) for blocking-wait.
        let event = node
            .service_builder(&svc_id)
            .event()
            .open_or_create()
            .map_err(|e| Iceoryx2Error::Service(alloc::format!("event: {e:?}")))?;
        let notifier = event
            .notifier_builder()
            .create()
            .map_err(|e| Iceoryx2Error::Publisher(alloc::format!("notifier: {e:?}")))?;
        Ok(Self {
            _node: node,
            publisher,
            notifier,
        })
    }

    /// Loans an iceoryx2 slot, writes `bytes` into it and sends. Zero-copy with
    /// respect to the IPC path (a single copy into the SHM slot, no kernel
    /// round-trip).
    ///
    /// # Errors
    /// `Send` on loan/send errors.
    pub fn send(&self, bytes: &[u8]) -> Result<(), Iceoryx2Error> {
        let loan = self
            .publisher
            .loan_slice_uninit(bytes.len())
            .map_err(|e| Iceoryx2Error::Send(alloc::format!("loan: {e:?}")))?;
        let initialized = loan.write_from_slice(bytes);
        initialized
            .send()
            .map_err(|e| Iceoryx2Error::Send(alloc::format!("send: {e:?}")))?;
        // Wake any blocking subscriber (best-effort — delivery already happened).
        let _ = self.notifier.notify();
        Ok(())
    }
}

/// Byte-oriented iceoryx2 subscriber — the receive counterpart to
/// [`RawIceoryx2Publisher`]. `receive` copies the sample payload out into an
/// owned buffer (the byte FFI hands the caller an owned region anyway).
pub struct RawIceoryx2Subscriber {
    _node: Node<TsService>,
    subscriber: Subscriber<TsService, [u8], ()>,
    /// Event listener on the same service name — blocks in [`Self::wait`] until
    /// the publisher's notifier pings (a sample was sent).
    listener: Listener<TsService>,
}

impl RawIceoryx2Subscriber {
    /// Creates a subscriber on `service_name`.
    ///
    /// # Errors
    /// `EmptyServiceName` on an empty name; `Service`/`Subscriber` on build
    /// errors.
    pub fn create(service_name: &str) -> Result<Self, Iceoryx2Error> {
        if service_name.is_empty() {
            return Err(Iceoryx2Error::EmptyServiceName);
        }
        let node = NodeBuilder::new()
            .create::<TsService>()
            .map_err(|e| Iceoryx2Error::Service(alloc::format!("{e:?}")))?;
        let svc_id = service_name
            .try_into()
            .map_err(|e| Iceoryx2Error::Service(alloc::format!("invalid service name: {e:?}")))?;
        let factory = node
            .service_builder(&svc_id)
            .publish_subscribe::<[u8]>()
            .open_or_create()
            .map_err(|e| Iceoryx2Error::Service(alloc::format!("{e:?}")))?;
        let subscriber = factory
            .subscriber_builder()
            .create()
            .map_err(|e| Iceoryx2Error::Subscriber(alloc::format!("{e:?}")))?;
        let event = node
            .service_builder(&svc_id)
            .event()
            .open_or_create()
            .map_err(|e| Iceoryx2Error::Service(alloc::format!("event: {e:?}")))?;
        let listener = event
            .listener_builder()
            .create()
            .map_err(|e| Iceoryx2Error::Subscriber(alloc::format!("listener: {e:?}")))?;
        Ok(Self {
            _node: node,
            subscriber,
            listener,
        })
    }

    /// Blocks until the publisher signals a new sample or `timeout` elapses
    /// (event-driven via the iceoryx2 listener — no busy-poll). Returns `true` if
    /// woken by a notification. A spurious `true` is harmless: the caller then
    /// `receive`s and finds nothing.
    #[must_use]
    pub fn wait(&self, timeout: core::time::Duration) -> bool {
        matches!(self.listener.timed_wait_one(timeout), Ok(Some(_)))
    }

    /// Receives the next available sample (FIFO), copied into an owned buffer.
    /// `Ok(None)` means "no new sample", not end of stream.
    ///
    /// # Errors
    /// `Receive` on iceoryx2 receive errors.
    pub fn receive(&self) -> Result<Option<alloc::vec::Vec<u8>>, Iceoryx2Error> {
        let Some(sample) = self
            .subscriber
            .receive()
            .map_err(|e| Iceoryx2Error::Receive(alloc::format!("{e:?}")))?
        else {
            return Ok(None);
        };
        Ok(Some(sample.payload().to_vec()))
    }
}

/// Iceoryx2 subscriber. Receives samples via FIFO `receive`
/// (spec §9.1 zerodds-flatdata-1.0).
pub struct Iceoryx2Subscriber<T: FlatStruct> {
    _ctx: ServiceCtx<T>,
    subscriber: Subscriber<Service, [u8], ()>,
}

impl<T: FlatStruct> Iceoryx2Subscriber<T> {
    /// Creates a subscriber for the given service name.
    ///
    /// # Errors
    /// `Service`/`Subscriber` on a service- or subscriber-build
    /// error.
    pub fn create(service_name: &str) -> Result<Self, Iceoryx2Error> {
        let ctx = ServiceCtx::<T>::open_or_create(service_name)?;
        let subscriber = ctx
            .factory
            .subscriber_builder()
            .create()
            .map_err(|e| Iceoryx2Error::Subscriber(alloc::format!("{e:?}")))?;
        Ok(Self {
            _ctx: ctx,
            subscriber,
        })
    }

    /// Spec §9.1 `read_flat` — receives the next available
    /// sample (FIFO). `Ok(None)` means "no new sample", not
    /// end of stream.
    ///
    /// Spec §6.1 type-hash cross-validation has already happened at the
    /// service-build match (the service name encodes
    /// `TYPE_HASH`); a subscriber with a different `T` would open a
    /// different iceoryx2 service and receive no samples.
    /// We additionally validate the slice length against
    /// `T::WIRE_SIZE`.
    ///
    /// # Errors
    /// `Receive` on iceoryx2 receive errors. A sample with an
    /// invalid slice length is discarded (logged + None).
    pub fn receive(&self) -> Result<Option<T>, Iceoryx2Error> {
        let Some(sample) = self
            .subscriber
            .receive()
            .map_err(|e| Iceoryx2Error::Receive(alloc::format!("{e:?}")))?
        else {
            return Ok(None);
        };
        let bytes: &[u8] = sample.payload();
        if bytes.len() < T::WIRE_SIZE {
            // Schema drift / truncation — spec §6.1.
            return Ok(None);
        }
        // SAFETY: the WIRE_SIZE check above + the service-name TYPE_HASH
        // match guarantee layout consistency.
        let value = unsafe { T::from_bytes_unchecked(bytes) };
        Ok(Some(value))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[derive(Copy, Clone, Debug, PartialEq, Default)]
    #[repr(C)]
    struct Pose {
        x: f64,
        y: f64,
        z: f64,
    }

    // SAFETY: repr(C) + Copy + 'static + all fields primitive.
    unsafe impl FlatStruct for Pose {
        const TYPE_HASH: [u8; 16] = [0xCC; 16];
    }

    #[derive(Copy, Clone, Debug, PartialEq, Default)]
    #[repr(C)]
    struct OtherPose {
        x: f64,
        y: f64,
        z: f64,
    }

    // SAFETY: repr(C) + Copy + 'static + all fields primitive.
    unsafe impl FlatStruct for OtherPose {
        const TYPE_HASH: [u8; 16] = [0xDD; 16]; // different hash
    }

    fn unique_service_name(suffix: &str) -> alloc::string::String {
        // iceoryx2 lives in the /dev/shm namespace; use a unique
        // service per test so that test ordering and parallel
        // execution do not conflict.
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        alloc::format!("zerodds_flatdata_test_{pid}_{n}_{suffix}")
    }

    #[test]
    fn publisher_subscriber_roundtrip_same_process() {
        let svc = unique_service_name("rt");
        let publisher = Iceoryx2Publisher::<Pose>::create(&svc).expect("publisher");
        let subscriber = Iceoryx2Subscriber::<Pose>::create(&svc).expect("subscriber");

        let p = Pose {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        publisher.send(&p).expect("send");

        // iceoryx2 is synchronous on the same-process path — receive
        // should deliver the just-sent sample immediately.
        let got = subscriber.receive().expect("receive");
        assert_eq!(got, Some(p));
    }

    #[test]
    fn subscriber_returns_none_without_publisher_data() {
        let svc = unique_service_name("none");
        let subscriber = Iceoryx2Subscriber::<Pose>::create(&svc).expect("subscriber");
        let got = subscriber.receive().expect("receive");
        assert!(got.is_none());
    }

    #[test]
    fn type_hash_mismatch_isolates_services() {
        // Spec §6.1: schema drift → different iceoryx2
        // service name → pub and sub do not match, sub receives
        // no samples.
        let svc = unique_service_name("drift");
        let publisher = Iceoryx2Publisher::<Pose>::create(&svc).expect("publisher");
        // Subscriber with a different hash → its own service.
        let other_subscriber =
            Iceoryx2Subscriber::<OtherPose>::create(&svc).expect("other subscriber");

        publisher
            .send(&Pose {
                x: 9.0,
                y: 9.0,
                z: 9.0,
            })
            .expect("send");

        let got = other_subscriber.receive().expect("receive");
        assert!(got.is_none(), "other-typed subscriber must not see samples");
    }

    #[test]
    fn empty_service_name_rejected() {
        let res = Iceoryx2Publisher::<Pose>::create("");
        assert!(matches!(res, Err(Iceoryx2Error::EmptyServiceName)));
    }

    #[test]
    fn raw_byte_publisher_subscriber_roundtrip() {
        let svc = unique_service_name("raw");
        let publisher = RawIceoryx2Publisher::create(&svc, 64).expect("raw publisher");
        let subscriber = RawIceoryx2Subscriber::create(&svc).expect("raw subscriber");

        let payload: [u8; 8] = [0x2a, 0, 0, 0, 0xFF, 0xEE, 0xDD, 0xCC];
        publisher.send(&payload).expect("raw send");

        let got = subscriber.receive().expect("raw receive");
        assert_eq!(got.as_deref(), Some(&payload[..]));
    }

    #[test]
    fn raw_empty_service_name_rejected() {
        assert!(matches!(
            RawIceoryx2Publisher::create("", 16),
            Err(Iceoryx2Error::EmptyServiceName)
        ));
        assert!(matches!(
            RawIceoryx2Subscriber::create(""),
            Err(Iceoryx2Error::EmptyServiceName)
        ));
    }
}
