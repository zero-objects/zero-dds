// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! SSLIOP — IIOP over TLS (rustls 0.23, ring provider). Provides ServerConfig/
//! ClientConfig builders from PEM as well as `connect_tls`/`accept_tls`, which
//! wrap a handshaken `rustls::StreamOwned` into a [`Connection`]. The GIOP wire
//! on top of it is identical to plain IIOP — only the transport is
//! TLS-protected. SSLIOP presence is advertised in the IOR via `TAG_SSL_SEC_TRANS`
//! (corba-ior `Ssl` component); the caller picks the TLS path when the target
//! IOR carries this component.

use std::net::TcpStream;
use std::sync::Arc;

use rustls::pki_types::ServerName;
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection, StreamOwned,
};

use crate::connection::Connection;
use crate::error::IiopError;

fn tls_err(msg: impl core::fmt::Display) -> IiopError {
    IiopError::Other(alloc::format!("TLS: {msg}"))
}

fn provider() -> rustls::crypto::CryptoProvider {
    rustls::crypto::ring::default_provider()
}

/// Builds a `ServerConfig` (without client auth) from a PEM cert chain + PEM key.
///
/// # Errors
/// PEM parse or rustls config error.
pub fn load_server_config(cert_pem: &[u8], key_pem: &[u8]) -> Result<Arc<ServerConfig>, IiopError> {
    let certs = rustls_pemfile::certs(&mut &cert_pem[..])
        .collect::<Result<alloc::vec::Vec<_>, _>>()
        .map_err(tls_err)?;
    if certs.is_empty() {
        return Err(tls_err("no certificate in PEM"));
    }
    let key = rustls_pemfile::private_key(&mut &key_pem[..])
        .map_err(tls_err)?
        .ok_or_else(|| tls_err("no private key in PEM"))?;
    let cfg = ServerConfig::builder_with_provider(Arc::new(provider()))
        .with_safe_default_protocol_versions()
        .map_err(tls_err)?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(tls_err)?;
    Ok(Arc::new(cfg))
}

/// Builds a `ClientConfig` that trusts the cert(s) contained in `ca_pem` as
/// roots (for self-signed test certs: the server cert itself as root).
///
/// # Errors
/// PEM parse or rustls config error.
pub fn load_client_config_trusting(ca_pem: &[u8]) -> Result<Arc<ClientConfig>, IiopError> {
    let mut roots = RootCertStore::empty();
    for c in rustls_pemfile::certs(&mut &ca_pem[..]) {
        roots.add(c.map_err(tls_err)?).map_err(tls_err)?;
    }
    let cfg = ClientConfig::builder_with_provider(Arc::new(provider()))
        .with_safe_default_protocol_versions()
        .map_err(tls_err)?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(Arc::new(cfg))
}

/// Connects via TLS to `host:port` and returns an SSLIOP [`Connection`].
/// `server_name` must match the server cert's SAN/CN (e.g. `"localhost"`).
///
/// # Errors
/// TCP connect, handshake, or config error.
pub fn connect_tls(
    host: &str,
    port: u16,
    server_name: &str,
    config: Arc<ClientConfig>,
) -> Result<Connection, IiopError> {
    let tcp = TcpStream::connect((host, port))?;
    let sock = tcp.try_clone()?;
    let name = ServerName::try_from(server_name.to_string()).map_err(tls_err)?;
    let conn = ClientConnection::new(config, name).map_err(tls_err)?;
    Connection::from_tls_stream(Box::new(StreamOwned::new(conn, tcp)), sock)
}

/// Wraps an accepted `TcpStream` as a server-side SSLIOP [`Connection`]
/// (the rustls handshake happens lazily on the first IO).
///
/// # Errors
/// Config or IO error.
pub fn accept_tls(tcp: TcpStream, config: Arc<ServerConfig>) -> Result<Connection, IiopError> {
    let sock = tcp.try_clone()?;
    let conn = ServerConnection::new(config).map_err(tls_err)?;
    Connection::from_tls_stream(Box::new(StreamOwned::new(conn, tcp)), sock)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;
    use zerodds_cdr::Endianness;
    use zerodds_corba_giop::{
        Message, Reply, ReplyStatusType, Request, ResponseFlags, ServiceContextList, TargetAddress,
        Version,
    };

    fn self_signed() -> (alloc::string::String, alloc::string::String) {
        let ck = rcgen::generate_simple_self_signed(alloc::vec!["localhost".to_string()]).unwrap();
        (ck.cert.pem(), ck.key_pair.serialize_pem())
    }

    /// SSLIOP roundtrip: GIOP Request/Reply over a real TLS connection
    /// (self-signed, client trusts the server cert as root).
    #[test]
    fn ssliop_request_reply_roundtrip() {
        let (cert, key) = self_signed();
        let server_cfg = load_server_config(cert.as_bytes(), key.as_bytes()).unwrap();
        let client_cfg = load_client_config_trusting(cert.as_bytes()).unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let srv = thread::spawn(move || {
            let (tcp, _) = listener.accept().unwrap();
            let mut conn = accept_tls(tcp, server_cfg).unwrap();
            let req = conn.read_message().unwrap();
            assert!(matches!(req, Message::Request(_)));
            let reply = Message::Reply(Reply {
                request_id: 1,
                reply_status: ReplyStatusType::NoException,
                service_context: ServiceContextList::default(),
                body: alloc::vec![0xAA, 0xBB],
            });
            conn.write_message(Version::V1_2, Endianness::Big, false, &reply)
                .unwrap();
        });

        let mut client = connect_tls("localhost", addr.port(), "localhost", client_cfg).unwrap();
        let request = Message::Request(Request {
            request_id: 1,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(alloc::vec![0x01]),
            operation: "ping".into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
        });
        client
            .write_message(Version::V1_2, Endianness::Big, false, &request)
            .unwrap();
        let reply = client.read_message().unwrap();
        match reply {
            Message::Reply(r) => assert_eq!(r.body, alloc::vec![0xAA, 0xBB]),
            other => panic!("expected Reply, got {other:?}"),
        }
        srv.join().unwrap();
    }

    #[test]
    fn client_rejects_untrusted_cert() {
        // Server cert NOT in the client root → handshake must fail.
        let (cert, key) = self_signed();
        let (other_cert, _) = self_signed();
        let server_cfg = load_server_config(cert.as_bytes(), key.as_bytes()).unwrap();
        let client_cfg = load_client_config_trusting(other_cert.as_bytes()).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let srv = thread::spawn(move || {
            if let Ok((tcp, _)) = listener.accept() {
                if let Ok(mut c) = accept_tls(tcp, server_cfg) {
                    let _ = c.read_message();
                }
            }
        });
        let mut client = connect_tls("localhost", addr.port(), "localhost", client_cfg).unwrap();
        let request = Message::Request(Request {
            request_id: 1,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(alloc::vec![0x01]),
            operation: "ping".into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: alloc::vec![1, 2, 3, 4],
        });
        // The (lazy) handshake fails at the latest on the first write/read.
        let w = client.write_message(Version::V1_2, Endianness::Big, false, &request);
        let r = w.and_then(|()| client.read_message());
        assert!(r.is_err(), "untrusted cert must fail the TLS handshake");
        let _ = srv.join();
    }
}
