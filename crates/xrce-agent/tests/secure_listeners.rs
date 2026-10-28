// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The DTLS and TLS listeners of the agent (DDS-XRCE 1.0 §8.3.2.4, §11.1 (5),
//! §11.2, §11.3): a client that trusts the agent's certificate opens a
//! session over the encrypted transport; a client that does not trust it
//! never gets a session; a plain-text CREATE_CLIENT on the secured port gets
//! no STATUS_AGENT. The agent is the real daemon (`daemon::server::start`),
//! its certificate a self-signed one that `rcgen` wrote to files.

#![cfg(any(feature = "tls", feature = "dtls"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_xrce::handshake::{ClientRepresentation, XRCE_VERSION};
use zerodds_xrce::header::{ClientKey, MessageHeader, SessionId, StreamId};
use zerodds_xrce::object_info::ResultStatusCode;
use zerodds_xrce::serial_number::SerialNumber16;
use zerodds_xrce::submessages::{CreateClientPayload, Message, StatusAgentPayload, SubmessageId};
use zerodds_xrce_agent::daemon::config::{DaemonConfig, ListenerConfig};
use zerodds_xrce_agent::daemon::server;

/// A temporary directory with an agent certificate and key (PEM, PKCS#8),
/// and a second, unrelated certificate for a client that must not trust the
/// agent.
struct Credentials {
    dir: PathBuf,
}

impl Credentials {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("xrce-secure-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["agent", "other"] {
            let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
            std::fs::write(dir.join(format!("{name}.pem")), ck.cert.pem()).unwrap();
            std::fs::write(dir.join(format!("{name}.key")), ck.key_pair.serialize_pem()).unwrap();
        }
        Self { dir }
    }

    fn path(&self, name: &str) -> String {
        self.dir.join(name).to_string_lossy().into_owned()
    }
}

impl Drop for Credentials {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// How long a test body may run. A handshake that never completes, or a read
/// on a connection nobody answers, fails the test; it must not hang the run.
const TEST_DEADLINE_SECONDS: u64 = 60;

/// Runs `body` on a thread of its own and fails when it has not finished
/// after `seconds`. A panic of the body is the panic of the test.
fn bounded(seconds: u64, body: impl FnOnce() + Send + 'static) {
    let handle = std::thread::spawn(body);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
    while !handle.is_finished() {
        assert!(
            std::time::Instant::now() < deadline,
            "the test body is still running after {seconds} s (a handshake or a read without an answer)"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if let Err(panic) = handle.join() {
        std::panic::resume_unwind(panic);
    }
}

fn free_tcp_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn free_udp_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn start_daemon(listener: ListenerConfig) -> server::DaemonHandle {
    let cfg = DaemonConfig {
        discovery_enabled: false,
        listeners: vec![listener],
        ..DaemonConfig::default()
    };
    server::start(cfg, RuntimeConfig::default(), None).expect("daemon start")
}

/// The first datagram of a session: CREATE_CLIENT without a `clientKey` in
/// the header (§8.3.2.1: `session_id` 0x80 and above carry none).
fn create_client_message() -> Message {
    let header =
        MessageHeader::without_client_key(SessionId(0x80), StreamId::NONE, SerialNumber16::new(0))
            .unwrap();
    Message::new(
        header,
        vec![
            CreateClientPayload {
                representation: ClientRepresentation {
                    timestamp: None,
                    version: XRCE_VERSION,
                    vendor_id: [0, 0],
                    client_key: ClientKey([0x5E, 0xC0, 0x00, 0x01]),
                    session_id: 0x81,
                    properties: None,
                    requested_mtu: None,
                },
            }
            .into_submessage()
            .unwrap(),
        ],
    )
    .unwrap()
}

/// The status of a STATUS_AGENT reply.
fn status_of_agent_reply(reply: &Message) -> ResultStatusCode {
    let sm = reply
        .submessages
        .iter()
        .find(|sm| sm.header.submessage_id == SubmessageId::StatusAgent)
        .expect("a STATUS_AGENT submessage");
    let payload = StatusAgentPayload::try_from_submessage(sm).unwrap();
    payload
        .result_and_position(zerodds_xrce::encoding::Endianness::Little)
        .unwrap()
        .0
        .status_code
}

#[cfg(feature = "tls")]
mod tls {
    use super::*;
    use std::io::{Read, Write};
    use zerodds_xrce::transport_tls::{RustlsTlsClient, XrceTlsStream};

    fn start(creds: &Credentials, client_ca_file: Option<String>) -> (server::DaemonHandle, u16) {
        let port = free_tcp_port();
        let daemon = start_daemon(ListenerConfig::Tls {
            bind: format!("127.0.0.1:{port}"),
            cert_file: creds.path("agent.pem"),
            key_file: creds.path("agent.key"),
            client_ca_file,
        });
        (daemon, port)
    }

    /// §11.1 (5), §11.3: a client that trusts the agent's certificate opens
    /// a session over TLS and gets STATUS_AGENT with STATUS_OK.
    #[test]
    fn a_client_that_trusts_the_agents_certificate_opens_a_session() {
        bounded(TEST_DEADLINE_SECONDS, move || {
            let creds = Credentials::new("tls-ok");
            let (_daemon, port) = start(&creds, None);
            let addr = format!("127.0.0.1:{port}").parse().unwrap();
            let mut client = RustlsTlsClient::connect_verified(
                addr,
                "localhost",
                &creds.path("agent.pem"),
                None,
            )
            .unwrap();
            client.send_message(&create_client_message()).unwrap();
            let reply = client.recv_message().unwrap();
            assert_eq!(status_of_agent_reply(&reply), ResultStatusCode::Ok);
        });
    }

    /// §11.1 (5): a client authenticates the agent. One that trusts another
    /// certificate than the agent's does not complete the handshake and
    /// receives nothing.
    #[test]
    fn a_client_that_does_not_trust_the_agents_certificate_gets_no_session() {
        bounded(TEST_DEADLINE_SECONDS, move || {
            let creds = Credentials::new("tls-untrusted");
            let (_daemon, port) = start(&creds, None);
            let addr = format!("127.0.0.1:{port}").parse().unwrap();
            let mut client = RustlsTlsClient::connect_verified(
                addr,
                "localhost",
                &creds.path("other.pem"),
                None,
            )
            .unwrap();
            let sent = client.send_message(&create_client_message());
            let received = sent.and_then(|()| client.recv_message());
            assert!(received.is_err(), "the handshake must fail: {received:?}");
        });
    }

    /// §8.3.2.4: the ClientKey is not sent in the clear. A plain-text
    /// CREATE_CLIENT on the TLS port is not answered with a STATUS_AGENT.
    #[test]
    fn a_plain_text_create_client_on_the_tls_port_is_not_answered() {
        bounded(TEST_DEADLINE_SECONDS, move || {
            let creds = Credentials::new("tls-plain");
            let (_daemon, port) = start(&creds, None);
            let mut tcp = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            tcp.set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let bytes = create_client_message().encode().unwrap();
            let mut framed = (bytes.len() as u16).to_le_bytes().to_vec();
            framed.extend_from_slice(&bytes);
            tcp.write_all(&framed).unwrap();
            let mut got = Vec::new();
            let _ = tcp.read_to_end(&mut got);
            assert!(
                !got.windows(4).any(|w| w == b"XRCE"),
                "the agent answered a plain-text CREATE_CLIENT: {got:02x?}"
            );
        });
    }

    /// With `client_ca_file` the agent asks for a client certificate: one
    /// that chains to the trust anchors gets a session, one without does not.
    #[test]
    fn a_client_certificate_is_required_when_a_client_ca_is_configured() {
        bounded(TEST_DEADLINE_SECONDS, move || {
            let creds = Credentials::new("tls-mtls");
            // The client certificate is `other`; the agent trusts it.
            let (_daemon, port) = start(&creds, Some(creds.path("other.pem")));
            let addr = format!("127.0.0.1:{port}").parse().unwrap();
            let mut with = RustlsTlsClient::connect_verified(
                addr,
                "localhost",
                &creds.path("agent.pem"),
                Some((&creds.path("other.pem"), &creds.path("other.key"))),
            )
            .unwrap();
            with.send_message(&create_client_message()).unwrap();
            assert_eq!(
                status_of_agent_reply(&with.recv_message().unwrap()),
                ResultStatusCode::Ok
            );
            let mut without = RustlsTlsClient::connect_verified(
                addr,
                "localhost",
                &creds.path("agent.pem"),
                None,
            )
            .unwrap();
            let sent = without.send_message(&create_client_message());
            let received = sent.and_then(|()| without.recv_message());
            assert!(
                received.is_err(),
                "no client certificate, no session: {received:?}"
            );
        });
    }

    /// The listener refuses to start with a key that does not belong to the
    /// certificate, and says which files.
    #[test]
    fn a_certificate_that_does_not_match_its_key_is_refused_at_start() {
        bounded(TEST_DEADLINE_SECONDS, move || {
            let creds = Credentials::new("tls-mismatch");
            let cfg = DaemonConfig {
                discovery_enabled: false,
                listeners: vec![ListenerConfig::Tls {
                    bind: format!("127.0.0.1:{}", free_tcp_port()),
                    cert_file: creds.path("agent.pem"),
                    key_file: creds.path("other.key"),
                    client_ca_file: None,
                }],
                ..DaemonConfig::default()
            };
            match server::start(cfg, RuntimeConfig::default(), None) {
                Err(server::ServerError::Bind(m)) => {
                    assert!(m.contains("agent.pem"), "{m}");
                }
                Err(e) => panic!("another error: {e}"),
                Ok(_) => panic!("started with a certificate that does not match its key"),
            }
        });
    }
}

#[cfg(feature = "dtls")]
mod dtls {
    use super::*;
    use zerodds_xrce::transport_dtls::{DtlsLayer, WebrtcDtls};

    fn start(creds: &Credentials, client_ca_file: Option<String>) -> (server::DaemonHandle, u16) {
        let port = free_udp_port();
        let daemon = start_daemon(ListenerConfig::Dtls {
            bind: format!("127.0.0.1:{port}"),
            cert_file: creds.path("agent.pem"),
            key_file: creds.path("agent.key"),
            client_ca_file,
        });
        (daemon, port)
    }

    /// §11.1 (5), §11.2: a client that trusts the agent's certificate opens
    /// a session over DTLS and gets STATUS_AGENT with STATUS_OK.
    #[test]
    fn a_client_that_trusts_the_agents_certificate_opens_a_session() {
        bounded(TEST_DEADLINE_SECONDS, move || {
            let creds = Credentials::new("dtls-ok");
            let (_daemon, port) = start(&creds, None);
            let addr = format!("127.0.0.1:{port}").parse().unwrap();
            let mut client =
                WebrtcDtls::connect_verified(addr, "localhost", &creds.path("agent.pem"), None)
                    .unwrap();
            client
                .send(&create_client_message().encode().unwrap())
                .unwrap();
            let reply = Message::decode(&client.recv().unwrap()).unwrap();
            assert_eq!(status_of_agent_reply(&reply), ResultStatusCode::Ok);
        });
    }

    /// §11.1 (5): a client that trusts another certificate than the agent's
    /// does not complete the DTLS handshake.
    #[test]
    fn a_client_that_does_not_trust_the_agents_certificate_gets_no_session() {
        bounded(TEST_DEADLINE_SECONDS, move || {
            let creds = Credentials::new("dtls-untrusted");
            let (_daemon, port) = start(&creds, None);
            let addr = format!("127.0.0.1:{port}").parse().unwrap();
            let connected =
                WebrtcDtls::connect_verified(addr, "localhost", &creds.path("other.pem"), None);
            assert!(connected.is_err(), "the handshake must fail");
        });
    }

    /// §8.3.2.4: a plain-text CREATE_CLIENT datagram on the DTLS port gets
    /// no STATUS_AGENT.
    #[test]
    fn a_plain_text_create_client_on_the_dtls_port_is_not_answered() {
        bounded(TEST_DEADLINE_SECONDS, move || {
            let creds = Credentials::new("dtls-plain");
            let (_daemon, port) = start(&creds, None);
            let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            socket
                .send_to(
                    &create_client_message().encode().unwrap(),
                    ("127.0.0.1", port),
                )
                .unwrap();
            let mut buf = [0u8; 2048];
            if let Ok((n, _)) = socket.recv_from(&mut buf) {
                assert!(
                    !buf[..n].windows(4).any(|w| w == b"XRCE"),
                    "the agent answered a plain-text CREATE_CLIENT: {:02x?}",
                    &buf[..n]
                );
            }
        });
    }

    /// With `client_ca_file` the agent requires a client certificate.
    #[test]
    fn a_client_certificate_is_required_when_a_client_ca_is_configured() {
        bounded(TEST_DEADLINE_SECONDS, move || {
            let creds = Credentials::new("dtls-mtls");
            let (_daemon, port) = start(&creds, Some(creds.path("other.pem")));
            let addr = format!("127.0.0.1:{port}").parse().unwrap();
            let mut with = WebrtcDtls::connect_verified(
                addr,
                "localhost",
                &creds.path("agent.pem"),
                Some((&creds.path("other.pem"), &creds.path("other.key"))),
            )
            .unwrap();
            with.send(&create_client_message().encode().unwrap())
                .unwrap();
            let reply = Message::decode(&with.recv().unwrap()).unwrap();
            assert_eq!(status_of_agent_reply(&reply), ResultStatusCode::Ok);
            let without =
                WebrtcDtls::connect_verified(addr, "localhost", &creds.path("agent.pem"), None);
            assert!(without.is_err(), "no client certificate, no handshake");
        });
    }
}
