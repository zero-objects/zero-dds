// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! CSIv2 GSSUP authentication (spec §10/§24) e2e: a server requires
//! credentials; a client attaches a SAS EstablishContext
//! (GSSUP username/password) as service context id 15 to every request. Correct
//! credentials → the call goes through; wrong/missing → `NO_PERMISSION`.

use zerodds_corba_interop::runtime::{CorbaServer, IiopCorbaConnection, object_reference};
use zerodds_corba_interop::string_argument;
use zerodds_corba_interop::{decode_string_body, encode_string_body};
use zerodds_corba_rust::{CorbaConnection, SkeletonResult};
use zerodds_wire::Endianness;

#[test]
fn csiv2_gssup_authenticates_and_rejects() {
    let server = CorbaServer::new();
    server.register(b"Echo", |op, body, _e| {
        if op == "ping" {
            SkeletonResult::Reply(body.bytes().to_vec())
        } else {
            SkeletonResult::BadOperation
        }
    });
    // GSSUP validator: only alice/secret.
    server.require_credentials(|user, pass| user == "alice" && pass == "secret");
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let ior = object_reference("IDL:Echo:1.0", &addr.ip().to_string(), addr.port(), b"Echo");

    // Correct credentials → the authenticated call goes through.
    let good = IiopCorbaConnection::new().with_csiv2_credentials("alice", "secret");
    let (body, _e) = good
        .invoke(&ior, "ping", Endianness::Big, &string_argument("hi"))
        .expect("authenticated call must succeed");
    assert_eq!(decode_string_body(&body).unwrap(), "hi");

    // Wrong password → NO_PERMISSION (Err).
    let bad = IiopCorbaConnection::new().with_csiv2_credentials("alice", "wrong");
    assert!(
        bad.invoke(&ior, "ping", Endianness::Big, &string_argument("hi"))
            .is_err(),
        "wrong password must be rejected"
    );

    // No credentials at all → NO_PERMISSION (Err).
    let anon = IiopCorbaConnection::new();
    assert!(
        anon.invoke(&ior, "ping", Endianness::Big, &string_argument("hi"))
            .is_err(),
        "missing security context must be rejected"
    );

    acceptor.shutdown();
}

/// Without `require_credentials` the server stays open (CSIv2 is opt-in) —
/// regression guard against accidental mandatory auth.
#[test]
fn csiv2_is_opt_in_server_open_by_default() {
    let server = CorbaServer::new();
    server.register(b"Echo", |op, body, _e| {
        if op == "ping" {
            SkeletonResult::Reply(body.bytes().to_vec())
        } else {
            SkeletonResult::BadOperation
        }
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let ior = object_reference("IDL:Echo:1.0", &addr.ip().to_string(), addr.port(), b"Echo");

    let plain = IiopCorbaConnection::new();
    let (body, _e) = plain
        .invoke(&ior, "ping", Endianness::Big, &string_argument("hi"))
        .expect("open server accepts unauthenticated call");
    assert_eq!(decode_string_body(&body).unwrap(), "hi");
    acceptor.shutdown();
}

/// CORBA 3.3 Part 2 §10.6.1.2.3 and §10.6.2.1 on the client side: the
/// `EstablishContext` of every request carries the asserted identity (a
/// GSSUP exported name) and the pushed authorization token, and the
/// target's GSSUP evaluator accepts the identity only when it is the
/// authenticated user itself or its trust rules allow it.
#[test]
fn corba_p2_10_6_2_1_a_client_asserts_an_identity_and_pushes_an_authorization_token() {
    use std::sync::{Arc, Mutex};
    use zerodds_corba_csiv2::identity::exported_gssup_name;
    use zerodds_corba_csiv2::sas::{AuthorizationElement, IdentityToken, SasMessage};
    use zerodds_corba_csiv2::{
        AssertedIdentity, AssertionTrust, GssupUserStore, IDENTITY_TYPE_PRINCIPAL_NAME, SasOutcome,
        ScopedUsername, authenticate_gssup, authenticate_gssup_with,
    };

    let seen: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
    let server = CorbaServer::new();
    server.register(b"Echo", |_op, body, _e| {
        SkeletonResult::Reply(body.bytes().to_vec())
    });
    {
        let seen = seen.clone();
        server.on_request_contexts(move |ctxs| {
            let sas = ctxs.0.iter().find(|c| c.context_id == 15);
            *seen.lock().unwrap() = sas.map(|c| c.context_data.clone());
        });
    }
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let ior = object_reference("IDL:Echo:1.0", &addr.ip().to_string(), addr.port(), b"Echo");

    let end_user = ScopedUsername::parse("end-user@REALM").unwrap();
    let ac = AuthorizationElement {
        the_type: 0x4f4d_0001, // OMG VMCID, X.509 attribute certificate chain
        the_element: vec![0x30, 0x03, 0x02, 0x01, 0x01],
    };
    let conn = IiopCorbaConnection::new()
        .with_csiv2_credentials("gateway", "pw")
        .with_csiv2_identity_assertion(IdentityToken::PrincipalName(exported_gssup_name(&end_user)))
        .with_csiv2_authorization_token(vec![ac.clone()]);
    conn.invoke(&ior, "ping", Endianness::Big, &string_argument("x"))
        .unwrap();
    let bytes = seen.lock().unwrap().clone().expect("a SAS context");
    let SasMessage::EstablishContext(ec) = SasMessage::decode_encapsulation(&bytes).unwrap() else {
        panic!("an EstablishContext");
    };
    assert_eq!(ec.authorization_token, vec![ac]);
    assert_eq!(
        AssertedIdentity::from_token(&ec.identity_token).unwrap(),
        Some(AssertedIdentity::GssupPrincipal(end_user.clone()))
    );

    // The target side: gateway authenticates; asserting end-user needs a rule.
    let line = zerodds_corba_csiv2::gssup_store::format_entry("gateway", "pw", b"salt", 1000);
    let store = GssupUserStore::parse(&line).unwrap();
    assert!(matches!(
        authenticate_gssup(&bytes, &store, IDENTITY_TYPE_PRINCIPAL_NAME),
        SasOutcome::Rejected { .. }
    ));
    let mut trust = AssertionTrust::new();
    trust.allow(
        ScopedUsername::parse("gateway").unwrap(),
        AssertedIdentity::GssupPrincipal(end_user),
    );
    match authenticate_gssup_with(&bytes, &store, IDENTITY_TYPE_PRINCIPAL_NAME, &trust) {
        SasOutcome::Accepted {
            principal,
            asserted,
            ..
        } => {
            assert_eq!(principal, "end-user@REALM");
            assert!(asserted);
        }
        other => panic!("expected Accepted, got {other:?}"),
    }
}

/// One request with `contexts` over a raw connection; the reply.
fn raw_request(
    addr: std::net::SocketAddr,
    contexts: zerodds_corba_giop::ServiceContextList,
) -> zerodds_corba_giop::Reply {
    use zerodds_corba_giop::{Message, Request, ResponseFlags, TargetAddress, Version};
    let stream = std::net::TcpStream::connect(addr).unwrap();
    let mut conn = zerodds_corba_iiop::Connection::from_stream(stream).unwrap();
    let req = Message::Request(Request {
        request_id: 7,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(b"Echo".to_vec()),
        operation: "ping".into(),
        requesting_principal: None,
        service_context: contexts,
        body: encode_string_body("x"),
    });
    conn.write_message(Version::V1_2, Endianness::Big, false, &req)
        .unwrap();
    match conn.read_message().unwrap() {
        Message::Reply(r) => r,
        other => panic!("expected a Reply, got {:?}", other.message_type()),
    }
}

/// CORBA 3.3 Part 2 §10.3.1.1 and Table 10.4 on the target side of a
/// `CorbaServer` that requires credentials: an identity token is
/// evaluated against the supported identity types and the trust rules
/// (backward trust evaluation); a refused assertion is `NO_PERMISSION`
/// with a `ContextError` service context, an accepted one is answered
/// with `CompleteEstablishContext`, and the dispatcher sees the
/// invocation principal.
#[test]
fn corba_p2_10_3_1_1_a_corba_server_evaluates_the_asserted_identity() {
    use zerodds_corba_csiv2::identity::exported_gssup_name;
    use zerodds_corba_csiv2::sas::{EstablishContext, IdentityToken, SasMessage};
    use zerodds_corba_csiv2::{
        AssertedIdentity, AssertionTrust, CONTEXT_ERROR_INVALID_EVIDENCE,
        CONTEXT_ERROR_INVALID_MECHANISM, GssupCredentialToken, IDENTITY_TYPE_PRINCIPAL_NAME,
        ScopedUsername,
    };
    use zerodds_corba_giop::{ServiceContext, ServiceContextList};
    use zerodds_corba_interop::runtime::received_principal;
    use zerodds_corba_rust::{CorbaException, SystemExceptionKind};

    let server = CorbaServer::new();
    server.register(b"Echo", |_op, _body, _e| {
        let p = received_principal().expect("a principal while dispatching");
        SkeletonResult::Reply(encode_string_body(&format!(
            "{}/{}",
            p.principal, p.asserted
        )))
    });
    server.require_credentials(|user, pass| user == "gateway" && pass == "pw");
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let ior = object_reference("IDL:Echo:1.0", &addr.ip().to_string(), addr.port(), b"Echo");
    let end_user = ScopedUsername::parse("end-user@REALM").unwrap();
    let assert_end_user = IdentityToken::PrincipalName(exported_gssup_name(&end_user));
    let call = |identity: Option<IdentityToken>| {
        let mut conn = IiopCorbaConnection::new().with_csiv2_credentials("gateway", "pw");
        if let Some(identity) = identity {
            conn = conn.with_csiv2_identity_assertion(identity);
        }
        conn.invoke(&ior, "ping", Endianness::Big, &string_argument("x"))
            .map(|(body, _)| decode_string_body(&body).unwrap())
    };
    let no_permission = |r: Result<String, CorbaException>| {
        let err = r.expect_err("the request must be refused");
        assert_eq!(
            err.as_system().map(|e| e.kind),
            Some(SystemExceptionKind::NoPermission)
        );
    };

    // Without an identity token the GSSUP user is the principal.
    assert_eq!(call(None).unwrap(), "gateway/false");
    // No identity type is supported by default: invalid mechanism.
    no_permission(call(Some(assert_end_user.clone())));
    server.set_identity_assertion(IDENTITY_TYPE_PRINCIPAL_NAME, AssertionTrust::new());
    // A user asserts itself (§10.3.1.1: exactly the same identity).
    let me = ScopedUsername::parse("gateway").unwrap();
    assert_eq!(
        call(Some(IdentityToken::PrincipalName(exported_gssup_name(&me)))).unwrap(),
        "gateway/true"
    );
    // Another identity needs a trust rule.
    no_permission(call(Some(assert_end_user.clone())));
    let mut trust = AssertionTrust::new();
    trust.allow(me, AssertedIdentity::GssupPrincipal(end_user));
    server.set_identity_assertion(IDENTITY_TYPE_PRINCIPAL_NAME, trust);
    assert_eq!(
        call(Some(assert_end_user.clone())).unwrap(),
        "end-user@REALM/true"
    );

    // The SAS reply in the reply's service contexts.
    let sas_contexts = |identity: IdentityToken| {
        let gssup = GssupCredentialToken::new("gateway".into(), "pw".into(), Vec::new())
            .to_gss_token(Endianness::Big)
            .unwrap();
        let ec = SasMessage::EstablishContext(EstablishContext {
            client_context_id: 0,
            authorization_token: Vec::new(),
            identity_token: identity,
            client_authentication_token: gssup,
        });
        ServiceContextList(vec![ServiceContext::new(
            15,
            ec.encode_encapsulation(Endianness::Big).unwrap(),
        )])
    };
    let sas_of = |reply: &zerodds_corba_giop::Reply| {
        let sc = reply
            .service_context
            .0
            .iter()
            .find(|c| c.context_id == 15)
            .expect("a SAS reply context");
        SasMessage::decode_encapsulation(&sc.context_data).unwrap()
    };
    let accepted = raw_request(addr, sas_contexts(assert_end_user));
    assert!(matches!(
        sas_of(&accepted),
        SasMessage::CompleteEstablishContext(_)
    ));
    let stranger = ScopedUsername::parse("stranger").unwrap();
    let refused = raw_request(
        addr,
        sas_contexts(IdentityToken::PrincipalName(exported_gssup_name(&stranger))),
    );
    match sas_of(&refused) {
        SasMessage::ContextError(e) => assert_eq!(
            (e.major_status, e.minor_status),
            CONTEXT_ERROR_INVALID_EVIDENCE
        ),
        other => panic!("expected a ContextError, got {other:?}"),
    }
    let x509 = raw_request(
        addr,
        sas_contexts(IdentityToken::X509CertChain(vec![0x30, 0x00])),
    );
    match sas_of(&x509) {
        SasMessage::ContextError(e) => assert_eq!(
            (e.major_status, e.minor_status),
            CONTEXT_ERROR_INVALID_MECHANISM
        ),
        other => panic!("expected a ContextError, got {other:?}"),
    }
    acceptor.shutdown();
}

/// CORBA 3.3 Part 2 §10.6.1.1 (with TLS 1.2 and 1.3 in place of SSL 3.0
/// and TLS 1.0, see the §10.6 coverage entry): the SAS context of a
/// request over an SSLIOP connection is evaluated like over IIOP —
/// GSSUP credentials, a refused wrong password and an identity
/// assertion with a trust rule — once with a TLS 1.3 server and once
/// with a server that offers only TLS 1.2.
#[test]
fn corba_p2_10_6_1_1_the_sas_context_is_evaluated_over_tls_1_2_and_1_3() {
    use std::sync::Arc;
    use zerodds_corba_csiv2::identity::exported_gssup_name;
    use zerodds_corba_csiv2::sas::IdentityToken;
    use zerodds_corba_csiv2::{
        AssertedIdentity, AssertionTrust, IDENTITY_TYPE_PRINCIPAL_NAME, ScopedUsername,
    };
    use zerodds_corba_interop::runtime::{
        object_reference_from_ior, received_principal, stringify_object_ref_ssl,
    };
    use zerodds_corba_rust::SystemExceptionKind;

    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_pem = ck.cert.pem();
    let tls13 = zerodds_corba_iiop::tls::load_server_config(
        cert_pem.as_bytes(),
        ck.key_pair.serialize_pem().as_bytes(),
    )
    .unwrap();
    let tls12 = Arc::new(
        rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS12])
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![rustls::pki_types::CertificateDer::from(
                ck.cert.der().to_vec(),
            )],
            rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                ck.key_pair.serialize_der(),
            )),
        )
        .unwrap(),
    );
    for (version, tls) in [("1.3", tls13), ("1.2", tls12)] {
        let server = CorbaServer::new();
        server.register(b"Echo", |_op, _body, _e| {
            let p = received_principal().expect("a principal while dispatching");
            SkeletonResult::Reply(encode_string_body(&p.principal))
        });
        server.require_credentials(|user, pass| user == "gateway" && pass == "pw");
        let end_user = ScopedUsername::parse("end-user@REALM").unwrap();
        let mut trust = AssertionTrust::new();
        trust.allow(
            ScopedUsername::parse("gateway").unwrap(),
            AssertedIdentity::GssupPrincipal(end_user.clone()),
        );
        server.set_identity_assertion(IDENTITY_TYPE_PRINCIPAL_NAME, trust);
        let acceptor = server
            .serve_tls("127.0.0.1:0".parse().unwrap(), tls)
            .unwrap();
        let port = acceptor.listen_addr().port();
        let ior = object_reference_from_ior(&stringify_object_ref_ssl(
            "IDL:Echo:1.0",
            "127.0.0.1",
            port,
            b"Echo",
        ))
        .unwrap();
        let client = |user: &str, pass: &str| {
            IiopCorbaConnection::with_client_tls(cert_pem.as_bytes(), "localhost")
                .unwrap()
                .with_csiv2_credentials(user, pass)
        };

        let (body, _) = client("gateway", "pw")
            .invoke(&ior, "ping", Endianness::Big, &string_argument("x"))
            .unwrap_or_else(|e| panic!("TLS {version}: {e:?}"));
        assert_eq!(
            decode_string_body(&body).unwrap(),
            "gateway",
            "TLS {version}"
        );

        let err = client("gateway", "wrong")
            .invoke(&ior, "ping", Endianness::Big, &string_argument("x"))
            .expect_err("a wrong password is refused");
        assert_eq!(
            err.as_system().map(|e| e.kind),
            Some(SystemExceptionKind::NoPermission),
            "TLS {version}"
        );

        let (body, _) = client("gateway", "pw")
            .with_csiv2_identity_assertion(IdentityToken::PrincipalName(exported_gssup_name(
                &end_user,
            )))
            .invoke(&ior, "ping", Endianness::Big, &string_argument("x"))
            .unwrap_or_else(|e| panic!("TLS {version}: {e:?}"));
        assert_eq!(
            decode_string_body(&body).unwrap(),
            "end-user@REALM",
            "TLS {version}"
        );
        acceptor.shutdown();
    }
}
