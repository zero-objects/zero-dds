// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Multi-peer SHM adapter for `user_unicast`.
//!
//! `transport-shm` (PosixShmTransport) is 1:1 and role-asymmetric: per
//! peer an owner segment (for send) and a consumer segment (for recv).
//! But DcpsRuntime's `user_unicast` needs **multi-peer + bidirectional**
//! behind the `Transport` trait.
//!
//! `ShmUserTransport` wraps that:
//! - Its own `local_id` (16-byte SHM segment ID, derived from GuidPrefix).
//! - Per discovered peer (lazy on first `send`): opens an owner segment
//!   for send + a consumer segment for recv, spawns a recv thread that
//!   pushes from the consumer segment into a central `inbound` queue.
//! - `Transport::send(dest, data)`: routes to the owner segment of the
//!   peer encoded in the `dest` locator.
//! - `Transport::recv()`: pops from the central queue (filled by the
//!   per-peer recv threads).
//!
//! ## Locator convention
//!
//! `Locator { kind: Shm, address: [16-byte peer-SHM-Id], port: 0 }`.
//! The peer SHM ID is announced in the SPDP beacon
//! (PID_DEFAULT_UNICAST_LOCATOR); it is the 16-byte locator address from
//! the SHM adapter's `local_locator()`.
//!
//! ## Limitation (sprint 1)
//!
//! - Intra-host only (PosixShm needs a shared FS / OS).
//! - **ZeroDDS↔ZeroDDS** only (vendor-specific segment format).
//! - SPDP discovery itself still runs over UDPv4 multicast.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use zerodds_rtps::wire_types::{Locator, LocatorKind};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};
use zerodds_transport_shm::posix::{PosixShmTransport, ShmConfig};

/// Multi-peer SHM adapter (see module docs).
pub struct ShmUserTransport {
    local_id: [u8; 16],
    local_locator: Locator,
    config: ShmConfig,
    state: Arc<Mutex<AdapterState>>,
    inbound_cv: Arc<Condvar>,
    stop: Arc<std::sync::atomic::AtomicBool>,
}

struct AdapterState {
    /// peer-id → owner segment (for `send` to this peer).
    /// Lazy-created on first send per peer.
    owners: HashMap<[u8; 16], Arc<PosixShmTransport>>,
    /// peer-id → consumer segment + its recv thread.
    /// Lazy-created on first send per peer (or via an explicit
    /// `register_peer` call for inbound-only).
    consumers: HashMap<[u8; 16], ConsumerEntry>,
    /// Central inbound queue. Per-peer recv threads push in,
    /// `Transport::recv` pops out.
    inbound: std::collections::VecDeque<ReceivedDatagram>,
}

struct ConsumerEntry {
    _join: thread::JoinHandle<()>,
}

impl ShmUserTransport {
    /// New adapter with the given `local_id` (16 bytes, unique per
    /// process — typically derived from GuidPrefix + padding).
    ///
    /// # Errors
    /// None — the initial adapter creation only allocates maps.
    /// Segments are created lazily on the first send / peer register.
    pub fn new(local_id: [u8; 16], config: ShmConfig) -> Self {
        let mut addr = [0u8; 16];
        addr.copy_from_slice(&local_id);
        let local_locator = Locator {
            kind: LocatorKind::Shm,
            port: 0,
            address: addr,
        };
        Self {
            local_id,
            local_locator,
            config,
            state: Arc::new(Mutex::new(AdapterState {
                owners: HashMap::new(),
                consumers: HashMap::new(),
                inbound: std::collections::VecDeque::with_capacity(128),
            })),
            inbound_cv: Arc::new(Condvar::new()),
            stop: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Stops all recv threads (idempotent).
    pub fn shutdown(&self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        // Recv threads see the stop flag in the next recv timeout cycle.
        // No join here — Drop waits implicitly via the JoinHandle.
    }

    /// Lazy-create the owner+consumer pair for a peer.
    /// Owner = "I write to the peer", consumer = "I read from the peer".
    fn ensure_pair(&self, peer_id: [u8; 16]) -> Result<Arc<PosixShmTransport>, SendError> {
        // Fast path: owner already present.
        if let Ok(st) = self.state.lock() {
            if let Some(o) = st.owners.get(&peer_id) {
                return Ok(Arc::clone(o));
            }
        }
        // Slow path: open_owner + open_consumer + start recv thread.
        let owner = PosixShmTransport::open_owner(self.local_id, peer_id, self.config.clone())
            .map_err(|_| SendError::Io {
                message: "shm open_owner failed",
            })?;
        let consumer =
            PosixShmTransport::open_consumer(self.local_id, peer_id, self.config.clone()).map_err(
                |_| SendError::Io {
                    message: "shm open_consumer failed",
                },
            )?;
        let owner_arc = Arc::new(owner);
        let consumer_arc = Arc::new(consumer);
        // Recv thread for this consumer segment.
        let state_cl = Arc::clone(&self.state);
        let cv_cl = Arc::clone(&self.inbound_cv);
        let stop_cl = Arc::clone(&self.stop);
        let consumer_for_thread = Arc::clone(&consumer_arc);
        let join = thread::Builder::new()
            .name(format!(
                "zdds-shm-recv-{:02x}{:02x}",
                peer_id[0], peer_id[1]
            ))
            .spawn(move || {
                while !stop_cl.load(std::sync::atomic::Ordering::Relaxed) {
                    match consumer_for_thread.recv() {
                        Ok(dg) => {
                            if let Ok(mut st) = state_cl.lock() {
                                st.inbound.push_back(dg);
                                cv_cl.notify_one();
                            }
                        }
                        Err(_) => {
                            // Timeout or error: sleep briefly, then retry.
                            std::thread::sleep(Duration::from_millis(10));
                        }
                    }
                }
            })
            .map_err(|_| SendError::Io {
                message: "shm recv thread spawn failed",
            })?;
        if let Ok(mut st) = self.state.lock() {
            st.owners.insert(peer_id, Arc::clone(&owner_arc));
            st.consumers.insert(peer_id, ConsumerEntry { _join: join });
        }
        Ok(owner_arc)
    }
}

impl Transport for ShmUserTransport {
    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        if dest.kind != LocatorKind::Shm {
            return Err(SendError::UnsupportedLocator);
        }
        let peer_id = dest.address;
        let owner = self.ensure_pair(peer_id)?;
        // Owner.send needs the peer locator, not the local one.
        owner.send(dest, data)
    }

    fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
        let mut guard = self.state.lock().map_err(|_| RecvError::Io {
            message: "shm adapter state poisoned",
        })?;
        loop {
            if let Some(dg) = guard.inbound.pop_front() {
                return Ok(dg);
            }
            // Block at most 1s so the stop flag is picked up early.
            let (g, _wto) = self
                .inbound_cv
                .wait_timeout(guard, Duration::from_secs(1))
                .map_err(|_| RecvError::Io {
                    message: "shm adapter cv poisoned",
                })?;
            guard = g;
            if self.stop.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(RecvError::Timeout);
            }
        }
    }

    fn local_locator(&self) -> Locator {
        self.local_locator
    }
}

impl Drop for ShmUserTransport {
    fn drop(&mut self) {
        self.shutdown();
    }
}
