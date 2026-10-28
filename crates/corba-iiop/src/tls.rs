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
use rustls::server::WebPkiClientVerifier;
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

/// How a server verifies a client certificate — the transport-level
/// half of SSLIOP's association options (OMG Security/SSLIOP module,
/// the `AssociationOptions` bitmask `corba-csiv2::AssociationOptions`
/// also models): `EstablishTrustInClient` in `target_requires` is
/// [`Required`](Self::Required), in `target_supports` only is
/// [`Optional`](Self::Optional), absent from both is [`None`](Self::None).
/// The mapping from a daemon's own `target_supports`/`target_requires`
/// bits to one of these three is the caller's — this module has no
/// AssociationOptions dependency of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClientAuth {
    /// No client certificate is requested. `load_server_config`'s
    /// long-standing behavior.
    #[default]
    None,
    /// A client certificate is requested and, if presented, must
    /// validate against the configured roots; a connection without one
    /// is still accepted (`EstablishTrustInClient` in
    /// `target_supports`, not in `target_requires`).
    Optional,
    /// A client certificate is required and must validate; the
    /// handshake fails without one (`EstablishTrustInClient` in
    /// `target_requires`).
    Required,
}

/// Builds a `ServerConfig` from a PEM cert chain + PEM key, without
/// client authentication ([`ClientAuth::None`]). Kept as the simple
/// entry point most callers (no mutual TLS) want; equivalent to
/// [`load_server_config_with_client_auth`]`(cert_pem, key_pem, &[],
/// ClientAuth::None)`.
///
/// # Errors
/// PEM parse or rustls config error.
pub fn load_server_config(cert_pem: &[u8], key_pem: &[u8]) -> Result<Arc<ServerConfig>, IiopError> {
    load_server_config_with_client_auth(cert_pem, key_pem, &[], ClientAuth::None)
}

/// Builds a `ServerConfig` from a PEM cert chain + PEM key, verifying a
/// client certificate as `mode` says (SSLIOP association options:
/// [`ClientAuth::Required`] for `EstablishTrustInClient` in
/// `target_requires`, [`ClientAuth::Optional`] for `target_supports`
/// only). `client_ca_pem` is the trust anchor(s) client certificates are
/// checked against — required (non-empty) for
/// [`ClientAuth::Optional`]/[`ClientAuth::Required`], ignored for
/// [`ClientAuth::None`].
///
/// # Errors
/// PEM parse or rustls config error; [`ClientAuth::Optional`]/
/// [`ClientAuth::Required`] with an empty or unparsable `client_ca_pem`.
pub fn load_server_config_with_client_auth(
    cert_pem: &[u8],
    key_pem: &[u8],
    client_ca_pem: &[u8],
    mode: ClientAuth,
) -> Result<Arc<ServerConfig>, IiopError> {
    let certs = rustls_pemfile::certs(&mut &cert_pem[..])
        .collect::<Result<alloc::vec::Vec<_>, _>>()
        .map_err(tls_err)?;
    if certs.is_empty() {
        return Err(tls_err("no certificate in PEM"));
    }
    let key = rustls_pemfile::private_key(&mut &key_pem[..])
        .map_err(tls_err)?
        .ok_or_else(|| tls_err("no private key in PEM"))?;
    let provider = Arc::new(provider());
    let builder = ServerConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .map_err(tls_err)?;
    let cfg = match mode {
        ClientAuth::None => builder.with_no_client_auth().with_single_cert(certs, key),
        ClientAuth::Optional | ClientAuth::Required => {
            let mut roots = RootCertStore::empty();
            for c in rustls_pemfile::certs(&mut &client_ca_pem[..]) {
                roots.add(c.map_err(tls_err)?).map_err(tls_err)?;
            }
            if roots.is_empty() {
                return Err(tls_err(
                    "ClientAuth::Optional/Required needs a non-empty client_ca_pem",
                ));
            }
            let roots = Arc::new(roots);
            // The verifier is built with the explicit provider too — its
            // default builder pulls the *process-level* CryptoProvider,
            // ambiguous (panics) when both ring and aws-lc-rs are linked
            // in (same reasoning `zerodds-bridge-security`'s tls module
            // documents for the identical construction).
            let mut verifier_builder = WebPkiClientVerifier::builder_with_provider(roots, provider);
            if mode == ClientAuth::Optional {
                verifier_builder = verifier_builder.allow_unauthenticated();
            }
            let verifier = verifier_builder.build().map_err(tls_err)?;
            builder
                .with_client_cert_verifier(verifier)
                .with_single_cert(certs, key)
        }
    }
    .map_err(tls_err)?;
    Ok(Arc::new(cfg))
}

/// Builds a `ClientConfig` that trusts the cert(s) contained in `ca_pem` as
/// roots (for self-signed test certs: the server cert itself as root),
/// without presenting a client certificate. Equivalent to
/// [`load_client_config`]`(ca_pem, &[], &[])`.
///
/// # Errors
/// PEM parse or rustls config error.
pub fn load_client_config_trusting(ca_pem: &[u8]) -> Result<Arc<ClientConfig>, IiopError> {
    load_client_config(ca_pem, &[], &[])
}

/// As [`load_client_config_trusting`], additionally presenting a client
/// certificate — the client-side half of SSLIOP's
/// `EstablishTrustInClient`: a server with [`ClientAuth::Required`] (or
/// [`ClientAuth::Optional`], to be trusted rather than treated as
/// anonymous) needs one from the peer connecting to it. `client_cert_pem`
/// empty means no certificate is presented (same as
/// [`load_client_config_trusting`]); a non-empty one needs
/// `client_key_pem` too.
///
/// # Errors
/// PEM parse or rustls config error; a non-empty `client_cert_pem` with
/// an empty or unparsable `client_key_pem`.
pub fn load_client_config(
    ca_pem: &[u8],
    client_cert_pem: &[u8],
    client_key_pem: &[u8],
) -> Result<Arc<ClientConfig>, IiopError> {
    let mut roots = RootCertStore::empty();
    for c in rustls_pemfile::certs(&mut &ca_pem[..]) {
        roots.add(c.map_err(tls_err)?).map_err(tls_err)?;
    }
    let builder = ClientConfig::builder_with_provider(Arc::new(provider()))
        .with_safe_default_protocol_versions()
        .map_err(tls_err)?
        .with_root_certificates(roots);
    let cfg = if client_cert_pem.is_empty() {
        builder.with_no_client_auth()
    } else {
        let certs = rustls_pemfile::certs(&mut &client_cert_pem[..])
            .collect::<Result<alloc::vec::Vec<_>, _>>()
            .map_err(tls_err)?;
        if certs.is_empty() {
            return Err(tls_err("no certificate in client_cert_pem"));
        }
        let key = rustls_pemfile::private_key(&mut &client_key_pem[..])
            .map_err(tls_err)?
            .ok_or_else(|| tls_err("no private key in client_key_pem"))?;
        builder.with_client_auth_cert(certs, key).map_err(tls_err)?
    };
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
    use zerodds_corba_giop::{
        Message, Reply, ReplyStatusType, Request, ResponseFlags, ServiceContextList, TargetAddress,
        Version,
    };
    use zerodds_wire::Endianness;

    fn self_signed() -> (alloc::string::String, alloc::string::String) {
        let ck = rcgen::generate_simple_self_signed(alloc::vec!["localhost".to_string()]).unwrap();
        (ck.cert.pem(), ck.key_pair.serialize_pem())
    }

    /// A CA cert/key and, signed by it, a client leaf cert/key —
    /// `(ca_pem, client_cert_pem, client_key_pem)`.
    fn ca_and_signed_client_cert() -> (String, String, String) {
        let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        ca_params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "zerodds-test-ca");
        let ca_key = rcgen::KeyPair::generate().unwrap();
        let ca_cert = ca_params.self_signed(&ca_key).unwrap();

        let mut client_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        client_params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "zerodds-test-client");
        let client_key = rcgen::KeyPair::generate().unwrap();
        let client_cert = client_params
            .signed_by(&client_key, &ca_cert, &ca_key)
            .unwrap();

        (ca_cert.pem(), client_cert.pem(), client_key.serialize_pem())
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

    fn roundtrip(
        server_cfg: Arc<ServerConfig>,
        client_cfg: Arc<ClientConfig>,
    ) -> Result<(), IiopError> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let srv = thread::spawn(move || -> Result<(), IiopError> {
            let (tcp, _) = listener.accept().unwrap();
            let mut conn = accept_tls(tcp, server_cfg)?;
            let req = conn.read_message()?;
            assert!(matches!(req, Message::Request(_)));
            let reply = Message::Reply(Reply {
                request_id: 1,
                reply_status: ReplyStatusType::NoException,
                service_context: ServiceContextList::default(),
                body: alloc::vec![0xAA, 0xBB],
            });
            conn.write_message(Version::V1_2, Endianness::Big, false, &reply)?;
            Ok(())
        });
        let mut client = connect_tls("localhost", addr.port(), "localhost", client_cfg)?;
        let request = Message::Request(Request {
            request_id: 1,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(alloc::vec![0x01]),
            operation: "ping".into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: alloc::vec![1, 2, 3, 4],
        });
        client.write_message(Version::V1_2, Endianness::Big, false, &request)?;
        let reply = client.read_message()?;
        match reply {
            Message::Reply(r) => assert_eq!(r.body, alloc::vec![0xAA, 0xBB]),
            other => panic!("expected Reply, got {other:?}"),
        }
        srv.join().unwrap()
    }

    /// SSLIOP `EstablishTrustInClient` in `target_requires`
    /// ([`ClientAuth::Required`]): a client presenting a CA-signed
    /// certificate is accepted.
    #[test]
    fn required_client_auth_accepts_a_ca_signed_client_cert() {
        let (server_cert, server_key) = self_signed();
        let (ca_pem, client_cert_pem, client_key_pem) = ca_and_signed_client_cert();
        let server_cfg = load_server_config_with_client_auth(
            server_cert.as_bytes(),
            server_key.as_bytes(),
            ca_pem.as_bytes(),
            ClientAuth::Required,
        )
        .unwrap();
        let client_cfg = load_client_config(
            server_cert.as_bytes(),
            client_cert_pem.as_bytes(),
            client_key_pem.as_bytes(),
        )
        .unwrap();
        roundtrip(server_cfg, client_cfg).expect("mutual TLS handshake must succeed");
    }

    /// [`ClientAuth::Required`] refuses a connection with no client
    /// certificate at all.
    #[test]
    fn required_client_auth_refuses_a_missing_client_cert() {
        let (server_cert, server_key) = self_signed();
        let (ca_pem, _client_cert_pem, _client_key_pem) = ca_and_signed_client_cert();
        let server_cfg = load_server_config_with_client_auth(
            server_cert.as_bytes(),
            server_key.as_bytes(),
            ca_pem.as_bytes(),
            ClientAuth::Required,
        )
        .unwrap();
        // No client cert presented.
        let client_cfg = load_client_config_trusting(server_cert.as_bytes()).unwrap();
        let result = roundtrip(server_cfg, client_cfg);
        assert!(result.is_err(), "a missing client cert must be refused");
    }

    /// [`ClientAuth::Required`] refuses a client certificate signed by a
    /// foreign (not configured as trusted) CA.
    #[test]
    fn required_client_auth_refuses_a_foreign_client_cert() {
        let (server_cert, server_key) = self_signed();
        let (ca_pem, _client_cert_pem, _client_key_pem) = ca_and_signed_client_cert();
        // A second, unrelated CA + client cert — not in the server's trust roots.
        let (_foreign_ca_pem, foreign_client_cert_pem, foreign_client_key_pem) =
            ca_and_signed_client_cert();
        let server_cfg = load_server_config_with_client_auth(
            server_cert.as_bytes(),
            server_key.as_bytes(),
            ca_pem.as_bytes(),
            ClientAuth::Required,
        )
        .unwrap();
        let client_cfg = load_client_config(
            server_cert.as_bytes(),
            foreign_client_cert_pem.as_bytes(),
            foreign_client_key_pem.as_bytes(),
        )
        .unwrap();
        let result = roundtrip(server_cfg, client_cfg);
        assert!(result.is_err(), "a foreign-CA client cert must be refused");
    }

    /// [`ClientAuth::Optional`] accepts both a valid client cert and no
    /// client cert at all — only a *presented but invalid* one is
    /// refused, which is exercised by the `Required` foreign-cert case
    /// above (the verifier's validation logic does not depend on
    /// whether an absent cert is tolerated).
    #[test]
    fn optional_client_auth_accepts_both_a_valid_cert_and_none() {
        let (server_cert, server_key) = self_signed();
        let (ca_pem, client_cert_pem, client_key_pem) = ca_and_signed_client_cert();
        let server_cfg = load_server_config_with_client_auth(
            server_cert.as_bytes(),
            server_key.as_bytes(),
            ca_pem.as_bytes(),
            ClientAuth::Optional,
        )
        .unwrap();
        let with_cert = load_client_config(
            server_cert.as_bytes(),
            client_cert_pem.as_bytes(),
            client_key_pem.as_bytes(),
        )
        .unwrap();
        roundtrip(Arc::clone(&server_cfg), with_cert).expect("a valid client cert is accepted");

        let without_cert = load_client_config_trusting(server_cert.as_bytes()).unwrap();
        roundtrip(server_cfg, without_cert).expect("no client cert is also accepted");
    }

    /// [`load_server_config`] (`ClientAuth::None`) still works unchanged
    /// — the pre-existing behavior every non-mTLS caller relies on.
    #[test]
    fn client_auth_none_is_load_server_configs_behavior() {
        let (server_cert, server_key) = self_signed();
        let a = load_server_config(server_cert.as_bytes(), server_key.as_bytes()).unwrap();
        let b = load_server_config_with_client_auth(
            server_cert.as_bytes(),
            server_key.as_bytes(),
            &[],
            ClientAuth::None,
        )
        .unwrap();
        // Both configs accept a client that presents no certificate.
        let client_cfg = load_client_config_trusting(server_cert.as_bytes()).unwrap();
        roundtrip(a, Arc::clone(&client_cfg)).unwrap();
        roundtrip(b, client_cfg).unwrap();
    }

    #[test]
    fn required_client_auth_with_empty_ca_pem_is_refused_at_config_build() {
        let (server_cert, server_key) = self_signed();
        let err = load_server_config_with_client_auth(
            server_cert.as_bytes(),
            server_key.as_bytes(),
            &[],
            ClientAuth::Required,
        )
        .unwrap_err();
        let msg = alloc::format!("{err:?}");
        assert!(msg.contains("client_ca_pem"), "{msg}");
    }
}
