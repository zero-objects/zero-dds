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
    consumer: Arc<PosixShmTransport>,
    join: thread::JoinHandle<()>,
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
            st.consumers.insert(
                peer_id,
                ConsumerEntry {
                    consumer: consumer_arc,
                    join,
                },
            );
        }
        Ok(owner_arc)
    }
}

impl Transport for ShmUserTransport {
    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        if dest.kind != LocatorKind::Shm {
            return Err(SendError::UnsupportedLocator);
        }
        // After `close` no pair and no receive thread is made anew.
        if self.stop.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(SendError::Io {
                message: "shm adapter closed",
            });
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
            if self.stop.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(RecvError::Timeout);
            }
            let (g, wait) = self
                .inbound_cv
                .wait_timeout(guard, Duration::from_secs(1))
                .map_err(|_| RecvError::Io {
                    message: "shm adapter cv poisoned",
                })?;
            guard = g;
            // A second without a datagram hands control back, as a socket's
            // read timeout does: the runtime's receive loop checks its own
            // stop flag between calls. Waiting here for this adapter's stop
            // kept `DcpsRuntime::shutdown` in the join of that loop.
            if wait.timed_out() && guard.inbound.is_empty() {
                return Err(RecvError::Timeout);
            }
        }
    }

    /// Stops the per-peer receive threads, interrupting the wait each is
    /// in, and joins them; a `recv` that waits returns.
    fn close(&self) {
        self.shutdown();
        let consumers: Vec<ConsumerEntry> = match self.state.lock() {
            Ok(mut st) => st.consumers.drain().map(|(_, entry)| entry).collect(),
            Err(_) => Vec::new(),
        };
        for entry in &consumers {
            entry.consumer.interrupt_recv();
        }
        self.inbound_cv.notify_all();
        for entry in consumers {
            let _ = entry.join.join();
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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// An id distinct in every test process.
    fn id(n: u8) -> [u8; 16] {
        let mut id = [0u8; 16];
        id[..4].copy_from_slice(&std::process::id().to_be_bytes());
        id[4] = 0x5A;
        id[15] = n;
        id
    }

    /// The runtime's configuration: no receive timeout.
    fn config(dir: &std::path::Path) -> ShmConfig {
        ShmConfig {
            flink_dir: dir.to_path_buf(),
            recv_timeout: None,
            ..ShmConfig::default()
        }
    }

    /// `recv` with nothing to receive returns `Timeout` after a second.
    /// It waited until the adapter's own stop, which the runtime never
    /// set, and its shutdown stayed in the join of the receive loop.
    #[test]
    fn recv_without_a_datagram_returns_timeout_after_a_second() {
        let tmp = tempfile::tempdir().unwrap();
        let t = ShmUserTransport::new(id(1), config(tmp.path()));
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let start = Instant::now();
            let result = t.recv();
            let _ = done_tx.send((result, start.elapsed()));
        });
        let (result, took) = done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("recv returns");
        assert!(matches!(result, Err(RecvError::Timeout)), "{result:?}");
        assert!(took >= Duration::from_millis(900), "{took:?}");
    }

    /// `close` ends a peer's receive thread, which waits without a
    /// timeout, and a waiting `recv`; a later `send` fails.
    #[test]
    fn close_ends_the_peer_threads_and_a_waiting_recv() {
        let tmp = tempfile::tempdir().unwrap();
        let a = Arc::new(ShmUserTransport::new(id(2), config(tmp.path())));
        let b = ShmUserTransport::new(id(3), config(tmp.path()));
        a.send(&b.local_locator(), b"x").unwrap();
        assert_eq!(a.state.lock().unwrap().consumers.len(), 1);

        let (recv_tx, recv_rx) = std::sync::mpsc::channel();
        let waiting = Arc::clone(&a);
        thread::spawn(move || {
            let _ = recv_tx.send(waiting.recv());
        });
        thread::sleep(Duration::from_millis(50));

        let (closed_tx, closed_rx) = std::sync::mpsc::channel();
        let closing = Arc::clone(&a);
        thread::spawn(move || {
            let start = Instant::now();
            closing.close();
            let _ = closed_tx.send(start.elapsed());
        });
        let took = closed_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("close returns");
        assert!(took < Duration::from_millis(500), "{took:?}");
        let result = recv_rx
            .recv_timeout(Duration::from_millis(500))
            .expect("the waiting recv returns");
        assert!(matches!(result, Err(RecvError::Timeout)), "{result:?}");
        assert!(a.state.lock().unwrap().consumers.is_empty());
        assert!(matches!(
            a.send(&b.local_locator(), b"y"),
            Err(SendError::Io { .. })
        ));
    }
}
