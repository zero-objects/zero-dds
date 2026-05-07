// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! UDP-Chaos-Proxy.
//!
//! Hoert auf einem lokalen Bind, leitet auf einen Remote-Forward-Target
//! weiter und injiziert konfigurierbares Chaos. Plattformneutral, kein
//! root noetig — perfekt fuer macOS-Devs und CI-Container.

use std::collections::VecDeque;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use crate::prng::Xorshift64;

/// Chaos-Konfiguration.
#[derive(Clone)]
pub struct ChaosConfig {
    /// Wahrscheinlichkeit ein Paket zu droppen (0.0 - 1.0).
    pub loss: f64,
    /// Wenn aktiv: nach einem Drop folgen `burst-1` weitere Drops zwingend.
    pub loss_burst: u32,
    /// Wahrscheinlichkeit ein Paket zu duplizieren.
    pub duplicate: f64,
    /// Wahrscheinlichkeit ein Paket im Buffer zurueckzuhalten und mit
    /// einem spaeteren zu vertauschen.
    pub reorder: f64,
    /// Untergrenze des Jitter-Fensters (delay vor Forward).
    pub jitter_min: Duration,
    /// Obergrenze des Jitter-Fensters.
    pub jitter_max: Duration,
    /// PRNG-Seed (deterministisch wenn gesetzt).
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

/// Ergebnis-Counter, am Proxy-Ende ausgegeben.
#[derive(Default, Clone, Debug)]
pub struct ChaosStats {
    /// Erfolgreich an Forward gesendete Pakete.
    pub forwarded: u64,
    /// Wegen Loss verworfene Pakete.
    pub dropped: u64,
    /// Wegen Duplicate zusaetzlich erzeugte Kopien.
    pub duplicated: u64,
    /// Reorder-Vorgaenge (Held-Slot-Tausch).
    pub reordered: u64,
    /// Bytes auf der Eingangsseite (Bind).
    pub bytes_in: u64,
    /// Bytes auf der Forward-Seite.
    pub bytes_out: u64,
}

/// Eine Egress-Wartung — pendet ein Paket zur Versendung an einer
/// Wallclock-Frist.
struct PendingPacket {
    when: Instant,
    target: SocketAddr,
    bytes: Vec<u8>,
}

/// Bidirektionaler UDP-Chaos-Proxy: Pakete zwischen Client (jede
/// Source != forward_to) und dem Forward-Target werden in beide
/// Richtungen weitergeleitet, Chaos ist symmetrisch konfiguriert.
///
/// Routing-Modell:
///   * Eingang vom forward_to → an letzten bekannten Client zurueck.
///   * Eingang von beliebigem anderem Source → an forward_to,
///     letzter-Client-Source wird gemerkt (last-sender-wins).
///
/// Damit funktioniert sowohl Pub→Sub als auch Ping↔Pong durch den
/// Proxy.
pub struct UdpChaosProxy {
    listen: UdpSocket,
    forward_to: SocketAddr,
    cfg: ChaosConfig,
    rng: Xorshift64,
    pending: VecDeque<PendingPacket>,
    /// Reorder-Buffer-Slot: ein Paket, das wir kurz zuruecklegen, um
    /// es mit dem naechsten zu tauschen.
    held: Option<PendingPacket>,
    drops_left: u32,
    /// Letzter bekannter Client (≠ forward_to) — fuer Reply-Routing.
    last_client: Option<SocketAddr>,
    /// Lese-Counter, ueber `run_for` aktualisiert.
    pub stats: ChaosStats,
}

impl UdpChaosProxy {
    /// Erzeugt einen Proxy auf dem Bind-Socket. Der Proxy liest von
    /// `bind`, forwarded zu `forward`. Antworten von `forward` werden
    /// auf demselben Socket erwartet — das matchet das Standard-DDS-
    /// Pattern, in dem die SPDP-/SEDP-Antworten via Default-Locator
    /// zurueckkommen.
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

    /// Returnt die tatsaechliche Bind-Adresse (relevant wenn 0:0).
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.listen.local_addr()
    }

    /// Tick fuer eine begrenzte Wallclock-Dauer. Nicht-blockierend pro
    /// Iteration; spinned mit short sleeps wenn die Pending-Queue leer
    /// ist und kein Paket eintraf.
    pub fn run_for(&mut self, total: Duration) -> std::io::Result<()> {
        let deadline = Instant::now() + total;
        let mut buf = vec![0u8; 65_536];
        while Instant::now() < deadline {
            self.flush_due_packets()?;
            // Receive: nicht-blockierend pro Loop.
            match self.listen.recv_from(&mut buf) {
                Ok((n, from)) => {
                    self.stats.bytes_in = self.stats.bytes_in.saturating_add(n as u64);
                    let target = if from == self.forward_to {
                        // Reply von forward_to → an letzten bekannten
                        // Client zurueckschleusen.
                        match self.last_client {
                            Some(c) => c,
                            None => continue, // kein Client bekannt — droppen
                        }
                    } else {
                        // Eingang von Client → forward_to, Source merken.
                        self.last_client = Some(from);
                        self.forward_to
                    };
                    self.process_inbound(&buf[..n], target);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // sleep kurz wenn keine Pending-Queue
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
        // Restpakete senden, ignoriere Jitter-Frist.
        while let Some(p) = self.pending.pop_front() {
            let _ = self.listen.send_to(&p.bytes, p.target);
            self.stats.forwarded = self.stats.forwarded.saturating_add(1);
            self.stats.bytes_out = self.stats.bytes_out.saturating_add(p.bytes.len() as u64);
        }
        Ok(())
    }

    fn process_inbound(&mut self, bytes: &[u8], target: SocketAddr) {
        // 1) Loss (mit Burst).
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
        // 2) Reorder: in Held-Slot legen, oder Held mit aktuellem tauschen.
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
        // Held vorher rausgeben (in normaler Ordnung).
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
        // Insert sortiert nach `when`, sodass flush_due_packets in
        // O(1) pro entnommenem Paket arbeitet.
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
            // Pop and send. front() oben hat Some geliefert; pop_front()
            // ist dadurch garantiert Some.
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
#[allow(clippy::unwrap_used)] // tests duerfen unwrap nutzen.
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
        // Forward muss erst nach mind. 20ms passiert sein.
        assert_eq!(proxy.stats.forwarded, 1);
        assert!(t0.elapsed() >= Duration::from_millis(20));
    }
}
