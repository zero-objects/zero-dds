// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `UdpTransport`: konkrete `Transport`-Implementation ueber
//! `std::net::UdpSocket`.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use socket2::{Domain, Protocol, Socket, Type};
use zerodds_monitor::{Counter, Labels, default_registry, metric_names};
use zerodds_rtps::wire_types::{Locator, LocatorKind};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};

fn udp_counters() -> &'static UdpCounters {
    static C: OnceLock<UdpCounters> = OnceLock::new();
    C.get_or_init(|| {
        let r = default_registry();
        r.set_help(
            metric_names::DDS_TRANSPORT_PACKETS_SENT_TOTAL,
            "RTPS-Pakete gesendet (zerodds-monitor-1.0 §2.1)",
        );
        r.set_help(
            metric_names::DDS_TRANSPORT_PACKETS_RECEIVED_TOTAL,
            "RTPS-Pakete empfangen (zerodds-monitor-1.0 §2.1)",
        );
        r.set_help(
            metric_names::DDS_TRANSPORT_BYTES_SENT_TOTAL,
            "Bytes gesendet (zerodds-monitor-1.0 §2.1)",
        );
        r.set_help(
            metric_names::DDS_TRANSPORT_BYTES_RECEIVED_TOTAL,
            "Bytes empfangen (zerodds-monitor-1.0 §2.1)",
        );
        r.set_help(
            metric_names::DDS_TRANSPORT_SEND_ERRORS_TOTAL,
            "Send-Fehler (zerodds-monitor-1.0 §2.1)",
        );
        let labels = || Labels::new().with("transport", "udp");
        UdpCounters {
            packets_sent: r.counter(metric_names::DDS_TRANSPORT_PACKETS_SENT_TOTAL, labels()),
            packets_received: r
                .counter(metric_names::DDS_TRANSPORT_PACKETS_RECEIVED_TOTAL, labels()),
            bytes_sent: r.counter(metric_names::DDS_TRANSPORT_BYTES_SENT_TOTAL, labels()),
            bytes_received: r.counter(metric_names::DDS_TRANSPORT_BYTES_RECEIVED_TOTAL, labels()),
            send_errors: r.counter(
                metric_names::DDS_TRANSPORT_SEND_ERRORS_TOTAL,
                Labels::new()
                    .with("transport", "udp")
                    .with("error_kind", "io"),
            ),
        }
    })
}

struct UdpCounters {
    packets_sent: Arc<Counter>,
    packets_received: Arc<Counter>,
    bytes_sent: Arc<Counter>,
    bytes_received: Arc<Counter>,
    send_errors: Arc<Counter>,
}

/// Maximale Datagram-Groesse fuer einen UDP-recv. Begrenzt auf den
/// klassischen IP-Datagram-Limit ohne Fragmentation (sicher fuer
/// Phase 0).
pub const MAX_DATAGRAM_SIZE: usize = 65_507;

/// Konstruktions-Fehler.
#[derive(Debug)]
pub enum UdpTransportError {
    /// Bind fehlgeschlagen.
    Bind(std::io::Error),
    /// `set_read_timeout` fehlgeschlagen.
    SetTimeout(std::io::Error),
}

impl core::fmt::Display for UdpTransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Bind(e) => write!(f, "udp bind failed: {e}"),
            Self::SetTimeout(e) => write!(f, "udp set_read_timeout failed: {e}"),
        }
    }
}

impl std::error::Error for UdpTransportError {}

/// UDP-basierter Transport.
///
/// Konstruktion bindet einen Socket an einen lokalen Port und merkt
/// sich den lokalen Locator fuer `local_locator()`-Calls. Empfangs-
/// Timeout ist Default-`None` (blockiert bis Datagram da); via
/// `with_timeout` einstellbar.
#[derive(Debug)]
pub struct UdpTransport {
    socket: UdpSocket,
    local_locator: Locator,
}

impl UdpTransport {
    /// Bindet an die gegebene IPv4-Adresse + Port.
    /// `port = 0` laesst das OS einen freien Port waehlen.
    ///
    /// # Errors
    /// `UdpTransportError::Bind`.
    pub fn bind_v4(addr: Ipv4Addr, port: u16) -> Result<Self, UdpTransportError> {
        let bind_addr = SocketAddrV4::new(addr, port);
        let socket = UdpSocket::bind(bind_addr).map_err(UdpTransportError::Bind)?;
        let local = match socket.local_addr().map_err(UdpTransportError::Bind)? {
            SocketAddr::V4(v4) => v4,
            SocketAddr::V6(_) => {
                // IPv6 sollte bei IPv4-bind nicht auftreten — defensiv.
                return Err(UdpTransportError::Bind(std::io::Error::other(
                    "got V6 address on V4 bind",
                )));
            }
        };
        let local_locator = Locator::udp_v4(local.ip().octets(), u32::from(local.port()));
        Ok(Self {
            socket,
            local_locator,
        })
    }

    /// Setzt den Empfangs-Timeout. `None` bedeutet blockierend bis
    /// Datagram da.
    ///
    /// # Errors
    /// `UdpTransportError::SetTimeout`.
    pub fn with_timeout(self, timeout: Option<Duration>) -> Result<Self, UdpTransportError> {
        self.socket
            .set_read_timeout(timeout)
            .map_err(UdpTransportError::SetTimeout)?;
        Ok(self)
    }

    /// Konfiguriert den Socket als Multicast-Receiver: bindet an
    /// `0.0.0.0:port` mit `SO_REUSEADDR`+`SO_REUSEPORT` und tritt der
    /// Multicast-Group bei. SO_REUSE_* erlaubt mehrere Prozesse auf
    /// demselben Multicast-Port (z.B. ZeroDDS + Cyclone parallel).
    ///
    /// `interface = 0.0.0.0` laesst den Kernel das Default-Interface
    /// waehlen (oft Loopback). Fuer echte Discovery zwischen Prozessen
    /// auf demselben Host muss man die konkrete IP des Netzwerk-
    /// Interfaces angeben (z.B. `192.168.1.10`).
    ///
    /// # Errors
    /// `UdpTransportError::Bind` bei Bind/Multicast-Join-Fehler.
    ///
    /// Bei `EADDRINUSE`/`EADDRNOTAVAIL` retryt die Methode bis zu
    /// dreimal mit Backoff (100/300/700 ms). Hintergrund: in CI-Tests
    /// die sequentiell DomainParticipants in derselben Domain
    /// erzeugen und droppen, ist die Multicast-Membership-Cleanup-
    /// Latenz im Kernel nicht-deterministisch — der nachfolgende
    /// Bind kann transient EADDRINUSE bekommen, auch wenn
    /// `SO_REUSEADDR` gesetzt ist (insbesondere bei
    /// `IP_MAX_MEMBERSHIPS`-naher Auslastung). Der Retry-Loop laesst
    /// das eindeutig fluechtige Race verschwinden, ohne dauerhafte
    /// Sleep-Pausen einzuziehen.
    pub fn bind_multicast_v4(
        group: Ipv4Addr,
        port: u16,
        interface: Ipv4Addr,
    ) -> Result<Self, UdpTransportError> {
        const RETRY_BACKOFF_MS: &[u64] = &[100, 300, 700];

        let mut last_err: Option<std::io::Error> = None;
        for (attempt, backoff_ms) in core::iter::once(0)
            .chain(RETRY_BACKOFF_MS.iter().copied())
            .enumerate()
        {
            if attempt > 0 {
                std::thread::sleep(core::time::Duration::from_millis(backoff_ms));
            }
            match Self::try_bind_multicast_v4(group, port, interface) {
                Ok(t) => return Ok(t),
                Err(UdpTransportError::Bind(e)) => {
                    let kind = e.kind();
                    let retryable = matches!(
                        kind,
                        std::io::ErrorKind::AddrInUse | std::io::ErrorKind::AddrNotAvailable,
                    );
                    if !retryable {
                        return Err(UdpTransportError::Bind(e));
                    }
                    last_err = Some(e);
                }
                // try_bind_multicast_v4 erzeugt nur Bind-Errors —
                // SetTimeout kommt erst spaeter via with_timeout(). Andere
                // Varianten direkt durchreichen.
                Err(other) => return Err(other),
            }
        }
        // Alle Retries erschoepft.
        Err(UdpTransportError::Bind(last_err.unwrap_or_else(|| {
            std::io::Error::other("multicast bind retries exhausted")
        })))
    }

    fn try_bind_multicast_v4(
        group: Ipv4Addr,
        port: u16,
        interface: Ipv4Addr,
    ) -> Result<Self, UdpTransportError> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
            .map_err(UdpTransportError::Bind)?;
        socket
            .set_reuse_address(true)
            .map_err(UdpTransportError::Bind)?;
        #[cfg(unix)]
        socket
            .set_reuse_port(true)
            .map_err(UdpTransportError::Bind)?;
        let bind_addr: SocketAddr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into();
        socket
            .bind(&bind_addr.into())
            .map_err(UdpTransportError::Bind)?;
        socket
            .join_multicast_v4(&group, &interface)
            .map_err(UdpTransportError::Bind)?;
        let socket: UdpSocket = socket.into();
        let local = match socket.local_addr().map_err(UdpTransportError::Bind)? {
            SocketAddr::V4(v4) => v4,
            SocketAddr::V6(_) => {
                return Err(UdpTransportError::Bind(std::io::Error::other(
                    "got V6 address on V4 multicast bind",
                )));
            }
        };
        // Local-Locator zeigt auf die Multicast-Group, nicht auf
        // 0.0.0.0 — Caller-Sicht ist "ich empfange auf der Group".
        let local_locator = Locator::udp_v4(group.octets(), u32::from(local.port()));
        Ok(Self {
            socket,
            local_locator,
        })
    }

    /// Setzt das Multicast-TTL fuer ausgehende Multicast-Pakete.
    /// Default ist meist 1 (nur lokales Subnet). 32 = lokale Site
    /// (RFC-2365); 255 = global.
    ///
    /// # Errors
    /// `UdpTransportError::SetTimeout` (re-used).
    pub fn set_multicast_ttl(self, ttl: u32) -> Result<Self, UdpTransportError> {
        self.socket
            .set_multicast_ttl_v4(ttl)
            .map_err(UdpTransportError::SetTimeout)?;
        Ok(self)
    }
}

impl Transport for UdpTransport {
    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        if dest.kind != LocatorKind::UdpV4 {
            return Err(SendError::UnsupportedLocator);
        }
        let ip = dest.ipv4();
        let port = u16::try_from(dest.port).map_err(|_| SendError::UnsupportedLocator)?;
        let addr = SocketAddrV4::new(Ipv4Addr::from(ip), port);
        if data.len() > MAX_DATAGRAM_SIZE {
            return Err(SendError::PayloadTooLarge {
                size: data.len(),
                limit: MAX_DATAGRAM_SIZE,
            });
        }
        let counters = udp_counters();
        self.socket.send_to(data, addr).map_err(|_| {
            counters.send_errors.inc();
            SendError::Io {
                message: "udp send_to failed",
            }
        })?;
        counters.packets_sent.inc();
        counters.bytes_sent.add(data.len() as u64);
        #[cfg(feature = "inspect")]
        dispatch_transport_tap("udp:send", data);
        Ok(())
    }

    fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
        let mut buf = [0u8; MAX_DATAGRAM_SIZE];
        let (len, peer) = self.socket.recv_from(&mut buf).map_err(|e| {
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut
            {
                RecvError::Timeout
            } else {
                RecvError::Io {
                    message: "udp recv_from failed",
                }
            }
        })?;
        let counters = udp_counters();
        counters.packets_received.inc();
        counters.bytes_received.add(len as u64);
        let source = match peer {
            SocketAddr::V4(v4) => Locator::udp_v4(v4.ip().octets(), u32::from(v4.port())),
            SocketAddr::V6(_) => {
                return Err(RecvError::Io {
                    message: "received V6 datagram on V4 socket",
                });
            }
        };
        let data = buf[..len].to_vec();
        #[cfg(feature = "inspect")]
        dispatch_transport_tap("udp:recv", &data);
        Ok(ReceivedDatagram { source, data })
    }

    fn local_locator(&self) -> Locator {
        self.local_locator
    }
}

#[cfg(feature = "inspect")]
fn dispatch_transport_tap(label: &str, data: &[u8]) {
    let ts_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    let mut frame = zerodds_inspect_endpoint::Frame::transport(ts_ns, 0, data.to_vec());
    frame.topic = label.to_owned();
    zerodds_inspect_endpoint::tap::dispatch(&frame);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::print_stderr)]
    use super::*;

    fn make_loopback_pair() -> (UdpTransport, UdpTransport) {
        let a = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind a");
        let b = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind b");
        let a = a
            .with_timeout(Some(Duration::from_secs(2)))
            .expect("timeout a");
        let b = b
            .with_timeout(Some(Duration::from_secs(2)))
            .expect("timeout b");
        (a, b)
    }

    #[test]
    fn bind_v4_returns_local_locator_with_assigned_port() {
        let t = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind");
        let loc = t.local_locator();
        assert_eq!(loc.kind, LocatorKind::UdpV4);
        assert_eq!(loc.ipv4(), [127, 0, 0, 1]);
        assert!(loc.port > 0, "OS should assign a non-zero port");
    }

    #[test]
    fn loopback_send_and_recv_delivers_datagram() {
        let (sender, receiver) = make_loopback_pair();
        let dest = receiver.local_locator();
        let payload = b"hello rtps";
        sender.send(&dest, payload).expect("send");
        let received = receiver.recv().expect("recv");
        assert_eq!(received.data, payload);
        // Quell-Locator muss auf den Sender zeigen.
        assert_eq!(received.source.kind, LocatorKind::UdpV4);
        assert_eq!(received.source.ipv4(), [127, 0, 0, 1]);
        assert_eq!(received.source.port, sender.local_locator().port);
    }

    #[test]
    fn send_rejects_non_udpv4_locator() {
        let t = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind");
        let res = t.send(&Locator::INVALID, b"x");
        assert!(matches!(res, Err(SendError::UnsupportedLocator)));
    }

    #[test]
    fn send_rejects_payload_above_max() {
        let t = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind");
        let huge = vec![0u8; MAX_DATAGRAM_SIZE + 1];
        let res = t.send(&t.local_locator(), &huge);
        assert!(matches!(res, Err(SendError::PayloadTooLarge { .. })));
    }

    #[test]
    fn recv_timeout_returns_timeout_error() {
        let t = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)
            .expect("bind")
            .with_timeout(Some(Duration::from_millis(50)))
            .expect("timeout");
        let res = t.recv();
        assert!(matches!(res, Err(RecvError::Timeout)), "got {res:?}");
    }

    #[test]
    fn bind_multicast_v4_joins_group_and_returns_locator() {
        // Wir nutzen 239.0.0.1 als Test-Group (nicht reservierte SPDP-
        // Group, damit parallele Test-Laeufe sich nicht stoeren).
        let group = Ipv4Addr::new(239, 0, 0, 1);
        // Port=0 nicht erlaubt fuer Multicast-Bind — nutze
        // ephemeral Port via Bind-Versuch mit Default-Port. Wenn das
        // OS reserviert ist, schalten wir die Test-Variante auf
        // "kein-Multicast" um.
        let res = UdpTransport::bind_multicast_v4(group, 0, Ipv4Addr::LOCALHOST);
        // Manche CI-Umgebungen erlauben kein Multicast — dann ist
        // ein Bind-Fehler akzeptabel und der Test wird geskippt.
        let Ok(t) = res else {
            eprintln!("Multicast not available in environment; skipping");
            return;
        };
        let loc = t.local_locator();
        assert_eq!(loc.kind, LocatorKind::UdpV4);
        // Locator zeigt auf die Group, nicht auf 0.0.0.0.
        assert_eq!(loc.ipv4(), [239, 0, 0, 1]);
        assert!(loc.port > 0);
    }

    #[test]
    fn set_multicast_ttl_does_not_error() {
        let t = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind");
        let res = t.set_multicast_ttl(32);
        assert!(res.is_ok());
    }

    #[test]
    fn loopback_multiple_datagrams_in_order() {
        let (sender, receiver) = make_loopback_pair();
        let dest = receiver.local_locator();
        for i in 0u8..5 {
            sender.send(&dest, &[i, i, i]).expect("send");
        }
        for i in 0u8..5 {
            let r = receiver.recv().expect("recv");
            assert_eq!(r.data, vec![i, i, i]);
        }
    }
}
