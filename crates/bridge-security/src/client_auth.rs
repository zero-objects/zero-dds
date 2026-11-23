// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The client side of the authentication a bridge presents to a broker:
//! the SASL mechanisms SCRAM-SHA-256 (RFC 5802, RFC 7677) and OAUTHBEARER
//! (RFC 7628), and the `sub` of a JWT the bridge holds.
//!
//! The crypto runs on the crate's backend (`ring` or `aws-lc-rs`, see the
//! crate root): PBKDF2-HMAC-SHA-256 as the SCRAM `Hi` function, HMAC-SHA-256,
//! SHA-256, the system random generator for the client nonce. User name and
//! password are prepared with SASLprep (RFC 4013) before use (RFC 5802 §5.1).

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;

use crate::backend::{digest, hmac, pbkdf2};

/// The SASL mechanism name of SCRAM-SHA-256 (RFC 7677).
pub const SCRAM_SHA_256: &str = "SCRAM-SHA-256";

/// The SASL mechanism name of OAUTHBEARER (RFC 7628).
pub const OAUTHBEARER: &str = "OAUTHBEARER";

/// The smallest iteration count a server may ask for (RFC 7677 §4: "at
/// least 4096").
pub const SCRAM_MIN_ITERATIONS: u32 = 4096;

/// The largest iteration count the client computes: a bound on the work a
/// server can make it do.
pub const SCRAM_MAX_ITERATIONS: u32 = 10_000_000;

/// Octets of randomness in a client nonce (base64: 32 characters).
const NONCE_OCTETS: usize = 24;

/// What fails in a SCRAM exchange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScramError {
    /// The user name or password is no SASLprep string (RFC 4013 §2).
    Prep(String),
    /// A message of the server does not have the syntax of RFC 5802 §7.
    Malformed(String),
    /// The server's nonce does not begin with the client's (RFC 5802 §5.1).
    NonceMismatch,
    /// The iteration count is outside [`SCRAM_MIN_ITERATIONS`] ..=
    /// [`SCRAM_MAX_ITERATIONS`].
    IterationCount(u32),
    /// The server-final-message carries `e=` (RFC 5802 §7).
    ServerError(String),
    /// The server's signature is not the one the password gives: the server
    /// does not know the password (RFC 5802 §3).
    BadServerSignature,
    /// A step out of order: the client-final before the server-first, the
    /// verification before the client-final.
    OutOfOrder,
    /// The random generator failed.
    Random,
}

impl core::fmt::Display for ScramError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Prep(m) => write!(f, "SASLprep: {m}"),
            Self::Malformed(m) => write!(f, "malformed SCRAM message: {m}"),
            Self::NonceMismatch => f.write_str("the server nonce does not extend the client nonce"),
            Self::IterationCount(i) => write!(
                f,
                "iteration count {i} outside {SCRAM_MIN_ITERATIONS}..={SCRAM_MAX_ITERATIONS}"
            ),
            Self::ServerError(e) => write!(f, "server error `{e}`"),
            Self::BadServerSignature => f.write_str("the server signature does not verify"),
            Self::OutOfOrder => f.write_str("SCRAM step out of order"),
            Self::Random => f.write_str("the random generator failed"),
        }
    }
}

impl std::error::Error for ScramError {}

/// SASLprep (RFC 4013) of a user name or password.
///
/// # Errors
/// [`ScramError::Prep`] for a string with a prohibited or unassigned code
/// point, or one that breaks the bidirectional rules.
pub fn saslprep(s: &str) -> Result<String, ScramError> {
    stringprep::saslprep(s)
        .map(std::borrow::Cow::into_owned)
        .map_err(|e| ScramError::Prep(e.to_string()))
}

/// The `saslname` of RFC 5802 §7: `=` as `=3D`, `,` as `=2C`.
fn saslname(user: &str) -> String {
    user.replace('=', "=3D").replace(',', "=2C")
}

/// Where a [`ScramClient`] stands.
#[derive(Debug, Clone)]
enum ScramState {
    /// The client-first-message went out.
    First,
    /// The client-final-message went out; the server signature it expects.
    Final { server_signature: Vec<u8> },
    /// The server signature verified.
    Done,
}

/// The client of one SCRAM-SHA-256 exchange (RFC 5802 §3, §5; RFC 7677):
/// [`Self::client_first`], [`Self::client_final`] to the
/// server-first-message, [`Self::verify_server_final`] of the
/// server-final-message. No channel binding (GS2 header `n,,`), no
/// authorization identity.
#[derive(Debug, Clone)]
pub struct ScramClient {
    password: String,
    nonce: String,
    client_first_bare: String,
    state: ScramState,
}

impl ScramClient {
    /// A new exchange for `user` with `password`, with a client nonce of
    /// 24 random octets.
    ///
    /// # Errors
    /// SASLprep refuses `user` or `password`, or the random generator
    /// fails.
    pub fn new(user: &str, password: &str) -> Result<Self, ScramError> {
        use crate::backend::rand::SecureRandom as _;
        let mut raw = [0u8; NONCE_OCTETS];
        crate::backend::rand::SystemRandom::new()
            .fill(&mut raw)
            .map_err(|_| ScramError::Random)?;
        Self::with_nonce(user, password, &B64.encode(raw))
    }

    /// An exchange with the client nonce `nonce` (printable ASCII without
    /// `,`), for the test vectors of the RFCs.
    ///
    /// # Errors
    /// SASLprep refuses `user` or `password`; `nonce` holds a character a
    /// nonce cannot carry.
    pub fn with_nonce(user: &str, password: &str, nonce: &str) -> Result<Self, ScramError> {
        if nonce.is_empty()
            || !nonce
                .bytes()
                .all(|b| (0x21..=0x7e).contains(&b) && b != b',')
        {
            return Err(ScramError::Malformed(format!("client nonce `{nonce}`")));
        }
        let user = saslprep(user)?;
        let password = saslprep(password)?;
        Ok(Self {
            password,
            nonce: nonce.to_string(),
            client_first_bare: format!("n={},r={nonce}", saslname(&user)),
            state: ScramState::First,
        })
    }

    /// The client-first-message: `n,,n=<user>,r=<nonce>`.
    #[must_use]
    pub fn client_first(&self) -> String {
        format!("n,,{}", self.client_first_bare)
    }

    /// The client-final-message `c=biws,r=<nonce>,p=<proof>` for the
    /// server-first-message `r=<nonce>,s=<salt>,i=<count>`.
    ///
    /// # Errors
    /// A server-first-message of another syntax, with a mandatory extension
    /// (`m=`), a nonce that does not extend the client's, or an iteration
    /// count out of bounds.
    pub fn client_final(&mut self, server_first: &str) -> Result<String, ScramError> {
        if !matches!(self.state, ScramState::First) {
            return Err(ScramError::OutOfOrder);
        }
        if server_first.starts_with("m=") {
            return Err(ScramError::Malformed(
                "a mandatory extension (m=) the client does not know".into(),
            ));
        }
        let mut fields = server_first.splitn(3, ',');
        let nonce = fields
            .next()
            .and_then(|f| f.strip_prefix("r="))
            .ok_or_else(|| ScramError::Malformed(format!("no r= in `{server_first}`")))?;
        let salt = fields
            .next()
            .and_then(|f| f.strip_prefix("s="))
            .ok_or_else(|| ScramError::Malformed(format!("no s= in `{server_first}`")))?;
        let rest = fields
            .next()
            .ok_or_else(|| ScramError::Malformed(format!("no i= in `{server_first}`")))?;
        let count = rest.split(',').next().unwrap_or("");
        let iterations: u32 = count
            .strip_prefix("i=")
            .and_then(|i| i.parse().ok())
            .ok_or_else(|| ScramError::Malformed(format!("i= in `{server_first}`")))?;
        if !nonce.starts_with(&self.nonce) || nonce.len() == self.nonce.len() {
            return Err(ScramError::NonceMismatch);
        }
        if !(SCRAM_MIN_ITERATIONS..=SCRAM_MAX_ITERATIONS).contains(&iterations) {
            return Err(ScramError::IterationCount(iterations));
        }
        let salt = B64
            .decode(salt)
            .map_err(|e| ScramError::Malformed(format!("salt: {e}")))?;
        let Some(rounds) = core::num::NonZeroU32::new(iterations) else {
            return Err(ScramError::IterationCount(iterations));
        };
        let mut salted = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            rounds,
            &salt,
            self.password.as_bytes(),
            &mut salted,
        );
        let salted_key = hmac::Key::new(hmac::HMAC_SHA256, &salted);
        let client_key = hmac::sign(&salted_key, b"Client Key");
        let stored_key = digest::digest(&digest::SHA256, client_key.as_ref());
        let without_proof = format!("c=biws,r={nonce}");
        let auth_message = format!("{},{server_first},{without_proof}", self.client_first_bare);
        let signature = hmac::sign(
            &hmac::Key::new(hmac::HMAC_SHA256, stored_key.as_ref()),
            auth_message.as_bytes(),
        );
        let proof: Vec<u8> = client_key
            .as_ref()
            .iter()
            .zip(signature.as_ref())
            .map(|(k, s)| k ^ s)
            .collect();
        let server_key = hmac::sign(&salted_key, b"Server Key");
        let server_signature = hmac::sign(
            &hmac::Key::new(hmac::HMAC_SHA256, server_key.as_ref()),
            auth_message.as_bytes(),
        );
        self.state = ScramState::Final {
            server_signature: server_signature.as_ref().to_vec(),
        };
        Ok(format!("{without_proof},p={}", B64.encode(proof)))
    }

    /// Verifies the server-final-message `v=<signature>`: the server knows
    /// the password (RFC 5802 §3).
    ///
    /// # Errors
    /// `e=` from the server, a signature that does not verify, another
    /// syntax, or no client-final-message before.
    pub fn verify_server_final(&mut self, server_final: &str) -> Result<(), ScramError> {
        let ScramState::Final { server_signature } = &self.state else {
            return Err(ScramError::OutOfOrder);
        };
        if let Some(e) = server_final.strip_prefix("e=") {
            return Err(ScramError::ServerError(e.to_string()));
        }
        let v = server_final
            .split(',')
            .next()
            .and_then(|f| f.strip_prefix("v="))
            .ok_or_else(|| ScramError::Malformed(format!("no v= in `{server_final}`")))?;
        let got = B64
            .decode(v)
            .map_err(|e| ScramError::Malformed(format!("v=: {e}")))?;
        // Compared without an early exit.
        let same = got.len() == server_signature.len()
            && got
                .iter()
                .zip(server_signature)
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0;
        if !same {
            return Err(ScramError::BadServerSignature);
        }
        self.state = ScramState::Done;
        Ok(())
    }

    /// Whether the server signature verified.
    #[must_use]
    pub fn is_done(&self) -> bool {
        matches!(self.state, ScramState::Done)
    }
}

/// The OAUTHBEARER initial client response of RFC 7628 §3.1: the GS2
/// header without authorization identity, then `host`, `port` (when
/// given) and `auth=Bearer <token>`, each ended by `%x01`, and a final
/// `%x01`.
#[must_use]
pub fn oauthbearer_initial_response(token: &str, host: Option<&str>, port: Option<u16>) -> Vec<u8> {
    let mut out = String::from("n,,\x01");
    if let Some(h) = host {
        out.push_str("host=");
        out.push_str(h);
        out.push('\x01');
    }
    if let Some(p) = port {
        out.push_str(&format!("port={p}\x01"));
    }
    out.push_str("auth=Bearer ");
    out.push_str(token);
    out.push_str("\x01\x01");
    out.into_bytes()
}

/// The answer to an OAUTHBEARER error challenge (RFC 7628 §3.2.3): a single
/// `%x01`, after which the server fails the exchange.
pub const OAUTHBEARER_ERROR_ACK: &[u8] = b"\x01";

/// The `sub` claim of the JWT `token` without checking its signature: the
/// bridge reads the subject of its own token (Spec §7.3 of the bridges).
/// `None` for a token that is no JWT or has no `sub`.
#[must_use]
pub fn jwt_subject_unverified(token: &str) -> Option<String> {
    let mut parts = token.split('.');
    let (_header, payload, _sig) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    let json = core::str::from_utf8(&raw).ok()?;
    crate::auth::json_field(json, "sub").filter(|s| !s.is_empty())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// RFC 7677 §3: the exchange of user `user` with password `pencil`,
    /// octet for octet.
    #[test]
    fn rfc7677_3_the_exchange_of_the_rfc() {
        let mut c = ScramClient::with_nonce("user", "pencil", "rOprNGfwEbeRWgbNEkqO").unwrap();
        assert_eq!(c.client_first(), "n,,n=user,r=rOprNGfwEbeRWgbNEkqO");
        let fin = c
            .client_final(
                "r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096",
            )
            .unwrap();
        assert_eq!(
            fin,
            "c=biws,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,p=dHzbZapWIk4jUhN+Ute9ytag9zjfMHgsqmmiz7AndVQ="
        );
        assert!(!c.is_done());
        c.verify_server_final("v=6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4=")
            .unwrap();
        assert!(c.is_done());
    }

    /// RFC 5802 §3: a server that does not know the password signs wrong;
    /// `e=` is the server's refusal.
    #[test]
    fn rfc5802_3_a_wrong_server_signature_and_an_error_fail() {
        let server_first = "r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096";
        let mut c = ScramClient::with_nonce("user", "pencil", "rOprNGfwEbeRWgbNEkqO").unwrap();
        c.client_final(server_first).unwrap();
        assert_eq!(
            c.verify_server_final("v=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="),
            Err(ScramError::BadServerSignature)
        );
        let mut c = ScramClient::with_nonce("user", "pencil", "rOprNGfwEbeRWgbNEkqO").unwrap();
        c.client_final(server_first).unwrap();
        assert_eq!(
            c.verify_server_final("e=invalid-proof"),
            Err(ScramError::ServerError("invalid-proof".into()))
        );
        // Another password gives another proof.
        let mut other = ScramClient::with_nonce("user", "pencil2", "rOprNGfwEbeRWgbNEkqO").unwrap();
        assert!(
            !other
                .client_final(server_first)
                .unwrap()
                .ends_with("AndVQ=")
        );
    }

    /// RFC 5802 §5.1, §7: the server nonce extends the client's; `m=` is
    /// refused; the iteration count is bounded (RFC 7677 §4).
    #[test]
    fn rfc5802_5_1_what_a_server_first_message_must_hold() {
        let fresh = || ScramClient::with_nonce("user", "pencil", "abc").unwrap();
        assert_eq!(
            fresh().client_final("r=xyz123,s=QSXCR+Q6sek8bf92,i=4096"),
            Err(ScramError::NonceMismatch)
        );
        assert_eq!(
            fresh().client_final("r=abc,s=QSXCR+Q6sek8bf92,i=4096"),
            Err(ScramError::NonceMismatch)
        );
        assert!(matches!(
            fresh().client_final("m=ext,r=abcdef,s=QSXCR+Q6sek8bf92,i=4096"),
            Err(ScramError::Malformed(_))
        ));
        assert_eq!(
            fresh().client_final("r=abcdef,s=QSXCR+Q6sek8bf92,i=4095"),
            Err(ScramError::IterationCount(4095))
        );
        assert!(matches!(
            fresh().client_final("r=abcdef,i=4096"),
            Err(ScramError::Malformed(_))
        ));
        assert_eq!(
            fresh().verify_server_final("v=AAAA"),
            Err(ScramError::OutOfOrder)
        );
    }

    /// RFC 5802 §5.1 / RFC 4013: the user name is prepared and `=` and `,`
    /// escaped; SASLprep maps a non-ASCII space and refuses a control
    /// character.
    #[test]
    fn rfc5802_5_1_the_user_name_is_prepared_and_escaped() {
        let c = ScramClient::with_nonce("a=b,c", "p", "n1").unwrap();
        assert_eq!(c.client_first(), "n,,n=a=3Db=2Cc,r=n1");
        let c = ScramClient::with_nonce("us\u{00A0}er", "p", "n1").unwrap();
        assert_eq!(c.client_first(), "n,,n=us er,r=n1");
        assert!(matches!(
            ScramClient::with_nonce("user", "pa\u{0007}ss", "n1"),
            Err(ScramError::Prep(_))
        ));
        let a = ScramClient::new("user", "pencil").unwrap();
        let b = ScramClient::new("user", "pencil").unwrap();
        assert_ne!(a.client_first(), b.client_first(), "a nonce per exchange");
        assert_eq!(a.client_first().len(), "n,,n=user,r=".len() + 32);
    }

    /// RFC 7628 §3.1 and the example of §4.1.
    #[test]
    fn rfc7628_3_1_the_initial_client_response() {
        assert_eq!(
            oauthbearer_initial_response(
                "vF9dft4qmTc2Nvb3RlckBhbHRhdmlzdGEuY29tCg==",
                Some("server.example.com"),
                Some(143)
            ),
            b"n,,\x01host=server.example.com\x01port=143\x01auth=Bearer vF9dft4qmTc2Nvb3RlckBhbHRhdmlzdGEuY29tCg==\x01\x01"
                .to_vec()
        );
        assert_eq!(
            oauthbearer_initial_response("t", None, None),
            b"n,,\x01auth=Bearer t\x01\x01".to_vec()
        );
    }

    #[test]
    fn the_sub_of_a_jwt_is_read_without_its_signature() {
        // {"alg":"RS256"} . {"sub":"svc-7","iss":"x"}
        assert_eq!(
            jwt_subject_unverified("eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJzdmMtNyIsImlzcyI6IngifQ.c2ln")
                .as_deref(),
            Some("svc-7")
        );
        assert_eq!(jwt_subject_unverified("not-a-jwt"), None);
        // {"alg":"none"} . {"iss":"x"}
        assert_eq!(
            jwt_subject_unverified("eyJhbGciOiJub25lIn0.eyJpc3MiOiJ4In0."),
            None
        );
    }
}
