// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Transport carriers for serialised UADP messages (OPC Foundation
//! Part 14 §7.3).
//!
//! UADP is a connectionless wire format: a Publisher *sends* a serialised
//! NetworkMessage as one datagram and a Subscriber *receives* it. This module
//! captures that with the [`PubSubTransport`] trait and ships the carriers a
//! pure-Rust stack can offer safely:
//!
//! - [`UdpTransport`] (feature `std`) — UADP over UDP unicast/multicast
//!   (Part 14 §7.3.5), the canonical datagram carrier.
//! - [`LoopbackTransport`] (feature `std`) — an in-process queue for tests
//!   and the [`crate::daemon`].
//! - [`MqttTransport`] — UADP over MQTT (Part 14 §7.3.4), broker-agnostic via
//!   the [`MqttClient`] trait so any client (e.g. `zerodds-mqtt-bridge`) can
//!   back it.
//! - [`AmqpTransport`] — UADP over AMQP (Part 14 Annex B.3) via the
//!   [`AmqpClient`] trait, whose `send_to`/`recv_from` shape matches the
//!   existing `zerodds-amqp-endpoint` client (AMQP 0.9.1 / 1.0).
//! - [`KafkaTransport`] — UADP over Kafka (Part 14 Annex B.2) via the
//!   [`KafkaClient`] trait.
//! - [`EthernetTransport`] — UADP over Ethernet (Part 14 §7.3.3, EtherType
//!   [`ETHERNET_ETHERTYPE`]) via the [`EthernetInterface`] trait. The raw L2
//!   (`AF_PACKET`) socket needs privileged `unsafe` syscalls, so it lives in
//!   the injected interface impl (e.g. `zerodds-transport-tsn` `live`,
//!   `CAP_NET_RAW`) — keeping this `forbid(unsafe_code)` crate clean, exactly
//!   as the MQTT/AMQP clients keep the broker out.
//!
//! Note that *time-sensitive* (TSN) guarantees — scheduled transmission
//! (802.1Qbv), bounded latency, FRER redundancy, gPTP sync — are enforced by
//! TSN-capable NICs/switches plus external daemons, not by this software.
//! Those guarantees are not validated here as no TSN hardware is currently
//! available; see the `zerodds-transport-tsn` "Hardware-Validierung" docs.
//!
//! The carriers move *bytes*: encode a NetworkMessage with
//! [`crate::binary::to_binary`] (or protect it via `crate::security`) before
//! sending, and decode/`unprotect` after receiving. The [`crate::daemon`]
//! wires this to writer/reader groups.

use alloc::string::String;
use alloc::vec::Vec;

/// The IEEE-registered EtherType for OPC-UA PubSub over Ethernet
/// (Part 14 §7.3.3).
pub const ETHERNET_ETHERTYPE: u16 = 0xB62C;

/// The default UADP-over-UDP port (Part 14 §7.3.5).
pub const DEFAULT_UADP_PORT: u16 = 4840;

/// An error from a transport carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// An underlying I/O failure (message carries the OS description).
    Io(String),
    /// No datagram arrived within the carrier's receive timeout.
    Timeout,
    /// The datagram exceeds the carrier's maximum payload size.
    TooLarge {
        /// The datagram length.
        len: usize,
        /// The carrier's maximum.
        max: usize,
    },
    /// The carrier has been closed.
    Closed,
}

impl core::fmt::Display for TransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(m) => write!(f, "transport I/O error: {m}"),
            Self::Timeout => write!(f, "transport receive timed out"),
            Self::TooLarge { len, max } => {
                write!(f, "datagram of {len} bytes exceeds the {max}-byte limit")
            }
            Self::Closed => write!(f, "transport is closed"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TransportError {}

/// A connectionless carrier for serialised UADP messages (Part 14 §7.3).
pub trait PubSubTransport {
    /// Sends one serialised NetworkMessage as a single datagram.
    ///
    /// # Errors
    /// [`TransportError`] on an I/O failure or an oversized datagram.
    fn send(&self, datagram: &[u8]) -> Result<(), TransportError>;

    /// Receives the next datagram, or [`TransportError::Timeout`] if none
    /// arrives within the carrier's receive window.
    ///
    /// # Errors
    /// [`TransportError`] on timeout, closure or an I/O failure.
    fn receive(&self) -> Result<Vec<u8>, TransportError>;
}

// ---------------------------------------------------------------------------
// MQTT carrier (Part 14 §7.3.4) — broker-agnostic.
// ---------------------------------------------------------------------------

/// The publish/subscribe operations an MQTT client must offer to back an
/// [`MqttTransport`]. Implemented by a concrete client (e.g. the
/// `zerodds-mqtt-bridge` client) over a live broker connection.
pub trait MqttClient {
    /// Publishes `payload` to `topic`.
    ///
    /// # Errors
    /// [`TransportError`] on a publish failure.
    fn publish(&self, topic: &str, payload: &[u8]) -> Result<(), TransportError>;

    /// Returns the next received `(topic, payload)` from the subscribed
    /// topics, or [`TransportError::Timeout`] if none is pending.
    ///
    /// # Errors
    /// [`TransportError`] on timeout, closure or a receive failure.
    fn next_message(&self) -> Result<(String, Vec<u8>), TransportError>;
}

/// Builds the conventional UADP-over-MQTT topic for a WriterGroup
/// (Part 14 §7.3.4): `<prefix>/<publisherId>/<writerGroupName>`.
#[must_use]
pub fn mqtt_topic(prefix: &str, publisher_id: &str, writer_group: &str) -> String {
    let mut t = String::with_capacity(prefix.len() + publisher_id.len() + writer_group.len() + 2);
    t.push_str(prefix);
    t.push('/');
    t.push_str(publisher_id);
    t.push('/');
    t.push_str(writer_group);
    t
}

/// A UADP-over-MQTT carrier (Part 14 §7.3.4) layered on an [`MqttClient`].
#[derive(Debug, Clone)]
pub struct MqttTransport<C: MqttClient> {
    client: C,
    publish_topic: String,
}

impl<C: MqttClient> MqttTransport<C> {
    /// Creates a carrier that publishes to `publish_topic`. The `client` is
    /// expected to already be subscribed to the topics it should receive.
    pub fn new(client: C, publish_topic: impl Into<String>) -> Self {
        Self {
            client,
            publish_topic: publish_topic.into(),
        }
    }

    /// The underlying client.
    pub fn client(&self) -> &C {
        &self.client
    }
}

impl<C: MqttClient> PubSubTransport for MqttTransport<C> {
    fn send(&self, datagram: &[u8]) -> Result<(), TransportError> {
        self.client.publish(&self.publish_topic, datagram)
    }

    fn receive(&self) -> Result<Vec<u8>, TransportError> {
        self.client.next_message().map(|(_topic, payload)| payload)
    }
}

// ---------------------------------------------------------------------------
// AMQP carrier (Part 14 Annex B.3, informative) — broker-agnostic.
// ---------------------------------------------------------------------------

/// The send/receive operations an AMQP client must offer to back an
/// [`AmqpTransport`]. The signature mirrors `zerodds-amqp-endpoint`'s client
/// (`send_to` / `recv_from`), so the existing ZeroDDS AMQP 0.9.1 / 1.0 stack
/// backs this carrier directly.
pub trait AmqpClient {
    /// Sends `payload` to the AMQP `address` (a 1.0 node / 0.9.1
    /// exchange+routing-key).
    ///
    /// # Errors
    /// [`TransportError`] on a send failure.
    fn send_to(&self, address: &str, payload: &[u8]) -> Result<(), TransportError>;

    /// Receives the next message from `address`, or `Ok(None)` if none is
    /// pending.
    ///
    /// # Errors
    /// [`TransportError`] on a receive failure.
    fn recv_from(&self, address: &str) -> Result<Option<Vec<u8>>, TransportError>;
}

/// A UADP-over-AMQP carrier (Part 14 Annex B.3) layered on an [`AmqpClient`].
#[derive(Debug, Clone)]
pub struct AmqpTransport<C: AmqpClient> {
    client: C,
    address: String,
}

impl<C: AmqpClient> AmqpTransport<C> {
    /// Creates a carrier sending to and receiving from `address`.
    pub fn new(client: C, address: impl Into<String>) -> Self {
        Self {
            client,
            address: address.into(),
        }
    }

    /// The underlying client.
    pub fn client(&self) -> &C {
        &self.client
    }
}

impl<C: AmqpClient> PubSubTransport for AmqpTransport<C> {
    fn send(&self, datagram: &[u8]) -> Result<(), TransportError> {
        self.client.send_to(&self.address, datagram)
    }

    fn receive(&self) -> Result<Vec<u8>, TransportError> {
        self.client
            .recv_from(&self.address)?
            .ok_or(TransportError::Timeout)
    }
}

// ---------------------------------------------------------------------------
// Kafka carrier (Part 14 Annex B.2, informative) — broker-agnostic.
// ---------------------------------------------------------------------------

/// The produce/poll operations a Kafka client must offer to back a
/// [`KafkaTransport`] (Part 14 Annex B.2). Broker-agnostic, like
/// [`MqttClient`] / [`AmqpClient`].
pub trait KafkaClient {
    /// Produces `payload` to `topic`.
    ///
    /// # Errors
    /// [`TransportError`] on a produce failure.
    fn produce(&self, topic: &str, payload: &[u8]) -> Result<(), TransportError>;

    /// Polls the next `(topic, payload)` record, or `Ok(None)` if none is
    /// pending.
    ///
    /// # Errors
    /// [`TransportError`] on a poll failure.
    fn poll(&self) -> Result<Option<(String, Vec<u8>)>, TransportError>;
}

/// A UADP-over-Kafka carrier (Part 14 Annex B.2) layered on a [`KafkaClient`].
#[derive(Debug, Clone)]
pub struct KafkaTransport<C: KafkaClient> {
    client: C,
    topic: String,
}

impl<C: KafkaClient> KafkaTransport<C> {
    /// Creates a carrier producing to / consuming from `topic`.
    pub fn new(client: C, topic: impl Into<String>) -> Self {
        Self {
            client,
            topic: topic.into(),
        }
    }

    /// The underlying client.
    pub fn client(&self) -> &C {
        &self.client
    }
}

impl<C: KafkaClient> PubSubTransport for KafkaTransport<C> {
    fn send(&self, datagram: &[u8]) -> Result<(), TransportError> {
        self.client.produce(&self.topic, datagram)
    }

    fn receive(&self) -> Result<Vec<u8>, TransportError> {
        self.client
            .poll()?
            .map(|(_topic, payload)| payload)
            .ok_or(TransportError::Timeout)
    }
}

// ---------------------------------------------------------------------------
// Ethernet carrier (Part 14 §7.3.3) — raw L2 injected.
// ---------------------------------------------------------------------------

/// A raw layer-2 Ethernet interface that sends/receives the payload of OPC-UA
/// PubSub frames (EtherType [`ETHERNET_ETHERTYPE`]). The implementation owns
/// the privileged `AF_PACKET`/`SOCK_RAW` socket and the MAC framing — keeping
/// it out of this `forbid(unsafe_code)` crate, exactly as [`MqttClient`] /
/// [`AmqpClient`] keep the broker out. `zerodds-transport-tsn` (`live`) is a
/// natural backend.
pub trait EthernetInterface {
    /// Sends `payload` as the body of an EtherType-`0xB62C` frame to the
    /// configured peer/multicast MAC.
    ///
    /// # Errors
    /// [`TransportError`] on a send failure.
    fn send_frame(&self, payload: &[u8]) -> Result<(), TransportError>;

    /// Receives the next EtherType-`0xB62C` frame body, or `Ok(None)` if none
    /// is pending.
    ///
    /// # Errors
    /// [`TransportError`] on a receive failure.
    fn recv_frame(&self) -> Result<Option<Vec<u8>>, TransportError>;
}

/// A UADP-over-Ethernet carrier (Part 14 §7.3.3) layered on an
/// [`EthernetInterface`].
#[derive(Debug, Clone)]
pub struct EthernetTransport<E: EthernetInterface> {
    interface: E,
}

impl<E: EthernetInterface> EthernetTransport<E> {
    /// Creates a carrier over `interface`.
    pub fn new(interface: E) -> Self {
        Self { interface }
    }

    /// The underlying interface.
    pub fn interface(&self) -> &E {
        &self.interface
    }
}

impl<E: EthernetInterface> PubSubTransport for EthernetTransport<E> {
    fn send(&self, datagram: &[u8]) -> Result<(), TransportError> {
        self.interface.send_frame(datagram)
    }

    fn receive(&self) -> Result<Vec<u8>, TransportError> {
        self.interface.recv_frame()?.ok_or(TransportError::Timeout)
    }
}

// ---------------------------------------------------------------------------
// std carriers: UDP and in-process loopback.
// ---------------------------------------------------------------------------

#[cfg(feature = "std")]
pub use std_carriers::{LoopbackTransport, UdpTransport};

#[cfg(feature = "std")]
mod std_carriers {
    use super::{PubSubTransport, TransportError};
    use alloc::collections::VecDeque;
    use alloc::sync::Arc;
    use alloc::vec;
    use alloc::vec::Vec;
    use std::io::ErrorKind;
    use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
    use std::string::ToString;
    use std::sync::Mutex;

    /// The largest UADP datagram a UDP carrier accepts (a safe upper bound
    /// below the 65 507-byte IPv4 UDP payload ceiling).
    const UDP_MAX_DATAGRAM: usize = 65_507;

    fn io(e: std::io::Error) -> TransportError {
        match e.kind() {
            ErrorKind::WouldBlock | ErrorKind::TimedOut => TransportError::Timeout,
            _ => TransportError::Io(e.to_string()),
        }
    }

    /// A UADP-over-UDP carrier (Part 14 §7.3.5) for unicast and multicast.
    #[derive(Debug)]
    pub struct UdpTransport {
        socket: UdpSocket,
        destination: SocketAddr,
    }

    impl UdpTransport {
        /// Binds a socket to `local` and targets `destination` for sends.
        ///
        /// # Errors
        /// [`TransportError::Io`] if the socket cannot be bound.
        pub fn bind(local: SocketAddr, destination: SocketAddr) -> Result<Self, TransportError> {
            let socket = UdpSocket::bind(local).map_err(io)?;
            Ok(Self {
                socket,
                destination,
            })
        }

        /// Joins an IPv4 multicast group on the given local interface so that
        /// [`receive`](PubSubTransport::receive) yields multicast datagrams.
        ///
        /// # Errors
        /// [`TransportError::Io`] if the group cannot be joined.
        pub fn join_multicast_v4(
            &self,
            group: Ipv4Addr,
            interface: Ipv4Addr,
        ) -> Result<(), TransportError> {
            self.socket
                .join_multicast_v4(&group, &interface)
                .map_err(io)
        }

        /// Sets the receive timeout; `None` blocks indefinitely.
        ///
        /// # Errors
        /// [`TransportError::Io`] if the option cannot be set.
        pub fn set_read_timeout(
            &self,
            timeout: Option<core::time::Duration>,
        ) -> Result<(), TransportError> {
            self.socket.set_read_timeout(timeout).map_err(io)
        }

        /// The local address the socket is bound to.
        ///
        /// # Errors
        /// [`TransportError::Io`] if the address cannot be queried.
        pub fn local_addr(&self) -> Result<SocketAddr, TransportError> {
            self.socket.local_addr().map_err(io)
        }
    }

    impl PubSubTransport for UdpTransport {
        fn send(&self, datagram: &[u8]) -> Result<(), TransportError> {
            if datagram.len() > UDP_MAX_DATAGRAM {
                return Err(TransportError::TooLarge {
                    len: datagram.len(),
                    max: UDP_MAX_DATAGRAM,
                });
            }
            let sent = self
                .socket
                .send_to(datagram, self.destination)
                .map_err(io)?;
            if sent != datagram.len() {
                return Err(TransportError::Io("short UDP send".to_string()));
            }
            Ok(())
        }

        fn receive(&self) -> Result<Vec<u8>, TransportError> {
            let mut buf = vec![0u8; UDP_MAX_DATAGRAM];
            let (n, _src) = self.socket.recv_from(&mut buf).map_err(io)?;
            buf.truncate(n);
            Ok(buf)
        }
    }

    /// An in-process carrier backed by a shared queue. Clones share the queue,
    /// so a Publisher clone's `send` is read by a Subscriber clone's
    /// `receive` — used by tests and the [`crate::daemon`].
    #[derive(Debug, Clone, Default)]
    pub struct LoopbackTransport {
        queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    }

    impl LoopbackTransport {
        /// Creates an empty loopback carrier.
        #[must_use]
        pub fn new() -> Self {
            Self::default()
        }

        /// Number of datagrams waiting to be received (0 if the lock is
        /// poisoned).
        #[must_use]
        pub fn pending(&self) -> usize {
            self.queue.lock().map(|q| q.len()).unwrap_or(0)
        }
    }

    impl PubSubTransport for LoopbackTransport {
        fn send(&self, datagram: &[u8]) -> Result<(), TransportError> {
            self.queue
                .lock()
                .map_err(|_| TransportError::Closed)?
                .push_back(datagram.to_vec());
            Ok(())
        }

        fn receive(&self) -> Result<Vec<u8>, TransportError> {
            self.queue
                .lock()
                .map_err(|_| TransportError::Closed)?
                .pop_front()
                .ok_or(TransportError::Timeout)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;

    #[test]
    fn mqtt_topic_convention() {
        assert_eq!(
            mqtt_topic("opcua/json", "pub1", "wg1"),
            "opcua/json/pub1/wg1"
        );
    }

    /// A single-threaded mock MQTT client backed by an in-memory inbox.
    struct MockMqtt {
        inbox: RefCell<Vec<(String, Vec<u8>)>>,
    }

    impl MqttClient for MockMqtt {
        fn publish(&self, topic: &str, payload: &[u8]) -> Result<(), TransportError> {
            self.inbox
                .borrow_mut()
                .push((String::from(topic), payload.to_vec()));
            Ok(())
        }

        fn next_message(&self) -> Result<(String, Vec<u8>), TransportError> {
            self.inbox.borrow_mut().pop().ok_or(TransportError::Timeout)
        }
    }

    #[test]
    fn mqtt_transport_round_trip() {
        let t = MqttTransport::new(
            MockMqtt {
                inbox: RefCell::new(Vec::new()),
            },
            "opcua/uadp/pub1/wg1",
        );
        t.send(&[1, 2, 3, 4]).expect("send");
        assert_eq!(t.receive().expect("recv"), alloc::vec![1, 2, 3, 4]);
        assert_eq!(t.receive(), Err(TransportError::Timeout));
    }

    /// A single-threaded mock AMQP client backed by an in-memory inbox keyed
    /// by address.
    struct MockAmqp {
        inbox: RefCell<Vec<(String, Vec<u8>)>>,
    }

    impl AmqpClient for MockAmqp {
        fn send_to(&self, address: &str, payload: &[u8]) -> Result<(), TransportError> {
            self.inbox
                .borrow_mut()
                .push((String::from(address), payload.to_vec()));
            Ok(())
        }

        fn recv_from(&self, address: &str) -> Result<Option<Vec<u8>>, TransportError> {
            let mut inbox = self.inbox.borrow_mut();
            if let Some(pos) = inbox.iter().position(|(a, _)| a == address) {
                Ok(Some(inbox.remove(pos).1))
            } else {
                Ok(None)
            }
        }
    }

    #[test]
    fn amqp_transport_round_trip() {
        let t = AmqpTransport::new(
            MockAmqp {
                inbox: RefCell::new(Vec::new()),
            },
            "/topic/pub1.wg1",
        );
        t.send(&[9, 8, 7]).expect("send");
        assert_eq!(t.receive().expect("recv"), alloc::vec![9, 8, 7]);
        assert_eq!(t.receive(), Err(TransportError::Timeout));
    }

    struct MockKafka {
        log: RefCell<Vec<(String, Vec<u8>)>>,
    }

    impl KafkaClient for MockKafka {
        fn produce(&self, topic: &str, payload: &[u8]) -> Result<(), TransportError> {
            self.log
                .borrow_mut()
                .push((String::from(topic), payload.to_vec()));
            Ok(())
        }

        fn poll(&self) -> Result<Option<(String, Vec<u8>)>, TransportError> {
            Ok(self.log.borrow_mut().pop())
        }
    }

    #[test]
    fn kafka_transport_round_trip() {
        let t = KafkaTransport::new(
            MockKafka {
                log: RefCell::new(Vec::new()),
            },
            "opcua.uadp.wg1",
        );
        t.send(&[4, 5, 6]).expect("send");
        assert_eq!(t.receive().expect("recv"), alloc::vec![4, 5, 6]);
        assert_eq!(t.receive(), Err(TransportError::Timeout));
    }

    struct MockEth {
        wire: RefCell<Vec<Vec<u8>>>,
    }

    impl EthernetInterface for MockEth {
        fn send_frame(&self, payload: &[u8]) -> Result<(), TransportError> {
            self.wire.borrow_mut().push(payload.to_vec());
            Ok(())
        }

        fn recv_frame(&self) -> Result<Option<Vec<u8>>, TransportError> {
            let mut w = self.wire.borrow_mut();
            Ok((!w.is_empty()).then(|| w.remove(0)))
        }
    }

    #[test]
    fn ethernet_transport_round_trip() {
        let t = EthernetTransport::new(MockEth {
            wire: RefCell::new(Vec::new()),
        });
        t.send(&[0xB6, 0x2C]).expect("send");
        assert_eq!(t.receive().expect("recv"), alloc::vec![0xB6, 0x2C]);
        assert_eq!(t.receive(), Err(TransportError::Timeout));
    }

    #[cfg(feature = "std")]
    #[test]
    fn loopback_round_trip_across_clones() {
        let tx = LoopbackTransport::new();
        let rx = tx.clone();
        assert_eq!(tx.receive(), Err(TransportError::Timeout));
        tx.send(&[0xDE, 0xAD]).expect("send");
        tx.send(&[0xBE, 0xEF]).expect("send");
        assert_eq!(rx.pending(), 2);
        assert_eq!(rx.receive().expect("r1"), alloc::vec![0xDE, 0xAD]);
        assert_eq!(rx.receive().expect("r2"), alloc::vec![0xBE, 0xEF]);
        assert_eq!(rx.receive(), Err(TransportError::Timeout));
    }

    #[cfg(feature = "std")]
    #[test]
    fn udp_unicast_localhost_round_trip() {
        use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

        // Bind the receiver first to learn its ephemeral port.
        let recv = UdpTransport::bind(
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
        )
        .expect("bind recv");
        recv.set_read_timeout(Some(core::time::Duration::from_secs(2)))
            .expect("timeout");
        let recv_addr = recv.local_addr().expect("addr");

        let send = UdpTransport::bind(
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
            recv_addr,
        )
        .expect("bind send");

        send.send(&[0x01, 0x02, 0x03]).expect("send");
        assert_eq!(recv.receive().expect("recv"), alloc::vec![0x01, 0x02, 0x03]);
    }
}
