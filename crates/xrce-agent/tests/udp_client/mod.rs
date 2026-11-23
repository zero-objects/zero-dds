// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The client side the UDP tests of the agent share: the sample type they
//! write, a UDP `ClientTransport` and the wait that keeps the client ticking.
#![allow(dead_code)]

use std::net::UdpSocket;
use std::time::{Duration, Instant};

use zerodds_xrce_client::{ClientError, ClientTransport, XrceClient};

/// `@final struct KeyedPoint { @key long id; sequence<octet> payload; };`
/// -- plain XCDR2 (no DHEADER, this type is `@final`): `id`(4 bytes LE) +
/// `payload.length`(4 bytes LE) + `payload` bytes, no trailing padding
/// (nothing follows within the sample).
pub fn encode_keyed_point(id: i32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + payload.len());
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// UDP `ClientTransport`: a connected, non-blocking socket. `try_recv`
/// treats `WouldBlock` as "nothing yet", per the trait's "does not
/// block" contract.
pub struct UdpTransport(pub UdpSocket);

impl ClientTransport for UdpTransport {
    fn send(&mut self, payload: &[u8]) -> Result<(), ClientError> {
        self.0
            .send(payload)
            .map(|_| ())
            .map_err(|_| ClientError::Transport)
    }
    fn try_recv(&mut self) -> Result<Option<Vec<u8>>, ClientError> {
        let mut buf = [0u8; 65_507];
        match self.0.recv(&mut buf) {
            Ok(n) => Ok(Some(buf[..n].to_vec())),
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                Ok(None)
            }
            Err(_) => Err(ClientError::Transport),
        }
    }
}

/// Drives `step` until it returns something non-empty, or panics after
/// `budget`. Ticks `client` (§8.4.14.1 `HEARTBEAT` emission for any
/// reliable stream with unacknowledged in-flight messages) on every
/// iteration before calling `step` — without this, a datagram UDP
/// loopback occasionally drops (real packet loss, not a simulated one)
/// on a `reliable: true` request is never recovered, since the agent
/// never NACKs on its own (§8.4.14.2) and nothing else in the test
/// prompts a resend.
pub fn wait_until<T>(
    client: &mut XrceClient<UdpTransport>,
    start: Instant,
    budget: Duration,
    mut step: impl FnMut(&mut XrceClient<UdpTransport>) -> Vec<T>,
    what: &str,
) -> Vec<T> {
    let wait_start = Instant::now();
    loop {
        client.tick(start.elapsed()).expect("tick");
        let got = step(client);
        if !got.is_empty() {
            return got;
        }
        if wait_start.elapsed() > budget {
            panic!("timed out after {budget:?} waiting for: {what}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Connects a client with `key` over UDP loopback to `agent` and waits for
/// the `STATUS_AGENT` of its `CREATE_CLIENT`.
pub fn connect_udp(
    agent: std::net::SocketAddr,
    key: [u8; 4],
    start: Instant,
) -> XrceClient<UdpTransport> {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("bind client socket");
    socket.set_nonblocking(true).expect("nonblocking");
    socket.connect(agent).expect("connect to the agent");
    let mut client = XrceClient::new(
        zerodds_xrce::header::ClientKey(key),
        0x81,
        UdpTransport(socket),
    );
    client.connect([0x01, 0x00]).expect("send CREATE_CLIENT");
    let events = wait_until(
        &mut client,
        start,
        Duration::from_secs(5),
        |c| c.poll(Duration::ZERO).unwrap(),
        "CREATE_CLIENT handshake",
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, zerodds_xrce_client::ClientEvent::Connected { .. })),
        "expected Connected, got {events:?}"
    );
    client
}

/// Waits for the `STATUS` of `request_id` and returns its code.
pub fn status_of(
    client: &mut XrceClient<UdpTransport>,
    start: Instant,
    request_id: u16,
    what: &str,
) -> zerodds_xrce::object_info::ResultStatusCode {
    let events = wait_until(
        client,
        start,
        Duration::from_secs(10),
        |c| {
            c.poll(Duration::ZERO)
                .unwrap()
                .into_iter()
                .filter_map(|e| match e {
                    zerodds_xrce_client::ClientEvent::Status {
                        request_id: r,
                        code,
                        ..
                    } if r == request_id => Some(code),
                    _ => None,
                })
                .collect()
        },
        what,
    );
    events[0]
}

/// The objects of [`create_endpoints`]: participant 0x001, topic 0x002,
/// publisher and subscriber 0x001, DataWriter and DataReader 0x003.
pub struct Endpoints {
    /// The DataWriter.
    pub writer: zerodds_xrce::object_id::ObjectId,
    /// The DataReader.
    pub reader: zerodds_xrce::object_id::ObjectId,
}

/// Creates a participant on `domain`, the topic `topic` of type
/// `type_name`, a publisher, a subscriber and a RELIABLE DataWriter and
/// DataReader of the topic, each by XML and each answered `STATUS_OK`.
pub fn create_endpoints(
    client: &mut XrceClient<UdpTransport>,
    start: Instant,
    domain: i32,
    topic: &str,
    type_name: &str,
) -> Endpoints {
    use zerodds_xrce::object_id::ObjectId;
    use zerodds_xrce::object_kind::ObjectKind;
    use zerodds_xrce::object_repr::{ObjectVariant, ObjectVariantExtra};
    let participant = ObjectId::new(0x001, ObjectKind::Participant).unwrap();
    let topic_oid = ObjectId::new(0x002, ObjectKind::Topic).unwrap();
    let publisher = ObjectId::new(0x001, ObjectKind::Publisher).unwrap();
    let subscriber = ObjectId::new(0x001, ObjectKind::Subscriber).unwrap();
    let writer = ObjectId::new(0x003, ObjectKind::DataWriter).unwrap();
    let reader = ObjectId::new(0x003, ObjectKind::DataReader).unwrap();
    let under = |parent| ObjectVariantExtra {
        domain_id: None,
        parent: Some(parent),
    };
    let steps: Vec<(ObjectId, String, ObjectVariantExtra)> = vec![
        (
            participant,
            format!("<dds><participant><domain_id>{domain}</domain_id></participant></dds>"),
            ObjectVariantExtra {
                domain_id: Some(i16::try_from(domain).unwrap()),
                parent: None,
            },
        ),
        (
            topic_oid,
            format!("<dds><topic name=\"{topic}\" type_name=\"{type_name}\"/></dds>"),
            under(participant),
        ),
        (publisher, String::new(), under(participant)),
        (subscriber, String::new(), under(participant)),
        (
            writer,
            format!(
                "<dds><data_writer topic_ref=\"0x{:04x}\" reliability=\"RELIABLE\"/></dds>",
                topic_oid.raw()
            ),
            under(publisher),
        ),
        (
            reader,
            format!(
                "<dds><data_reader topic_ref=\"0x{:04x}\" reliability=\"RELIABLE\"/></dds>",
                topic_oid.raw()
            ),
            under(subscriber),
        ),
    ];
    for (oid, xml, extra) in steps {
        let rid = client
            .create_object(
                oid,
                ObjectVariant::ByXmlString(xml),
                extra,
                false,
                false,
                true,
            )
            .unwrap();
        let code = status_of(client, start, rid, "CREATE");
        assert!(code.is_success(), "CREATE {:04x}: {code:?}", oid.raw());
    }
    Endpoints { writer, reader }
}
