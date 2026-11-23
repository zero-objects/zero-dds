// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The client side of the SASL layer (AMQP 1.0 Part 5 §5.3; daemon spec
//! §7.2): the mechanisms PLAIN, SCRAM-SHA-256, SCRAM-SHA-512, EXTERNAL,
//! ANONYMOUS and XOAUTH2, the challenge of SCRAM and the outcome.

use std::io::{self, Write};
use std::string::{String, ToString};
use std::vec::Vec;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::frame::FrameType;
use zerodds_amqp_bridge::performatives::{
    decode_performative, descriptor, sasl_init, sasl_mechanisms_from_body, sasl_outcome_code,
    sasl_plain_response, sasl_response,
};

use super::{FrameReader, SASL_HEADER, Wire, codec, protocol, write_frame};
use crate::scram::ScramHash;
use crate::terminus::field;

/// How the client authenticates in the SASL layer (Part 5, daemon spec
/// §7.2). The broker must offer the mechanism; there is no fallback to a
/// weaker one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaslCredentials {
    /// `ANONYMOUS` (RFC 4505).
    Anonymous,
    /// `PLAIN` (RFC 4616).
    Plain {
        /// Authentication identity.
        username: String,
        /// Password.
        password: String,
    },
    /// `SCRAM-SHA-256` (RFC 7677); the server signature is verified.
    ScramSha256 {
        /// Authentication identity.
        username: String,
        /// Password.
        password: String,
    },
    /// `SCRAM-SHA-512` (RFC 5802 over SHA-512); the server signature is
    /// verified.
    ScramSha512 {
        /// Authentication identity.
        username: String,
        /// Password.
        password: String,
    },
    /// `EXTERNAL` (RFC 4422 App. A): the identity of the TLS client
    /// certificate.
    External,
    /// `XOAUTH2`: an OAuth 2.0 bearer token.
    XOAuth2 {
        /// The user the token is for.
        username: String,
        /// The bearer token.
        token: String,
    },
}

impl SaslCredentials {
    /// The SASL mechanism name (Part 5 §5.3.3.2).
    #[must_use]
    pub fn mechanism(&self) -> &'static str {
        match self {
            Self::Anonymous => "ANONYMOUS",
            Self::Plain { .. } => "PLAIN",
            Self::ScramSha256 { .. } => "SCRAM-SHA-256",
            Self::ScramSha512 { .. } => "SCRAM-SHA-512",
            Self::External => "EXTERNAL",
            Self::XOAuth2 { .. } => "XOAUTH2",
        }
    }

    /// The hash of a SCRAM mechanism, `None` for the others.
    fn scram(&self) -> Option<ScramHash> {
        match self {
            Self::ScramSha256 { .. } => Some(ScramHash::Sha256),
            Self::ScramSha512 { .. } => Some(ScramHash::Sha512),
            _ => None,
        }
    }

    /// The authenticated subject the daemon's ACL uses (spec §7.3).
    #[must_use]
    pub fn subject(&self) -> Option<&str> {
        match self {
            Self::Plain { username, .. }
            | Self::ScramSha256 { username, .. }
            | Self::ScramSha512 { username, .. }
            | Self::XOAuth2 { username, .. } => Some(username),
            Self::Anonymous | Self::External => None,
        }
    }
}

fn random_nonce() -> io::Result<String> {
    let mut bytes = [0u8; 18];
    zerodds_bridge_security::tls_provider()
        .secure_random
        .fill(&mut bytes)
        .map_err(|_| io::Error::other("no random source for the SCRAM nonce"))?;
    Ok(crate::scram::b64(&bytes))
}

/// The initial response of the mechanism `creds` names (Part 5 §5.3.3.2),
/// and for SCRAM what the challenge is answered with: the
/// client-first-bare, the nonce and the password (RFC 5802 §5.1). `nonce`
/// is the client nonce of SCRAM; the other mechanisms ignore it.
///
/// PLAIN `\0user\0password` (RFC 4616); ANONYMOUS an empty trace (RFC 4505);
/// EXTERNAL empty, the identity is the TLS client certificate (RFC 4422
/// App. A); XOAUTH2 `user=<u>^Aauth=Bearer <token>^A^A`; SCRAM-SHA-256 and
/// SCRAM-SHA-512 `n,,n=<user>,r=<nonce>` with `=` and `,` of the user name
/// escaped (RFC 5802 §5.1).
pub(super) fn initial_response(
    creds: &SaslCredentials,
    nonce: &str,
) -> (Vec<u8>, Option<(String, String, String)>) {
    match creds {
        SaslCredentials::Anonymous | SaslCredentials::External => (Vec::new(), None),
        SaslCredentials::Plain { username, password } => {
            (sasl_plain_response(username, password), None)
        }
        SaslCredentials::XOAuth2 { username, token } => (
            format!("user={username}\x01auth=Bearer {token}\x01\x01").into_bytes(),
            None,
        ),
        SaslCredentials::ScramSha256 { username, password }
        | SaslCredentials::ScramSha512 { username, password } => {
            let user = username.replace('=', "=3D").replace(',', "=2C");
            let bare = format!("n={user},r={nonce}");
            let first = format!("n,,{bare}");
            (
                first.into_bytes(),
                Some((bare, nonce.to_string(), password.clone())),
            )
        }
    }
}

/// The SASL layer (Part 5 §5.3): the broker offers mechanisms, the client
/// picks the configured one, answers challenges, reads the outcome.
pub(super) fn exchange(
    wire: &mut Wire,
    reader: &mut FrameReader,
    creds: &SaslCredentials,
) -> io::Result<()> {
    wire.write_all(&SASL_HEADER)?;
    let peer = reader.read_exact_header(8)?;
    if peer[..] != SASL_HEADER[..] {
        return Err(protocol(format!(
            "the broker answered {peer:02x?} to the SASL header; it does not offer SASL"
        )));
    }
    let next_sasl = |reader: &mut FrameReader| -> io::Result<(u64, AmqpExtValue)> {
        let f = reader.next()?;
        if f.frame_type != FrameType::Sasl {
            return Err(protocol("expected a SASL frame"));
        }
        let (d, v, _) = decode_performative(&f.body).map_err(codec)?;
        Ok((d, v))
    };
    let (d, v) = next_sasl(reader)?;
    if d != descriptor::SASL_MECHANISMS {
        return Err(protocol("expected sasl-mechanisms"));
    }
    let offered = sasl_mechanisms_from_body(&v);
    let mech = creds.mechanism();
    if !offered.iter().any(|m| m == mech) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "the broker offers SASL {}, not the configured {mech}",
                offered.join(", ")
            ),
        ));
    }
    let hash = creds.scram();
    let nonce = match hash {
        Some(_) => random_nonce()?,
        None => String::new(),
    };
    let (initial, scram) = initial_response(creds, &nonce);
    let init = sasl_init(mech, &initial).map_err(codec)?;
    write_frame(wire, FrameType::Sasl, 0, &init)?;
    let mut server_signature: Option<Vec<u8>> = None;
    // XOAUTH2: the error a broker sends as a challenge before it refuses.
    let mut oauth_error: Option<String> = None;
    loop {
        let (d, v) = next_sasl(reader)?;
        match d {
            descriptor::SASL_CHALLENGE => {
                let challenge = match field(&v, 0) {
                    Some(AmqpExtValue::Binary(b)) => b.clone(),
                    _ => Vec::new(),
                };
                // XOAUTH2: a token the broker does not accept is answered
                // with the error status (JSON) as a challenge; the client
                // sends an empty response and the refusal follows.
                if matches!(creds, SaslCredentials::XOAuth2 { .. }) && oauth_error.is_none() {
                    oauth_error = Some(String::from_utf8_lossy(&challenge).to_string());
                    let resp = sasl_response(&[]).map_err(codec)?;
                    write_frame(wire, FrameType::Sasl, 0, &resp)?;
                    continue;
                }
                let (Some((bare, nonce, password)), Some(hash)) = (scram.as_ref(), hash) else {
                    return Err(protocol(format!("unexpected SASL challenge for {mech}")));
                };
                let server_first = String::from_utf8(challenge)
                    .map_err(|_| protocol("SCRAM server-first is not UTF-8"))?;
                let (fin, signature) =
                    scram_client_final_with(hash, bare, nonce, &server_first, password)?;
                server_signature = Some(signature);
                let resp = sasl_response(fin.as_bytes()).map_err(codec)?;
                write_frame(wire, FrameType::Sasl, 0, &resp)?;
            }
            descriptor::SASL_OUTCOME => {
                return match sasl_outcome_code(&v) {
                    Some(0) => {
                        if let Some(expected) = &server_signature {
                            let data = match field(&v, 1) {
                                Some(AmqpExtValue::Binary(b)) => {
                                    String::from_utf8_lossy(b).to_string()
                                }
                                _ => String::new(),
                            };
                            let v_attr = data.split(',').find_map(|p| p.strip_prefix("v="));
                            let got = v_attr.and_then(crate::scram::b64_decode);
                            if got.as_deref() != Some(&expected[..]) {
                                return Err(io::Error::new(
                                    io::ErrorKind::PermissionDenied,
                                    "SCRAM server signature does not verify; the broker does not know the password",
                                ));
                            }
                        }
                        Ok(())
                    }
                    Some(code) => Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        match &oauth_error {
                            Some(e) => format!("SASL {mech} refused, outcome code {code}: {e}"),
                            None => format!("SASL {mech} refused, outcome code {code}"),
                        },
                    )),
                    None => Err(protocol("sasl-outcome without a code")),
                };
            }
            other => return Err(protocol(format!("unexpected SASL performative {other:#x}"))),
        }
    }
}

/// The SCRAM-SHA-256 client-final message for a server-first challenge,
/// and the server signature the outcome must carry (RFC 5802 §3, §5.1;
/// RFC 7677). `client_first_bare` is `n=<user>,r=<nonce>`.
///
/// # Errors
/// As `scram_client_final_with`.
pub fn scram_client_final(
    client_first_bare: &str,
    client_nonce: &str,
    server_first: &str,
    password: &str,
) -> io::Result<(String, [u8; 32])> {
    let (fin, signature) = scram_client_final_with(
        ScramHash::Sha256,
        client_first_bare,
        client_nonce,
        server_first,
        password,
    )?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&signature);
    Ok((fin, out))
}

/// The client-final message of SCRAM over `hash` for a server-first
/// challenge, and the server signature the outcome must carry (RFC 5802
/// §3, §5.1). `client_first_bare` is `n=<user>,r=<nonce>`.
///
/// # Errors
/// A server-first without nonce, salt or iteration count, a server nonce
/// that does not extend the client's, fewer than 4096 iterations
/// (RFC 7677 §4; the same floor for SHA-512).
pub fn scram_client_final_with(
    hash: ScramHash,
    client_first_bare: &str,
    client_nonce: &str,
    server_first: &str,
    password: &str,
) -> io::Result<(String, Vec<u8>)> {
    let attr = |k: &str| {
        server_first
            .split(',')
            .find_map(|p| p.strip_prefix(k))
            .map(str::to_string)
    };
    let full_nonce = attr("r=").ok_or_else(|| protocol("SCRAM server-first without r="))?;
    if !full_nonce.starts_with(client_nonce) || full_nonce.len() == client_nonce.len() {
        return Err(protocol(
            "SCRAM server nonce does not extend the client nonce",
        ));
    }
    let salt = attr("s=")
        .and_then(|s| crate::scram::b64_decode(&s))
        .ok_or_else(|| protocol("SCRAM server-first without a salt"))?;
    let iterations: u32 = attr("i=")
        .and_then(|s| s.parse().ok())
        .filter(|i| *i >= 4096)
        .ok_or_else(|| protocol("SCRAM iteration count missing or below 4096"))?;
    let without_proof = format!("c=biws,r={full_nonce}");
    let auth_message = crate::scram::auth_message(client_first_bare, server_first, &without_proof);
    let proof = hash.client_proof(password, &salt, iterations, &auth_message);
    let signature = hash.server_signature(password, &salt, iterations, &auth_message);
    Ok((
        format!("{without_proof},p={}", crate::scram::b64(&proof)),
        signature,
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::time::Duration;
    use zerodds_amqp_bridge::performatives::encode_performative;

    /// RFC 7677 §3: user `user`, password `pencil`, the client and server
    /// messages of the RFC; the client-final and the server signature are
    /// the RFC's bytes.
    #[test]
    fn amqpbridge_7_2_scram_client_final_matches_the_rfc_7677_example() {
        let (fin, sig) = scram_client_final(
            "n=user,r=rOprNGfwEbeRWgbNEkqO",
            "rOprNGfwEbeRWgbNEkqO",
            "r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096",
            "pencil",
        )
        .unwrap();
        assert_eq!(
            fin,
            "c=biws,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,p=dHzbZapWIk4jUhN+Ute9ytag9zjfMHgsqmmiz7AndVQ="
        );
        assert_eq!(
            crate::scram::b64(&sig),
            "6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4="
        );
        // A server nonce that does not extend the client's is refused.
        assert!(scram_client_final("n=u,r=abc", "abc", "r=xyz,s=AAAA,i=4096", "p").is_err());
        assert!(scram_client_final("n=u,r=abc", "abc", "r=abcd,s=AAAA,i=1000", "p").is_err());
    }

    /// The broker side of a SASL exchange: it reads the client's header and
    /// answers with `header`; with the SASL header it offers `offered`,
    /// reads the init and hands mechanism and initial response to `script`,
    /// which writes the rest. The join handle yields what the init carried,
    /// `None` when no init came.
    #[allow(clippy::type_complexity)]
    fn scripted(
        header: [u8; 8],
        offered: &[&str],
        script: impl FnOnce(&mut Wire, &mut FrameReader, &str, &[u8]) + Send + 'static,
    ) -> (
        Wire,
        FrameReader,
        std::thread::JoinHandle<Option<(String, Vec<u8>)>>,
    ) {
        let (client, server) = crate::broker::tests::pair();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let offered: Vec<AmqpExtValue> = offered
            .iter()
            .map(|m| AmqpExtValue::Symbol((*m).to_string()))
            .collect();
        let broker = std::thread::spawn(move || {
            let mut wire = Wire::Plain(server.try_clone().unwrap());
            let mut reader = FrameReader {
                wire: Wire::Plain(server),
                buf: Vec::new(),
            };
            reader.read_exact_header(8).ok()?;
            wire.write_all(&header).unwrap();
            if header != SASL_HEADER {
                return None;
            }
            let body = encode_performative(
                descriptor::SASL_MECHANISMS,
                &AmqpExtValue::List(vec![AmqpExtValue::Array(offered)]),
            )
            .unwrap();
            write_frame(&mut wire, FrameType::Sasl, 0, &body).unwrap();
            let init = reader.next().ok()?;
            let (d, v, _) = decode_performative(&init.body).unwrap();
            assert_eq!(d, descriptor::SASL_INIT);
            let mechanism = match field(&v, 0) {
                Some(AmqpExtValue::Symbol(m)) => m.clone(),
                other => panic!("mechanism {other:?}"),
            };
            let response = match field(&v, 1) {
                Some(AmqpExtValue::Binary(b)) => b.clone(),
                _ => Vec::new(),
            };
            script(&mut wire, &mut reader, &mechanism, &response);
            Some((mechanism, response))
        });
        let wire = Wire::Plain(client.try_clone().unwrap());
        let reader = FrameReader {
            wire: Wire::Plain(client),
            buf: Vec::new(),
        };
        (wire, reader, broker)
    }

    fn send(wire: &mut Wire, code: u64, fields: Vec<AmqpExtValue>) {
        let body = encode_performative(code, &AmqpExtValue::List(fields)).unwrap();
        write_frame(wire, FrameType::Sasl, 0, &body).unwrap();
    }

    fn outcome(wire: &mut Wire, code: u8, data: Option<&[u8]>) {
        let mut fields = vec![AmqpExtValue::Ubyte(code)];
        if let Some(d) = data {
            fields.push(AmqpExtValue::Binary(d.to_vec()));
        }
        send(wire, descriptor::SASL_OUTCOME, fields);
    }

    fn plain() -> SaslCredentials {
        SaslCredentials::Plain {
            username: "u".into(),
            password: "p".into(),
        }
    }

    /// Part 5 §5.3.3.2 with RFC 4616, 4505, 4422 App. A, RFC 5802 §5.1 and
    /// the XOAUTH2 form: the initial response of each mechanism.
    #[test]
    fn amqpbridge_12_1_sasl_each_mechanism_has_its_initial_response() {
        assert_eq!(initial_response(&plain(), "x"), (b"\0u\0p".to_vec(), None));
        assert_eq!(
            initial_response(&SaslCredentials::Anonymous, "x"),
            (Vec::new(), None)
        );
        assert_eq!(
            initial_response(&SaslCredentials::External, "x"),
            (Vec::new(), None)
        );
        let oauth = SaslCredentials::XOAuth2 {
            username: "svc".into(),
            token: "tok".into(),
        };
        assert_eq!(
            initial_response(&oauth, "x").0,
            b"user=svc\x01auth=Bearer tok\x01\x01".to_vec()
        );
        let scram = SaslCredentials::ScramSha256 {
            username: "a=b,c".into(),
            password: "pw".into(),
        };
        let (first, state) = initial_response(&scram, "NONCE");
        assert_eq!(first, b"n,,n=a=3Db=2Cc,r=NONCE".to_vec());
        assert_eq!(
            state,
            Some(("n=a=3Db=2Cc,r=NONCE".into(), "NONCE".into(), "pw".into()))
        );
        assert_eq!(
            [
                plain(),
                SaslCredentials::Anonymous,
                SaslCredentials::External,
                oauth,
                scram
            ]
            .map(|c| c.mechanism()),
            ["PLAIN", "ANONYMOUS", "EXTERNAL", "XOAUTH2", "SCRAM-SHA-256"]
        );
    }

    #[test]
    fn amqpbridge_12_1_sasl_the_configured_mechanism_is_sent_and_accepted() {
        let (mut wire, mut reader, broker) =
            scripted(SASL_HEADER, &["ANONYMOUS", "PLAIN"], |w, _, _, _| {
                outcome(w, 0, None);
            });
        exchange(&mut wire, &mut reader, &plain()).unwrap();
        assert_eq!(
            broker.join().unwrap(),
            Some(("PLAIN".to_string(), b"\0u\0p".to_vec()))
        );
    }

    /// §7.2: no fallback to another mechanism; the refusal comes before
    /// any credential is sent.
    #[test]
    fn amqpbridge_12_1_sasl_a_mechanism_not_offered_is_refused_before_the_init() {
        let (mut wire, mut reader, broker) = scripted(SASL_HEADER, &["PLAIN"], |_, _, _, _| {});
        let creds = SaslCredentials::ScramSha256 {
            username: "u".into(),
            password: "p".into(),
        };
        let e = exchange(&mut wire, &mut reader, &creds).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
        assert!(
            e.to_string().contains("not the configured SCRAM-SHA-256"),
            "{e}"
        );
        drop((wire, reader));
        assert_eq!(broker.join().unwrap(), None, "no sasl-init was sent");
    }

    /// Part 5 §5.3.3.6: every code but 0 (ok) is a refusal, and says which.
    #[test]
    fn amqpbridge_12_1_sasl_an_outcome_other_than_ok_is_a_refusal() {
        for code in [1u8, 2, 3, 4] {
            let (mut wire, mut reader, broker) =
                scripted(SASL_HEADER, &["PLAIN"], move |w, _, _, _| {
                    outcome(w, code, None);
                });
            let e = exchange(&mut wire, &mut reader, &plain()).unwrap_err();
            assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
            assert!(
                e.to_string().contains(&format!("outcome code {code}")),
                "{e}"
            );
            broker.join().unwrap();
        }
    }

    /// A challenge belongs to SCRAM alone; PLAIN answered with one is a
    /// broker the daemon does not understand.
    #[test]
    fn amqpbridge_12_1_sasl_a_challenge_for_plain_is_a_protocol_error() {
        let (mut wire, mut reader, broker) = scripted(SASL_HEADER, &["PLAIN"], |w, _, _, _| {
            send(
                w,
                descriptor::SASL_CHALLENGE,
                vec![AmqpExtValue::Binary(b"?".to_vec())],
            );
        });
        let e = exchange(&mut wire, &mut reader, &plain()).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidData);
        assert!(
            e.to_string()
                .contains("unexpected SASL challenge for PLAIN"),
            "{e}"
        );
        broker.join().unwrap();
    }

    /// Part 2 §2.2: a broker that answers the SASL header with the AMQP
    /// header has no SASL layer; the error says so.
    #[test]
    fn amqpbridge_12_1_sasl_a_broker_without_the_layer_is_named() {
        let amqp = [b'A', b'M', b'Q', b'P', 0, 1, 0, 0];
        let (mut wire, mut reader, broker) = scripted(amqp, &[], |_, _, _, _| {});
        let e = exchange(&mut wire, &mut reader, &plain()).unwrap_err();
        assert!(e.to_string().contains("does not offer SASL"), "{e}");
        broker.join().unwrap();
    }

    /// RFC 5802 §5.1: the client answers the server-first with its proof
    /// and accepts the outcome only with the server signature (`v=`) that
    /// proves the broker knows the password.
    #[test]
    fn amqpbridge_12_1_sasl_scram_checks_the_server_signature() {
        for honest in [true, false] {
            let (mut wire, mut reader, broker) =
                scripted(SASL_HEADER, &["SCRAM-SHA-256"], move |w, r, _, first| {
                    let first = String::from_utf8(first.to_vec()).unwrap();
                    let bare = first.strip_prefix("n,,").unwrap().to_string();
                    let nonce = bare.split_once(",r=").unwrap().1.to_string();
                    let server_first = format!(
                        "r={nonce}srv,s={},i=4096",
                        crate::scram::b64(b"salt-of-16-bytes")
                    );
                    send(
                        w,
                        descriptor::SASL_CHALLENGE,
                        vec![AmqpExtValue::Binary(server_first.clone().into_bytes())],
                    );
                    let resp = r.next().unwrap();
                    let (d, v, _) = decode_performative(&resp.body).unwrap();
                    assert_eq!(d, descriptor::SASL_RESPONSE);
                    let (fin, sig) =
                        scram_client_final(&bare, &nonce, &server_first, "pencil").unwrap();
                    assert_eq!(field(&v, 0), Some(&AmqpExtValue::Binary(fin.into_bytes())));
                    let sig = if honest { sig } else { [0u8; 32] };
                    let data = format!("v={}", crate::scram::b64(&sig));
                    outcome(w, 0, Some(data.as_bytes()));
                });
            let creds = SaslCredentials::ScramSha256 {
                username: "user".into(),
                password: "pencil".into(),
            };
            let got = exchange(&mut wire, &mut reader, &creds);
            if honest {
                got.unwrap();
            } else {
                let e = got.unwrap_err();
                assert!(e.to_string().contains("does not verify"), "{e}");
            }
            broker.join().unwrap();
        }
    }

    /// SCRAM-SHA-512: the client answers the server-first with the proof
    /// over SHA-512 and accepts the outcome only with the 64-octet server
    /// signature; a signature of the wrong hash is refused.
    #[test]
    fn amqpbridge_7_2_scram_sha_512_authenticates_and_checks_the_signature() {
        for honest in [true, false] {
            let (mut wire, mut reader, broker) = scripted(
                SASL_HEADER,
                &["SCRAM-SHA-256", "SCRAM-SHA-512"],
                move |w, r, mech, first| {
                    assert_eq!(mech, "SCRAM-SHA-512");
                    let first = String::from_utf8(first.to_vec()).unwrap();
                    let bare = first.strip_prefix("n,,").unwrap().to_string();
                    let nonce = bare.split_once(",r=").unwrap().1.to_string();
                    let salt = b"salt-of-16-bytes";
                    let server_first = format!("r={nonce}srv,s={},i=4096", crate::scram::b64(salt));
                    send(
                        w,
                        descriptor::SASL_CHALLENGE,
                        vec![AmqpExtValue::Binary(server_first.clone().into_bytes())],
                    );
                    let resp = r.next().unwrap();
                    let (_, v, _) = decode_performative(&resp.body).unwrap();
                    let Some(AmqpExtValue::Binary(fin)) = field(&v, 0) else {
                        panic!("{v:?}")
                    };
                    let fin = String::from_utf8(fin.clone()).unwrap();
                    let (without_proof, proof) = fin.rsplit_once(",p=").unwrap();
                    let am = crate::scram::auth_message(&bare, &server_first, without_proof);
                    let hash = ScramHash::Sha512;
                    assert_eq!(
                        crate::scram::b64_decode(proof).unwrap(),
                        hash.client_proof("pencil", salt, 4096, &am),
                        "the proof is over SHA-512"
                    );
                    let sig = if honest {
                        hash.server_signature("pencil", salt, 4096, &am)
                    } else {
                        ScramHash::Sha256.server_signature("pencil", salt, 4096, &am)
                    };
                    let data = format!("v={}", crate::scram::b64(&sig));
                    outcome(w, 0, Some(data.as_bytes()));
                },
            );
            let creds = SaslCredentials::ScramSha512 {
                username: "user".into(),
                password: "pencil".into(),
            };
            assert_eq!(creds.mechanism(), "SCRAM-SHA-512");
            assert_eq!(creds.subject(), Some("user"));
            let got = exchange(&mut wire, &mut reader, &creds);
            if honest {
                got.unwrap();
            } else {
                let e = got.unwrap_err();
                assert!(e.to_string().contains("does not verify"), "{e}");
            }
            broker.join().unwrap();
        }
    }

    /// XOAUTH2: the initial response is `user=<u>^Aauth=Bearer <token>^A^A`;
    /// a broker that accepts the token ends with outcome ok, one that does
    /// not sends its error as a challenge, gets an empty response, and the
    /// refusal names the error.
    #[test]
    fn amqpbridge_7_2_xoauth2_sends_the_bearer_token_and_reads_the_error() {
        let creds = SaslCredentials::XOAuth2 {
            username: "svc@example.com".into(),
            token: "ya29.token".into(),
        };
        let (mut wire, mut reader, broker) = scripted(SASL_HEADER, &["XOAUTH2"], |w, _, _, _| {
            outcome(w, 0, None);
        });
        exchange(&mut wire, &mut reader, &creds).unwrap();
        assert_eq!(
            broker.join().unwrap(),
            Some((
                "XOAUTH2".to_string(),
                b"user=svc@example.com\x01auth=Bearer ya29.token\x01\x01".to_vec()
            ))
        );
        let (mut wire, mut reader, broker) = scripted(SASL_HEADER, &["XOAUTH2"], |w, r, _, _| {
            send(
                w,
                descriptor::SASL_CHALLENGE,
                vec![AmqpExtValue::Binary(
                    br#"{"status":"401","schemes":"bearer"}"#.to_vec(),
                )],
            );
            let resp = r.next().unwrap();
            let (d, v, _) = decode_performative(&resp.body).unwrap();
            assert_eq!(d, descriptor::SASL_RESPONSE);
            assert_eq!(
                field(&v, 0),
                Some(&AmqpExtValue::Binary(Vec::new())),
                "empty response"
            );
            outcome(w, 1, None);
        });
        let e = exchange(&mut wire, &mut reader, &creds).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
        assert!(e.to_string().contains("outcome code 1"), "{e}");
        assert!(e.to_string().contains("\"status\":\"401\""), "{e}");
        broker.join().unwrap();
    }
}
