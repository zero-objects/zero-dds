// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `LayeredUserTransport` — a preference-ordered multi-transport for user
//! traffic.
//!
//! A participant can run several transports at once — e.g. shared memory for
//! same-host peers and UDP for cross-host peers — behind a single
//! [`Transport`] handle:
//!
//! - **send**: the datagram is routed to the first sub-transport whose locator
//!   kind matches the destination. Construction order is preference order, so
//!   list the fast/local transport first (SHM) and the fallback last (UDP). A
//!   peer reachable only over UDP is sent over UDP; a same-host peer that
//!   advertised an SHM locator is sent over SHM — automatically.
//! - **recv**: datagrams from every sub-transport are merged into one stream
//!   (one recv thread per leg pushes into a shared queue).
//! - **advertising**: [`Transport::local_locators`] returns every leg's locator
//!   so discovery announces all of them and peers can pick whichever they can
//!   reach.

use alloc::format;
use alloc::vec::Vec;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zerodds_rtps::wire_types::{Locator, LocatorKind};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};

/// One sub-transport plus the locator kind it serves.
struct Leg {
    kind: LocatorKind,
    transport: Arc<dyn Transport + Send + Sync>,
}

/// Multiplexes user traffic over an ordered set of sub-transports.
/// See the module docs.
pub struct LayeredUserTransport {
    legs: Vec<Leg>,
    inbound: Arc<(Mutex<VecDeque<ReceivedDatagram>>, Condvar)>,
    stop: Arc<AtomicBool>,
    recv_threads: Vec<JoinHandle<()>>,
}

impl LayeredUserTransport {
    /// Build a layered transport from preference-ordered sub-transports. Each
    /// leg's kind is taken from its `local_locator().kind`. Spawns one recv
    /// thread per leg.
    #[must_use]
    pub fn new(transports: Vec<Arc<dyn Transport + Send + Sync>>) -> Self {
        let inbound = Arc::new((Mutex::new(VecDeque::with_capacity(128)), Condvar::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let mut legs = Vec::with_capacity(transports.len());
        let mut recv_threads = Vec::with_capacity(transports.len());

        for transport in transports {
            let kind = transport.local_locator().kind;
            legs.push(Leg {
                kind,
                transport: Arc::clone(&transport),
            });

            let inbound_cl = Arc::clone(&inbound);
            let stop_cl = Arc::clone(&stop);
            let t_cl = Arc::clone(&transport);
            let spawn = thread::Builder::new()
                .name(format!("zdds-layered-recv-{}", legs.len()))
                .spawn(move || {
                    while !stop_cl.load(Ordering::Relaxed) {
                        match t_cl.recv() {
                            Ok(dg) => {
                                let (lock, cv) = &*inbound_cl;
                                if let Ok(mut q) = lock.lock() {
                                    q.push_back(dg);
                                    cv.notify_one();
                                }
                            }
                            // Timeout is the normal idle path: loop and re-check
                            // the stop flag so the thread shuts down promptly.
                            Err(RecvError::Timeout) => {}
                            Err(_) => thread::sleep(Duration::from_millis(10)),
                        }
                    }
                });
            // A failed thread spawn (OS resource exhaustion) degrades this leg
            // to send-only rather than aborting the whole transport.
            if let Ok(join) = spawn {
                recv_threads.push(join);
            }
        }

        Self {
            legs,
            inbound,
            stop,
            recv_threads,
        }
    }

    /// Number of sub-transports (legs).
    #[must_use]
    pub fn leg_count(&self) -> usize {
        self.legs.len()
    }

    /// `true` if a leg serves the given locator kind.
    #[must_use]
    pub fn serves_kind(&self, kind: LocatorKind) -> bool {
        self.legs.iter().any(|l| l.kind == kind)
    }

    /// Stop all recv threads (idempotent).
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Transport for LayeredUserTransport {
    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        // Preference order = leg order: first matching kind wins.
        for leg in &self.legs {
            if leg.kind == dest.kind {
                return leg.transport.send(dest, data);
            }
        }
        Err(SendError::UnsupportedLocator)
    }

    fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
        let (lock, cv) = &*self.inbound;
        let mut q = lock.lock().map_err(|_| RecvError::Io {
            message: "layered inbound poisoned",
        })?;
        loop {
            if let Some(dg) = q.pop_front() {
                return Ok(dg);
            }
            let (g, _timeout) =
                cv.wait_timeout(q, Duration::from_secs(1))
                    .map_err(|_| RecvError::Io {
                        message: "layered inbound cv poisoned",
                    })?;
            q = g;
            if self.stop.load(Ordering::Relaxed) {
                return Err(RecvError::Timeout);
            }
        }
    }

    fn local_locator(&self) -> Locator {
        // Primary = the most-preferred (first) leg.
        self.legs
            .first()
            .map_or_else(invalid_locator, |l| l.transport.local_locator())
    }

    fn local_locators(&self) -> Vec<Locator> {
        self.legs
            .iter()
            .map(|l| l.transport.local_locator())
            .collect()
    }
}

impl Drop for LayeredUserTransport {
    fn drop(&mut self) {
        self.shutdown();
        for join in self.recv_threads.drain(..) {
            let _ = join.join();
        }
    }
}

fn invalid_locator() -> Locator {
    Locator {
        kind: LocatorKind::Invalid,
        port: 0,
        address: [0u8; 16],
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::type_complexity
)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    /// A mock transport serving one locator kind. `send` records the datagram;
    /// `recv` drains a pre-seeded inbox then times out (so the recv thread can
    /// stop). The recorded sends are visible via `sent`.
    struct MockTransport {
        local: Locator,
        sent: Arc<StdMutex<Vec<(Locator, Vec<u8>)>>>,
        inbox: Arc<StdMutex<VecDeque<ReceivedDatagram>>>,
    }
    impl MockTransport {
        fn new(
            kind: LocatorKind,
            addr_tag: u8,
        ) -> (Arc<Self>, Arc<StdMutex<Vec<(Locator, Vec<u8>)>>>) {
            let sent = Arc::new(StdMutex::new(Vec::new()));
            let t = Arc::new(Self {
                local: Locator {
                    kind,
                    port: 7400,
                    address: [addr_tag; 16],
                },
                sent: Arc::clone(&sent),
                inbox: Arc::new(StdMutex::new(VecDeque::new())),
            });
            (t, sent)
        }
        fn seed_inbound(&self, dg: ReceivedDatagram) {
            self.inbox.lock().unwrap().push_back(dg);
        }
    }
    impl Transport for MockTransport {
        fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
            self.sent.lock().unwrap().push((*dest, data.to_vec()));
            Ok(())
        }
        fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
            if let Some(dg) = self.inbox.lock().unwrap().pop_front() {
                Ok(dg)
            } else {
                std::thread::sleep(Duration::from_millis(5));
                Err(RecvError::Timeout)
            }
        }
        fn local_locator(&self) -> Locator {
            self.local
        }
    }

    fn loc(kind: LocatorKind, tag: u8) -> Locator {
        Locator {
            kind,
            port: 7400,
            address: [tag; 16],
        }
    }

    #[test]
    fn advertises_every_leg_locator() {
        let (shm, _) = MockTransport::new(LocatorKind::Shm, 0x11);
        let (udp, _) = MockTransport::new(LocatorKind::Reserved, 0x22);
        let layered = LayeredUserTransport::new(alloc::vec![shm, udp]);
        assert_eq!(layered.leg_count(), 2);
        let locs = layered.local_locators();
        assert_eq!(locs.len(), 2);
        assert!(locs.iter().any(|l| l.kind == LocatorKind::Shm));
        assert!(locs.iter().any(|l| l.kind == LocatorKind::Reserved));
        // Primary locator = first (most-preferred) leg.
        assert_eq!(layered.local_locator().kind, LocatorKind::Shm);
    }

    #[test]
    fn send_routes_by_destination_kind() {
        let (shm, shm_sent) = MockTransport::new(LocatorKind::Shm, 0x11);
        let (udp, udp_sent) = MockTransport::new(LocatorKind::Reserved, 0x22);
        let layered = LayeredUserTransport::new(alloc::vec![shm, udp]);

        layered
            .send(&loc(LocatorKind::Shm, 0xAA), b"via-shm")
            .unwrap();
        layered
            .send(&loc(LocatorKind::Reserved, 0xBB), b"via-udp")
            .unwrap();

        assert_eq!(shm_sent.lock().unwrap().len(), 1);
        assert_eq!(shm_sent.lock().unwrap()[0].1, b"via-shm");
        assert_eq!(udp_sent.lock().unwrap().len(), 1);
        assert_eq!(udp_sent.lock().unwrap()[0].1, b"via-udp");
    }

    #[test]
    fn send_to_unserved_kind_errors() {
        let (shm, _) = MockTransport::new(LocatorKind::Shm, 0x11);
        let layered = LayeredUserTransport::new(alloc::vec![shm]);
        let r = layered.send(&loc(LocatorKind::Reserved, 0xBB), b"x");
        assert!(matches!(r, Err(SendError::UnsupportedLocator)));
    }

    #[test]
    fn recv_multiplexes_from_all_legs() {
        let (shm, _) = MockTransport::new(LocatorKind::Shm, 0x11);
        let (udp, _) = MockTransport::new(LocatorKind::Reserved, 0x22);
        shm.seed_inbound(ReceivedDatagram {
            data: Arc::from(&b"from-shm"[..]),
            source: loc(LocatorKind::Shm, 0xAA),
        });
        udp.seed_inbound(ReceivedDatagram {
            data: Arc::from(&b"from-udp"[..]),
            source: loc(LocatorKind::Reserved, 0xBB),
        });
        let layered = LayeredUserTransport::new(alloc::vec![shm, udp]);

        let mut got = Vec::new();
        for _ in 0..2 {
            if let Ok(dg) = layered.recv() {
                got.push(dg.data.to_vec());
            }
        }
        got.sort();
        assert_eq!(got, alloc::vec![b"from-shm".to_vec(), b"from-udp".to_vec()]);
    }
}
