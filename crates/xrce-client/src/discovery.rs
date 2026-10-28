// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Finding an agent (DDS-XRCE 1.0 §8.4.4, §11.2.4, §11.3.4).
//!
//! "This flow is only required when the Client is not pre-configured with
//! the TransportLocator of the XRCE Agent. It allows an XRCE Client to be
//! configured to content one or more TransportLocators (which may include
//! multicast addresses) in order to dynamically discover the presence and
//! actual Address of the Agents." The client sends `GET_INFO(OBJECTID_AGENT)`
//! "periodically to the configured addresses and ports" — a multicast
//! address and port (§11.2.4) or a list of candidate UDP addresses (unicast)
//! — and "shall stop sending the periodic message once it receives a
//! suitable response from an Agent". Over TCP (§11.3.4) the client
//! "periodically attempts to establish TCP connections to the configured
//! addresses and ports"; once a connection is established it sends the
//! `CREATE_CLIENT`, and stops attempting when it gets a suitable response.
//!
//! [`AgentDiscovery`] is the flow without I/O: what to send and when, and
//! what a datagram means. [`discover_over_udp`] and [`connect_over_tcp`]
//! (feature `std`) run it on sockets.
//!
//! A response is suitable when it is an `INFO` for the request with
//! `STATUS_OK` whose `AGENT_Representation` has the major version of this
//! client's protocol; an `INFO` with `STATUS_ERR_INCOMPATIBLE`, or of another
//! version, is another agent's refusal and the search goes on (Figure 10).

use alloc::vec;
use alloc::vec::Vec;
use core::time::Duration;

use zerodds_xrce::encoding::Endianness;
use zerodds_xrce::handshake::{ClientRepresentation, XRCE_VERSION};
use zerodds_xrce::header::{ClientKey, MessageHeader, SESSION_ID_NONE_WITHOUT_CLIENT_KEY};
use zerodds_xrce::header::{SessionId, StreamId};
use zerodds_xrce::object_id::OBJECTID_AGENT;
use zerodds_xrce::object_info::{
    ActivityInfoVariant, BaseObjectRequest, InfoMask, ObjectConfig, ObjectInfo, ResultStatusCode,
};
use zerodds_xrce::serial_number::SerialNumber16;
use zerodds_xrce::submessages::{GetInfoPayload, InfoPayload, Message, SubmessageId};
use zerodds_xrce::transport_locator::TransportLocator;

use crate::ClientError;

/// An agent that answered a discovery `GET_INFO` suitably.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredAgent<A> {
    /// Where the answer came from.
    pub from: A,
    /// `xrce_version` of its `AGENT_Representation`.
    pub version: [u8; 2],
    /// `xrce_vendor_id`.
    pub vendor_id: [u8; 2],
    /// The transport addresses it can be reached at (`address_seq` of its
    /// `AGENT_ActivityInfo`, §7.8.2.2): where `CREATE_CLIENT` goes.
    pub locators: Vec<TransportLocator>,
}

/// The messages and bookkeeping of a discovery over the candidates `A`
/// (UDP socket addresses, or any other handle the caller sends to).
#[derive(Debug, Clone)]
pub struct AgentDiscovery<A> {
    candidates: Vec<A>,
    period: Duration,
    request_id: [u8; 2],
    version: [u8; 2],
    client_info: bool,
    last_sent: Option<Duration>,
    attempts: u32,
    incompatible: u32,
    done: bool,
}

/// A discovery message that is due: `bytes` go to every address of `to`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryRequest<'a, A> {
    /// The encoded `Message` with the `GET_INFO`.
    pub bytes: Vec<u8>,
    /// The candidate addresses.
    pub to: &'a [A],
}

impl<A: Clone> AgentDiscovery<A> {
    /// A discovery that sends to `candidates` every `period`.
    ///
    /// # Errors
    /// [`ClientError::InvalidState`] for no candidate or a period of zero.
    pub fn new(candidates: Vec<A>, period: Duration) -> Result<Self, ClientError> {
        if candidates.is_empty() || period.is_zero() {
            return Err(ClientError::InvalidState);
        }
        Ok(Self {
            candidates,
            period,
            request_id: [0, 1],
            version: XRCE_VERSION,
            client_info: false,
            last_sent: None,
            attempts: 0,
            incompatible: 0,
            done: false,
        })
    }

    /// Sends the `client_info` of §7.8.2.2 with the `GET_INFO`: a
    /// `CLIENT_Representation` of this client's protocol version, with the
    /// `client_key` `CLIENTKEY_INVALID`. An agent of another major version
    /// then answers `STATUS_ERR_INCOMPATIBLE` (Figure 10). Annex A's
    /// `GET_INFO_Payload` has no member for it; an agent that does not read it
    /// answers as without.
    #[must_use]
    pub fn with_client_info(mut self, version: [u8; 2]) -> Self {
        self.version = version;
        self.client_info = true;
        self
    }

    /// Whether a suitable answer was received: the periodic message stops.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// How many times the message was sent.
    #[must_use]
    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    /// How many answers of `STATUS_ERR_INCOMPATIBLE` or of another major
    /// version were received.
    #[must_use]
    pub fn incompatible(&self) -> u32 {
        self.incompatible
    }

    /// The message to send at `now` (the time since the start of the search),
    /// if one is due: the first at once, then every `period`; none after a
    /// suitable answer.
    pub fn poll(&mut self, now: Duration) -> Option<DiscoveryRequest<'_, A>> {
        if self.done {
            return None;
        }
        if let Some(last) = self.last_sent {
            if now.saturating_sub(last) < self.period {
                return None;
            }
        }
        let bytes = self.request().ok()?;
        self.last_sent = Some(now);
        self.attempts += 1;
        Some(DiscoveryRequest {
            bytes,
            to: &self.candidates,
        })
    }

    /// `GET_INFO(OBJECTID_AGENT)` in a message without session and ClientKey
    /// (§7.8.2.2: "may be used prior to establishing a Session").
    fn request(&self) -> Result<Vec<u8>, ClientError> {
        let base = BaseObjectRequest {
            request_id: self.request_id,
            object_id: OBJECTID_AGENT,
        };
        let payload = if self.client_info {
            let info = ObjectInfo {
                activity: None,
                config: Some(ObjectConfig::Client(ClientRepresentation {
                    timestamp: None,
                    version: self.version,
                    vendor_id: [0, 0],
                    client_key: ClientKey([0; 4]),
                    session_id: SESSION_ID_NONE_WITHOUT_CLIENT_KEY,
                    properties: None,
                    requested_mtu: None,
                })),
            };
            GetInfoPayload::with_client_info(base, InfoMask::ALL, &info, Endianness::Little)
                .map_err(|_| ClientError::QueueFull)?
        } else {
            GetInfoPayload::new(base, InfoMask::ALL, Endianness::Little)
        };
        let sm = payload
            .into_submessage()
            .map_err(|_| ClientError::QueueFull)?;
        let header = MessageHeader::without_client_key(
            SessionId(SESSION_ID_NONE_WITHOUT_CLIENT_KEY),
            StreamId::NONE,
            SerialNumber16::new(0),
        )
        .map_err(|_| ClientError::QueueFull)?;
        Message::new(header, vec![sm])
            .and_then(|m| m.encode())
            .map_err(|_| ClientError::QueueFull)
    }

    /// Reads a datagram received from `from`. `Some` for the first suitable
    /// answer (the search is done then); `None` for anything else that
    /// decodes, such as an answer to another request, a refusal, or
    /// something that is no `INFO`.
    ///
    /// # Errors
    /// [`ClientError::Protocol`] if the bytes are no `Message`.
    pub fn on_datagram(
        &mut self,
        from: &A,
        bytes: &[u8],
    ) -> Result<Option<DiscoveredAgent<A>>, ClientError> {
        let msg = Message::decode(bytes).map_err(|_| ClientError::Protocol)?;
        if self.done {
            return Ok(None);
        }
        for sm in &msg.submessages {
            if sm.header.submessage_id != SubmessageId::Info {
                continue;
            }
            let Ok(payload) = InfoPayload::try_from_submessage(sm) else {
                continue;
            };
            let Ok((reply, info)) = payload.parse(sm.header.body_endianness()) else {
                continue;
            };
            if reply.related_request.request_id != self.request_id
                || reply.related_request.object_id != OBJECTID_AGENT
            {
                continue;
            }
            if reply.result.status_code == ResultStatusCode::Incompatible {
                self.incompatible += 1;
                continue;
            }
            if !reply.result.status_code.is_success() {
                continue;
            }
            let Some(ObjectConfig::Agent(agent)) = info.config else {
                continue;
            };
            if agent.version[0] != XRCE_VERSION[0] {
                self.incompatible += 1;
                continue;
            }
            let locators = match info.activity {
                Some(ActivityInfoVariant::Agent { address_seq, .. }) => address_seq,
                _ => Vec::new(),
            };
            self.done = true;
            return Ok(Some(DiscoveredAgent {
                from: from.clone(),
                version: agent.version,
                vendor_id: agent.vendor_id,
                locators,
            }));
        }
        Ok(None)
    }
}

#[cfg(feature = "std")]
pub use net::{TcpTransport, connect_over_tcp, discover_over_udp};

#[cfg(feature = "std")]
mod net {
    use super::{AgentDiscovery, DiscoveredAgent};
    use crate::{ClientError, ClientEvent, ClientState, ClientTransport, XrceClient};
    use alloc::vec::Vec;
    use core::time::Duration;
    use std::io::{ErrorKind, Read, Write};
    use std::net::{SocketAddr, TcpStream, UdpSocket};
    use std::time::Instant;
    use zerodds_xrce::header::ClientKey;

    /// Searches for an agent over UDP: `GET_INFO(OBJECTID_AGENT)` to every
    /// address of `candidates` every `period` — the multicast address and
    /// port (DDS-XRCE 1.0 §11.2.4, `239.255.0.2:7400` by default) or the
    /// candidate unicast addresses — until a suitable answer comes or
    /// `timeout` has passed. The answers arrive unicast, so a multicast
    /// search needs no group membership.
    ///
    /// # Errors
    /// The `std::io::Error` of binding the socket or sending.
    pub fn discover_over_udp(
        candidates: &[SocketAddr],
        period: Duration,
        timeout: Duration,
    ) -> std::io::Result<Option<DiscoveredAgent<SocketAddr>>> {
        let mut discovery = AgentDiscovery::new(candidates.to_vec(), period).map_err(|_| {
            std::io::Error::new(ErrorKind::InvalidInput, "no candidate, or no period")
        })?;
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(Duration::from_millis(20)))?;
        let start = Instant::now();
        let mut buf = [0u8; 65_507];
        while start.elapsed() < timeout {
            if let Some(request) = discovery.poll(start.elapsed()) {
                for to in request.to {
                    // A candidate that cannot be reached now (no route to the
                    // multicast group, say) does not end the search.
                    let _ = socket.send_to(&request.bytes, to);
                }
            }
            match socket.recv_from(&mut buf) {
                Ok((n, from)) => {
                    // A datagram that is no XRCE message is not an answer.
                    if let Ok(Some(agent)) = discovery.on_datagram(&from, &buf[..n]) {
                        return Ok(Some(agent));
                    }
                }
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(None)
    }

    /// The framing of TCP (DDS-XRCE 1.0 §11.3.3): a 2-octet little-endian
    /// length in front of each message.
    #[derive(Debug)]
    pub struct TcpTransport {
        stream: TcpStream,
        received: Vec<u8>,
    }

    impl TcpTransport {
        /// Wraps a connected stream.
        ///
        /// # Errors
        /// The `std::io::Error` of setting the read timeout.
        pub fn new(stream: TcpStream) -> std::io::Result<Self> {
            stream.set_read_timeout(Some(Duration::from_millis(2)))?;
            let _ = stream.set_nodelay(true);
            Ok(Self {
                stream,
                received: Vec::new(),
            })
        }
    }

    impl ClientTransport for TcpTransport {
        fn send(&mut self, payload: &[u8]) -> Result<(), ClientError> {
            let len = u16::try_from(payload.len()).map_err(|_| ClientError::QueueFull)?;
            let mut frame = Vec::with_capacity(payload.len() + 2);
            frame.extend_from_slice(&len.to_le_bytes());
            frame.extend_from_slice(payload);
            self.stream
                .write_all(&frame)
                .map_err(|_| ClientError::Transport)
        }

        fn try_recv(&mut self) -> Result<Option<Vec<u8>>, ClientError> {
            let mut chunk = [0u8; 4096];
            match self.stream.read(&mut chunk) {
                Ok(0) => return Err(ClientError::Transport),
                Ok(n) => self.received.extend_from_slice(&chunk[..n]),
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                Err(_) => return Err(ClientError::Transport),
            }
            if self.received.len() < 2 {
                return Ok(None);
            }
            let len = usize::from(u16::from_le_bytes([self.received[0], self.received[1]]));
            if self.received.len() < 2 + len {
                return Ok(None);
            }
            let frame = self.received[2..2 + len].to_vec();
            self.received.drain(..2 + len);
            Ok(Some(frame))
        }
    }

    /// Searches for an agent over TCP (DDS-XRCE 1.0 §11.3.4): every `period`
    /// the client attempts to connect to each address of `candidates` in turn;
    /// on a connection it sends the `CREATE_CLIENT` and waits up to `period`
    /// for the `STATUS_AGENT`. The first agent that accepts the session ends
    /// the search; its connected client is returned with the address. `None`
    /// after `timeout`.
    ///
    /// # Errors
    /// [`std::io::ErrorKind::InvalidInput`] for no candidate.
    pub fn connect_over_tcp(
        candidates: &[SocketAddr],
        client_key: ClientKey,
        session_id: u8,
        version: [u8; 2],
        period: Duration,
        timeout: Duration,
    ) -> std::io::Result<Option<(SocketAddr, XrceClient<TcpTransport>)>> {
        if candidates.is_empty() || period.is_zero() {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "no candidate, or no period",
            ));
        }
        let start = Instant::now();
        let mut round = Duration::ZERO;
        while start.elapsed() < timeout {
            for address in candidates {
                if start.elapsed() >= timeout {
                    break;
                }
                let Ok(stream) = TcpStream::connect_timeout(address, period) else {
                    continue;
                };
                let Ok(transport) = TcpTransport::new(stream) else {
                    continue;
                };
                let mut client = XrceClient::new(client_key, session_id, transport);
                if client.connect(version).is_err() {
                    continue;
                }
                let attempt = Instant::now();
                while attempt.elapsed() < period && start.elapsed() < timeout {
                    let events = client.poll(start.elapsed()).unwrap_or_default();
                    if events
                        .iter()
                        .any(|e| matches!(e, ClientEvent::ConnectRejected { .. }))
                    {
                        break;
                    }
                    if client.state() == ClientState::Connected {
                        return Ok(Some((*address, client)));
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
            }
            round += period;
            while start.elapsed() < round.min(timeout) {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_xrce::handshake::AgentRepresentation;
    use zerodds_xrce::object_info::ResultStatus;
    use zerodds_xrce::submessages::StatusAgentPayload;
    use zerodds_xrce::transport_locator::TransportLocatorMedium;

    const REQUEST: [u8; 2] = [0, 1];

    fn locator() -> TransportLocator {
        TransportLocator::Medium(TransportLocatorMedium {
            address: [127, 0, 0, 1],
            port: 2018,
        })
    }

    /// The INFO an agent answers a discovery GET_INFO with.
    fn info(request_id: [u8; 2], code: ResultStatusCode, version: [u8; 2]) -> Vec<u8> {
        let info = if code.is_success() {
            ObjectInfo {
                activity: Some(ActivityInfoVariant::Agent {
                    availability: 1,
                    address_seq: vec![locator()],
                }),
                config: Some(ObjectConfig::Agent(AgentRepresentation {
                    timestamp: None,
                    version,
                    vendor_id: [1, 15],
                    properties: None,
                })),
            }
        } else {
            ObjectInfo::default()
        };
        let reply = zerodds_xrce::object_info::BaseObjectReply {
            related_request: BaseObjectRequest {
                request_id,
                object_id: OBJECTID_AGENT,
            },
            result: ResultStatus {
                status_code: code,
                implementation_status: 0,
            },
        };
        let sm = InfoPayload::new(reply, &info, Endianness::Little)
            .unwrap()
            .into_submessage()
            .unwrap();
        let header = MessageHeader::without_client_key(
            SessionId(SESSION_ID_NONE_WITHOUT_CLIENT_KEY),
            StreamId::NONE,
            SerialNumber16::new(0),
        )
        .unwrap();
        Message::new(header, vec![sm]).unwrap().encode().unwrap()
    }

    fn sent_get_info(bytes: &[u8]) -> (MessageHeader, BaseObjectRequest, InfoMask, GetInfoPayload) {
        let msg = Message::decode(bytes).unwrap();
        assert_eq!(msg.submessages.len(), 1);
        let payload = GetInfoPayload::try_from_submessage(&msg.submessages[0]).unwrap();
        let (base, mask) = payload
            .parse(msg.submessages[0].header.body_endianness())
            .unwrap();
        (msg.header, base, mask, payload)
    }

    /// §11.2.4: "the XRCE Client shall send the GET_INFO message
    /// periodically to the configured multicast address and port" (§11.2.4
    /// unicast: to each address of a list), in a message without session and
    /// ClientKey; the first at once, then each period.
    #[test]
    fn get_info_goes_to_the_candidates_every_period() {
        let candidates = vec!["a", "b"];
        let mut d = AgentDiscovery::new(candidates.clone(), Duration::from_secs(1)).unwrap();
        let first = d
            .poll(Duration::ZERO)
            .expect("the first message is due at once");
        assert_eq!(first.to, candidates.as_slice());
        let (header, base, mask, _) = sent_get_info(&first.bytes);
        assert_eq!(header.session_id.0, SESSION_ID_NONE_WITHOUT_CLIENT_KEY);
        assert_eq!(header.stream_id, StreamId::NONE);
        assert!(header.client_key.is_none());
        assert_eq!(base.object_id, OBJECTID_AGENT);
        assert_eq!(mask, InfoMask::ALL);
        assert!(d.poll(Duration::from_millis(999)).is_none());
        assert!(d.poll(Duration::from_secs(1)).is_some());
        assert!(d.poll(Duration::from_millis(1_500)).is_none());
        assert!(d.poll(Duration::from_secs(2)).is_some());
        assert_eq!(d.attempts(), 3);
        assert!(AgentDiscovery::<&str>::new(Vec::new(), Duration::from_secs(1)).is_err());
        assert!(AgentDiscovery::new(candidates, Duration::ZERO).is_err());
    }

    /// "The XRCE Client shall stop sending the periodic message once it
    /// receives a suitable response from an Agent." The answer names the
    /// transport addresses (§7.8.2.2).
    #[test]
    fn a_suitable_answer_ends_the_search() {
        let mut d = AgentDiscovery::new(vec![7u8], Duration::from_secs(1)).unwrap();
        assert!(d.poll(Duration::ZERO).is_some());
        let agent = d
            .on_datagram(&9u8, &info(REQUEST, ResultStatusCode::Ok, XRCE_VERSION))
            .unwrap()
            .expect("suitable");
        assert_eq!(agent.from, 9);
        assert_eq!(agent.version, XRCE_VERSION);
        assert_eq!(agent.vendor_id, [1, 15]);
        assert_eq!(agent.locators, vec![locator()]);
        assert!(d.is_done());
        assert!(d.poll(Duration::from_secs(5)).is_none());
    }

    /// An answer that is not suitable does not end the search: a refusal
    /// (`STATUS_ERR_INCOMPATIBLE`, Figure 10), an agent of another major
    /// version, an answer to another request, an error, and something that
    /// is no INFO. Bytes that are no message are an error.
    #[test]
    fn answers_that_are_not_suitable_are_passed_over() {
        let mut d = AgentDiscovery::new(vec![7u8], Duration::from_secs(1)).unwrap();
        for bytes in [
            info(REQUEST, ResultStatusCode::Incompatible, XRCE_VERSION),
            info(REQUEST, ResultStatusCode::Ok, [2, 0]),
            info([0, 9], ResultStatusCode::Ok, XRCE_VERSION),
            info(REQUEST, ResultStatusCode::InvalidData, XRCE_VERSION),
        ] {
            assert_eq!(d.on_datagram(&9, &bytes).unwrap(), None);
        }
        assert_eq!(d.incompatible(), 2);
        // A STATUS_AGENT is no INFO.
        let mut body = ResultStatus::ok().encode().to_vec();
        body.extend_from_slice(
            &AgentRepresentation {
                timestamp: None,
                version: XRCE_VERSION,
                vendor_id: [0, 0],
                properties: None,
            }
            .encode()
            .unwrap(),
        );
        let sm = StatusAgentPayload {
            representation: body,
        }
        .into_submessage()
        .unwrap();
        let header = MessageHeader::without_client_key(
            SessionId(0x80),
            StreamId::NONE,
            SerialNumber16::new(0),
        )
        .unwrap();
        let status_agent = Message::new(header, vec![sm]).unwrap().encode().unwrap();
        assert_eq!(d.on_datagram(&9, &status_agent).unwrap(), None);
        assert_eq!(d.on_datagram(&9, &[1, 2, 3]), Err(ClientError::Protocol));
        assert!(!d.is_done());
        assert!(d.poll(Duration::ZERO).is_some());
    }

    /// Figure 10: the GET_INFO may carry the `client_info` with the version of
    /// the client, whose `client_key` is `CLIENTKEY_INVALID` (§7.8.2.2).
    #[test]
    fn client_info_names_the_version_and_hides_the_client_key() {
        let mut d = AgentDiscovery::new(vec![7u8], Duration::from_secs(1))
            .unwrap()
            .with_client_info([1, 3]);
        let request = d.poll(Duration::ZERO).unwrap();
        let (_, _, _, payload) = sent_get_info(&request.bytes);
        let info = payload.client_info(Endianness::Little).unwrap().unwrap();
        let Some(ObjectConfig::Client(rep)) = info.config else {
            panic!("no CLIENT_Representation");
        };
        assert_eq!(rep.version, [1, 3]);
        assert_eq!(rep.client_key, ClientKey([0; 4]));
    }

    /// Over real UDP sockets: an unreachable candidate and one that answers;
    /// the search finds the second, and gives up after the timeout where
    /// nothing answers.
    #[cfg(feature = "std")]
    #[test]
    fn the_search_over_udp_finds_the_agent_that_answers() {
        use std::net::UdpSocket;
        let silent = UdpSocket::bind("127.0.0.1:0").unwrap();
        let agent = UdpSocket::bind("127.0.0.1:0").unwrap();
        agent
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let agent_addr = agent.local_addr().unwrap();
        let silent_addr = silent.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut buf = [0u8; 2048];
            let (n, from) = agent.recv_from(&mut buf).unwrap();
            let (_, base, _, _) = sent_get_info(&buf[..n]);
            let answer = info(base.request_id, ResultStatusCode::Ok, XRCE_VERSION);
            agent.send_to(&answer, from).unwrap();
        });
        let found = discover_over_udp(
            &[silent_addr, agent_addr],
            Duration::from_millis(50),
            Duration::from_secs(5),
        )
        .unwrap()
        .expect("the agent answers");
        assert_eq!(found.from, agent_addr);
        assert_eq!(found.locators, vec![locator()]);
        server.join().unwrap();
        // Nobody answers: None after the timeout.
        let none = discover_over_udp(
            &[silent_addr],
            Duration::from_millis(20),
            Duration::from_millis(120),
        )
        .unwrap();
        assert_eq!(none, None);
    }

    /// §11.3.4: the client attempts connections to the candidates; the first
    /// agent that takes the CREATE_CLIENT ends the search, and the client is
    /// connected over that connection.
    #[cfg(feature = "std")]
    #[test]
    fn the_search_over_tcp_connects_to_the_agent_that_accepts() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let closed = {
            let l = TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap()
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let agent_addr = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = std::thread::spawn(move || {
            // Waits for the client for 10 s at most.
            let started = std::time::Instant::now();
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(accepted) => break accepted,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            started.elapsed() < Duration::from_secs(10),
                            "the client never connected"
                        );
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => panic!("accept: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut prefix = [0u8; 2];
            stream.read_exact(&mut prefix).unwrap();
            let mut body = vec![0u8; usize::from(u16::from_le_bytes(prefix))];
            stream.read_exact(&mut body).unwrap();
            let request = Message::decode(&body).unwrap();
            assert_eq!(
                request.submessages[0].header.submessage_id,
                SubmessageId::CreateClient
            );
            let mut answer = ResultStatus::ok().encode().to_vec();
            answer.extend_from_slice(
                &AgentRepresentation {
                    timestamp: None,
                    version: XRCE_VERSION,
                    vendor_id: [0, 0],
                    properties: None,
                }
                .encode()
                .unwrap(),
            );
            let sm = StatusAgentPayload {
                representation: answer,
            }
            .into_submessage()
            .unwrap();
            let header = MessageHeader::without_client_key(
                SessionId(0x80),
                StreamId::NONE,
                SerialNumber16::new(0),
            )
            .unwrap();
            let bytes = Message::new(header, vec![sm]).unwrap().encode().unwrap();
            stream
                .write_all(&u16::try_from(bytes.len()).unwrap().to_le_bytes())
                .unwrap();
            stream.write_all(&bytes).unwrap();
            // Keep the connection until the client has read.
            std::thread::sleep(Duration::from_millis(300));
        });
        let (address, client) = connect_over_tcp(
            &[closed, agent_addr],
            ClientKey([5, 6, 7, 8]),
            0x81,
            XRCE_VERSION,
            Duration::from_millis(200),
            Duration::from_secs(5),
        )
        .unwrap()
        .expect("the agent accepts");
        assert_eq!(address, agent_addr);
        assert_eq!(client.state(), crate::ClientState::Connected);
        server.join().unwrap();
    }
}
