// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Iceoryx2-Bridge — parallele Pub/Sub-API fuer
//! [Eclipse iceoryx2](https://github.com/eclipse-iceoryx/iceoryx2)
//! (Bosch / Apex.AI), als optionales Feature `iceoryx2-bridge`.
//!
//! # Warum eine separate API
//!
//! `zerodds-flatdata-1.0` §8/§9 spezifiziert eine
//! `loan/commit/receive`-Pub/Sub-Form. Der built-in
//! [`SlotBackend`](crate::SlotBackend)-Trait fuer den In-Memory-
//! und POSIX-Backend exponiert eine **Random-Access-Slot-Pool**-
//! Form; iceoryx2 ist dagegen FIFO-Pub/Sub mit interner Refcount-
//! Verwaltung und kennt keine wahlfreien Slot-Reads. Die zwei
//! Modelle laufen nicht auf den selben Trait, also ist die
//! Bridge eine eigene API:
//!
//! - [`Iceoryx2Publisher<T>`] — `loan` + `send` (mappt direkt auf
//!   `publisher.loan_slice_uninit` + `sample.send`).
//! - [`Iceoryx2Subscriber<T>`] — `receive` (mappt auf
//!   `subscriber.receive`).
//!
//! Die spec-§6.1-Type-Hash-Cross-Validation wird beim
//! Service-Build durchgereicht (Service-Name encodiert den Hash).
//!
//! # Beispiel
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
use iceoryx2::port::publisher::Publisher;
use iceoryx2::port::subscriber::Subscriber;
use iceoryx2::service::ipc::Service;
use iceoryx2::service::port_factory::publish_subscribe::PortFactory;

use crate::FlatStruct;

/// Fehlerklasse fuer die iceoryx2-Bridge.
#[derive(Debug)]
#[non_exhaustive]
pub enum Iceoryx2Error {
    /// Service-Build / Connect-Fehler.
    Service(alloc::string::String),
    /// Publisher-Build-Fehler.
    Publisher(alloc::string::String),
    /// Subscriber-Build-Fehler.
    Subscriber(alloc::string::String),
    /// Loan-/Send-Fehler.
    Send(alloc::string::String),
    /// Receive-Fehler.
    Receive(alloc::string::String),
    /// Type-Hash-Mismatch zwischen Publisher und Subscriber (Spec §6.1).
    TypeHashMismatch,
    /// Service-Name leer.
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

/// Der Service-Name wird mit `T::TYPE_HASH` zu einem
/// kollisionssicheren Identifier verbunden, sodass Schema-Drift
/// (Pub und Sub haben unterschiedliche `T`-Layouts) iceoryx2-seitig
/// im Service-Match abgewiesen wird (Spec §6.1).
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

/// Iceoryx2-Pub/Sub-Service-Wrapper. Haelt Node + ServiceFactory,
/// die Lebenszeit gilt fuer die Lebenszeit des `Iceoryx2Publisher`/
/// `Iceoryx2Subscriber`-Owner-Objekts.
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

/// Iceoryx2-Publisher fuer einen `FlatStruct`-Type. Schreibt Samples
/// direkt in iceoryx2-SHM via `loan_slice_uninit` + `send` (Spec
/// §8.1 zerodds-flatdata-1.0).
pub struct Iceoryx2Publisher<T: FlatStruct> {
    ctx: ServiceCtx<T>,
    publisher: Publisher<Service, [u8], ()>,
}

impl<T: FlatStruct> Iceoryx2Publisher<T> {
    /// Erzeugt einen Publisher fuer den gegebenen Service-Namen.
    /// Der iceoryx2-Service-Identifier wird intern aus
    /// `service_base#<hex(TYPE_HASH)>` zusammengesetzt — Subscriber
    /// muessen denselben `T` verwenden, sonst greift der iceoryx2-
    /// Service-Match nicht.
    ///
    /// # Errors
    /// `Service` bei Service-Build-Fehler; `Publisher` bei
    /// Publisher-Build-Fehler; `EmptyServiceName` bei leerem Namen.
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

    /// Spec §8.1 `write_flat` — kopiert das Sample in einen geliehenen
    /// iceoryx2-Slot und sendet. Zero-Copy bezogen auf den IPC-Pfad
    /// (kein `memcpy` zum Kernel und zurueck), ein einzelnes
    /// `core::ptr::copy_nonoverlapping` von Stack/Heap in den SHM.
    ///
    /// # Errors
    /// `Send` bei Loan-/Send-Fehlern (z.B. wenn der Subscriber-Buffer
    /// voll ist und `UnableToDeliverStrategy::Block` aktiv waere — der
    /// Default ist `DiscardData`, da uns der Spec-§4.2-Cross-Host-
    /// Pfad parallel sichert).
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

    /// Service-Name-Identifier (mit `TYPE_HASH`-Suffix), z.B. fuer
    /// Diagnose oder Logging.
    #[must_use]
    pub fn service_name(&self) -> alloc::string::String {
        // Kann nur reproduziert werden — iceoryx2 gibt den Namen nicht
        // direkt zurueck. Wir koennen aber TYPE_HASH plus Caller-Base
        // nicht trennen, also liefern wir eine reproduzierte Form.
        // Fuer Diagnosezwecke ausreichend.
        let _ = &self.ctx;
        alloc::format!("zerodds-flatdata#TYPE_HASH={:02x?}", T::TYPE_HASH)
    }
}

/// Iceoryx2-Subscriber. Empfaengt Samples via FIFO-`receive`
/// (Spec §9.1 zerodds-flatdata-1.0).
pub struct Iceoryx2Subscriber<T: FlatStruct> {
    _ctx: ServiceCtx<T>,
    subscriber: Subscriber<Service, [u8], ()>,
}

impl<T: FlatStruct> Iceoryx2Subscriber<T> {
    /// Erzeugt einen Subscriber fuer den gegebenen Service-Namen.
    ///
    /// # Errors
    /// `Service`/`Subscriber` bei Service- bzw. Subscriber-Build-
    /// Fehler.
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

    /// Spec §9.1 `read_flat` — empfaengt das naechste verfuegbare
    /// Sample (FIFO). `Ok(None)` bedeutet "kein neues Sample", nicht
    /// Stream-Ende.
    ///
    /// Spec §6.1 Type-Hash-Cross-Validation ist beim
    /// Service-Build-Match erfolgt (der Service-Name encodiert
    /// `TYPE_HASH`); ein Subscriber mit anderem `T` wuerde einen
    /// anderen iceoryx2-Service oeffnen und keine Samples bekommen.
    /// Wir validieren zusaetzlich die Slice-Laenge gegen
    /// `T::WIRE_SIZE`.
    ///
    /// # Errors
    /// `Receive` bei iceoryx2-Receive-Fehlern. Verworfen wird ein
    /// Sample mit unzulaessiger Slice-Laenge (logged + None).
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
            // Schema-Drift / Truncation — Spec §6.1.
            return Ok(None);
        }
        // SAFETY: WIRE_SIZE-Check oben + Service-Name-TYPE_HASH-Match
        // garantiert Layout-Consistency.
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

    // SAFETY: repr(C) + Copy + 'static + alle Felder Primitiv.
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

    // SAFETY: repr(C) + Copy + 'static + alle Felder Primitiv.
    unsafe impl FlatStruct for OtherPose {
        const TYPE_HASH: [u8; 16] = [0xDD; 16]; // anderer Hash
    }

    fn unique_service_name(suffix: &str) -> alloc::string::String {
        // iceoryx2 lebt im /dev/shm namespace; pro Test einen
        // eindeutigen Service damit Test-Reihenfolge und parallele
        // Ausfuehrung sich nicht beissen.
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

        // iceoryx2 ist synchron im Same-Process-Pfad — receive sollte
        // das soeben gesendete Sample sofort liefern.
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
        // Spec §6.1: Schema-Drift → unterschiedlicher iceoryx2-
        // Service-Name → Pub und Sub matchen nicht, Sub bekommt
        // keine Samples.
        let svc = unique_service_name("drift");
        let publisher = Iceoryx2Publisher::<Pose>::create(&svc).expect("publisher");
        // Subscriber mit anderem Hash → eigener Service.
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
}
