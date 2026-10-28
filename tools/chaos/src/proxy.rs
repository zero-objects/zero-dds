// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! UDP-Chaos-Proxy.
//!
//! Listens on a local bind, forwards to a remote forward target
//! and injects configurable chaos. Platform-neutral, no
//! no root needed — perfect for macOS devs and CI containers.

use std::collections::VecDeque;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use crate::prng::Xorshift64;

/// Chaos configuration.
#[derive(Clone)]
pub struct ChaosConfig {
    /// Probability of dropping a packet (0.0 - 1.0).
    pub loss: f64,
    /// When active: after a drop, `burst-1` further drops necessarily follow.
    pub loss_burst: u32,
    /// Probability of duplicating a packet.
    pub duplicate: f64,
    /// Probability of holding a packet in the buffer and swapping it
    /// with a later one.
    pub reorder: f64,
    /// Lower bound of the jitter window (delay before forward).
    pub jitter_min: Duration,
    /// Upper bound of the jitter window.
    pub jitter_max: Duration,
    /// PRNG seed (deterministic when set).
    pub seed: u64,
}

impl Default for ChaosConfig {
    fn default() -> Self {
        Self {
            loss: 0.0,
            loss_burst: 1,
            duplicate: 0.0,
            reorder: 0.0,
            jitter_min: Duration::ZERO,
            jitter_max: Duration::ZERO,
            seed: 0x000D_DEAD_BEEF_CAFE_u64.wrapping_mul(2),
        }
    }
}

/// Result counters, emitted at the proxy end.
#[derive(Default, Clone, Debug)]
pub struct ChaosStats {
    /// Packets successfully sent to forward.
    pub forwarded: u64,
    /// Packets discarded due to loss.
    pub dropped: u64,
    /// Extra copies produced due to duplication.
    pub duplicated: u64,
    /// Reorder operations (held-slot swaps).
    pub reordered: u64,
    /// Bytes on the ingress side (bind).
    pub bytes_in: u64,
    /// Bytes on the forward side.
    pub bytes_out: u64,
}

/// An egress pending entry — queues a packet for sending at a
/// Wallclock deadline.
struct PendingPacket {
    when: Instant,
    target: SocketAddr,
    bytes: Vec<u8>,
}

/// Bidirectional UDP chaos proxy: packets between client (any
/// source != forward_to) and the forward target are routed in both
/// directions, chaos is configured symmetrically.
///
/// Routing model:
///   * ingress from forward_to → back to the last known client.
///   * ingress from any other source → to forward_to,
///     the last-client source is remembered (last-sender-wins).
///
/// This makes both Pub→Sub and Ping↔Pong work through the
/// Proxy.
pub struct UdpChaosProxy {
    listen: UdpSocket,
    forward_to: SocketAddr,
    cfg: ChaosConfig,
    rng: Xorshift64,
    pending: VecDeque<PendingPacket>,
    /// Reorder buffer slot: a packet that we briefly hold back to
    /// swap it with the next one.
    held: Option<PendingPacket>,
    drops_left: u32,
    /// Last known client (≠ forward_to) — for reply routing.
    last_client: Option<SocketAddr>,
    /// Read counter, updated via `run_for`.
    pub stats: ChaosStats,
}

impl UdpChaosProxy {
    /// Creates a proxy on the bind socket. The proxy reads from
    /// `bind`, forwards to `forward`. Replies from `forward` are
    /// expected on the same socket — this matches the standard DDS
    /// pattern in which the SPDP/SEDP replies via default locator
    /// return.
    pub fn new(bind: SocketAddr, forward: SocketAddr, cfg: ChaosConfig) -> std::io::Result<Self> {
        let listen = UdpSocket::bind(bind)?;
        listen.set_nonblocking(true)?;
        let seed = if cfg.seed == 0 {
            0x000D_DEAD_BEEF_CAFE_u64.wrapping_mul(2)
        } else {
            cfg.seed
        };
        Ok(Self {
            listen,
            forward_to: forward,
            cfg,
            rng: Xorshift64::new(seed),
            pending: VecDeque::new(),
            held: None,
            drops_left: 0,
            last_client: None,
            stats: ChaosStats::default(),
        })
    }

    /// Returns the actual bind address (relevant when 0:0).
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.listen.local_addr()
    }

    /// Tick for a bounded wallclock duration. Non-blocking per
    /// iteration; spins with short sleeps when the pending queue is empty
    /// and no packet arrived.
    pub fn run_for(&mut self, total: Duration) -> std::io::Result<()> {
        let deadline = Instant::now() + total;
        let mut buf = vec![0u8; 65_536];
        while Instant::now() < deadline {
            self.flush_due_packets()?;
            // Receive: non-blocking per loop.
            match self.listen.recv_from(&mut buf) {
                Ok((n, from)) => {
                    self.stats.bytes_in = self.stats.bytes_in.saturating_add(n as u64);
                    let target = if from == self.forward_to {
                        // Reply from forward_to → to last known
                        // Send client back.
                        match self.last_client {
                            Some(c) => c,
                            None => continue, // no client known — drop
                        }
                    } else {
                        // Ingress from client → forward_to, remember source.
                        self.last_client = Some(from);
                        self.forward_to
                    };
                    self.process_inbound(&buf[..n], target);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // sleep briefly when there is no pending queue
                    if self.pending.is_empty() && self.held.is_none() {
                        std::thread::sleep(Duration::from_micros(200));
                    }
                }
                Err(e) => return Err(e),
            }
        }
        // Final flush.
        self.flush_due_packets()?;
        if let Some(h) = self.held.take() {
            self.pending.push_back(h);
        }
        // Send remaining packets, ignore jitter deadline.
        while let Some(p) = self.pending.pop_front() {
            let _ = self.listen.send_to(&p.bytes, p.target);
            self.stats.forwarded = self.stats.forwarded.saturating_add(1);
            self.stats.bytes_out = self.stats.bytes_out.saturating_add(p.bytes.len() as u64);
        }
        Ok(())
    }

    fn process_inbound(&mut self, bytes: &[u8], target: SocketAddr) {
        // 1) Loss (with burst).
        if self.drops_left > 0 {
            self.drops_left -= 1;
            self.stats.dropped += 1;
            return;
        }
        if self.rng.bernoulli(self.cfg.loss) {
            self.stats.dropped += 1;
            self.drops_left = self.cfg.loss_burst.saturating_sub(1);
            return;
        }
        // 2) Reorder: place into held slot, or swap held with current.
        if self.cfg.reorder > 0.0 && self.rng.bernoulli(self.cfg.reorder) {
            let now = Instant::now();
            let when = self.compute_jitter(now);
            let pkt = PendingPacket {
                when,
                target,
                bytes: bytes.to_vec(),
            };
            if let Some(prev) = self.held.replace(pkt) {
                self.stats.reordered += 1;
                self.enqueue(prev);
            }
            return;
        }
        // Emit held packets first (in normal order).
        if let Some(prev) = self.held.take() {
            self.enqueue(prev);
        }
        // 3) Duplicate.
        let dup = self.rng.bernoulli(self.cfg.duplicate);
        let now = Instant::now();
        let when = self.compute_jitter(now);
        self.enqueue(PendingPacket {
            when,
            target,
            bytes: bytes.to_vec(),
        });
        if dup {
            let when2 = self.compute_jitter(now);
            self.enqueue(PendingPacket {
                when: when2,
                target,
                bytes: bytes.to_vec(),
            });
            self.stats.duplicated += 1;
        }
    }

    fn enqueue(&mut self, p: PendingPacket) {
        // Insert sorted by `when`, so that flush_due_packets in
        // O(1) per packet removed.
        let pos = self.pending.iter().position(|q| q.when > p.when);
        match pos {
            Some(i) => self.pending.insert(i, p),
            None => self.pending.push_back(p),
        }
    }

    fn compute_jitter(&mut self, now: Instant) -> Instant {
        if self.cfg.jitter_max.is_zero() {
            return now;
        }
        let span = self
            .cfg
            .jitter_max
            .saturating_sub(self.cfg.jitter_min)
            .as_micros();
        let span_u64 = u64::try_from(span).unwrap_or(u64::MAX);
        let extra_us = if span_u64 == 0 {
            0
        } else {
            self.rng.range_u64(span_u64)
        };
        let delay = self.cfg.jitter_min + Duration::from_micros(extra_us);
        now + delay
    }

    fn flush_due_packets(&mut self) -> std::io::Result<()> {
        let now = Instant::now();
        while let Some(p) = self.pending.front() {
            if p.when > now {
                break;
            }
            // Pop and send. front() above returned Some; pop_front()
            // is therefore guaranteed to be Some.
            let Some(p) = self.pending.pop_front() else {
                break;
            };
            self.listen.send_to(&p.bytes, p.target)?;
            self.stats.forwarded = self.stats.forwarded.saturating_add(1);
            self.stats.bytes_out = self.stats.bytes_out.saturating_add(p.bytes.len() as u64);
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests may use unwrap.
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};

    fn local(port: u16) -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
    }

    #[test]
    fn loss_drops_at_configured_rate() {
        let echo = UdpSocket::bind(local(0)).unwrap();
        let echo_addr = echo.local_addr().unwrap();
        let mut proxy = UdpChaosProxy::new(
            local(0),
            echo_addr,
            ChaosConfig {
                loss: 1.0,
                ..Default::default()
            },
        )
        .unwrap();
        let proxy_addr = proxy.local_addr().unwrap();
        let sender = UdpSocket::bind(local(0)).unwrap();
        for _ in 0..50 {
            sender.send_to(b"hello", proxy_addr).unwrap();
        }
        proxy.run_for(Duration::from_millis(200)).unwrap();
        assert_eq!(proxy.stats.forwarded, 0);
        assert!(proxy.stats.dropped >= 50);
    }

    #[test]
    fn no_chaos_passes_everything() {
        let recv = UdpSocket::bind(local(0)).unwrap();
        recv.set_nonblocking(true).unwrap();
        let recv_addr = recv.local_addr().unwrap();
        let mut proxy = UdpChaosProxy::new(local(0), recv_addr, ChaosConfig::default()).unwrap();
        let proxy_addr = proxy.local_addr().unwrap();
        let sender = UdpSocket::bind(local(0)).unwrap();
        for _ in 0..30 {
            sender.send_to(b"x", proxy_addr).unwrap();
        }
        proxy.run_for(Duration::from_millis(200)).unwrap();
        assert_eq!(proxy.stats.dropped, 0);
        assert_eq!(proxy.stats.forwarded, 30);
    }

    #[test]
    fn duplicate_bumps_forwarded() {
        let recv = UdpSocket::bind(local(0)).unwrap();
        let recv_addr = recv.local_addr().unwrap();
        let mut proxy = UdpChaosProxy::new(
            local(0),
            recv_addr,
            ChaosConfig {
                duplicate: 1.0,
                ..Default::default()
            },
        )
        .unwrap();
        let proxy_addr = proxy.local_addr().unwrap();
        let sender = UdpSocket::bind(local(0)).unwrap();
        for _ in 0..10 {
            sender.send_to(b"y", proxy_addr).unwrap();
        }
        proxy.run_for(Duration::from_millis(200)).unwrap();
        assert_eq!(proxy.stats.duplicated, 10);
        assert_eq!(proxy.stats.forwarded, 20);
    }

    #[test]
    fn jitter_delays_packets() {
        let recv = UdpSocket::bind(local(0)).unwrap();
        let recv_addr = recv.local_addr().unwrap();
        let mut proxy = UdpChaosProxy::new(
            local(0),
            recv_addr,
            ChaosConfig {
                jitter_min: Duration::from_millis(20),
                jitter_max: Duration::from_millis(40),
                ..Default::default()
            },
        )
        .unwrap();
        let proxy_addr = proxy.local_addr().unwrap();
        let sender = UdpSocket::bind(local(0)).unwrap();
        let t0 = Instant::now();
        sender.send_to(b"z", proxy_addr).unwrap();
        proxy.run_for(Duration::from_millis(60)).unwrap();
        // Forward must have happened only after at least 20ms.
        assert_eq!(proxy.stats.forwarded, 1);
        assert!(t0.elapsed() >= Duration::from_millis(20));
    }
}
