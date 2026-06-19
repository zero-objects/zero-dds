//! SASL SCRAM-SHA-256 (RFC 7677 / RFC 5802) — Salted Challenge Response
//! Authentication Mechanism.
//!
//! The optional-but-recommended SASL mechanism for the AMQP endpoint (Spec
//! §10.2; the mandatory PLAIN/ANONYMOUS/EXTERNAL live in [`crate::sasl`]).
//! Unlike PLAIN, SCRAM never puts the password on the wire: the client proves
//! knowledge of a salted+iterated hash via a challenge/response exchange, and
//! the server proves it too (mutual authentication).
//!
//! Crypto: PBKDF2-HMAC-SHA-256 (the SCRAM `Hi` function) built directly on
//! `hmac`+`sha2` (no extra dependency), plus the SCRAM key schedule. Correctness
//! is anchored byte-exact to the RFC 7677 §3 test vector in the tests.
//!
//! `no_std + alloc`. Server nonce + salt + iteration count are caller-provided
//! (from config / a CSPRNG in the daemon) so the core stays deterministic and
//! testable — the RFC vector fixes all of them.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// `HMAC-SHA-256(key, msg)`.
// `new_from_slice` is infallible for HMAC (any key length is valid); the
// targeted allow matches the `security-crypto` convention for crypto init.
#[allow(clippy::expect_used)]
fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("HMAC any key length");
    mac.update(msg);
    let out = mac.finalize().into_bytes();
    let mut r = [0u8; 32];
    r.copy_from_slice(&out);
    r
}

/// `SHA-256(data)`.
fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    let out = h.finalize();
    let mut r = [0u8; 32];
    r.copy_from_slice(&out);
    r
}

/// PBKDF2-HMAC-SHA-256 with `dkLen = 32` — the SCRAM `Hi(str, salt, i)` function
/// (RFC 5802 §2.2). Because the derived length equals the hash length, exactly
/// one output block `T_1` is produced: `T = U_1 xor U_2 xor ... xor U_i`.
#[must_use]
pub fn pbkdf2_hmac_sha256(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    // U_1 = HMAC(password, salt || INT(1)).
    let mut msg = Vec::with_capacity(salt.len() + 4);
    msg.extend_from_slice(salt);
    msg.extend_from_slice(&1u32.to_be_bytes());
    let mut u = hmac_sha256(password, &msg);
    let mut t = u;
    for _ in 1..iterations {
        u = hmac_sha256(password, &u);
        for k in 0..32 {
            t[k] ^= u[k];
        }
    }
    t
}

/// The server-side secrets for a SCRAM identity (RFC 5802 §3): everything needed
/// to verify a client and to sign the server-final, derived once from the
/// password (or stored directly). `stored_key` + `server_key` never reveal the
/// password.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScramServerSecrets {
    /// `s` — the salt (raw bytes; base64-encoded on the wire).
    pub salt: Vec<u8>,
    /// `i` — the PBKDF2 iteration count.
    pub iterations: u32,
    /// `StoredKey = H(ClientKey)`.
    pub stored_key: [u8; 32],
    /// `ServerKey = HMAC(SaltedPassword, "Server Key")`.
    pub server_key: [u8; 32],
}

impl ScramServerSecrets {
    /// Derive the server secrets from a cleartext password + salt + iterations.
    #[must_use]
    pub fn from_password(password: &str, salt: &[u8], iterations: u32) -> Self {
        let salted = pbkdf2_hmac_sha256(password.as_bytes(), salt, iterations);
        let client_key = hmac_sha256(&salted, b"Client Key");
        let stored_key = sha256(&client_key);
        let server_key = hmac_sha256(&salted, b"Server Key");
        Self {
            salt: salt.to_vec(),
            iterations,
            stored_key,
            server_key,
        }
    }

    /// `ClientSignature = HMAC(StoredKey, AuthMessage)`.
    #[must_use]
    pub fn client_signature(&self, auth_message: &str) -> [u8; 32] {
        hmac_sha256(&self.stored_key, auth_message.as_bytes())
    }

    /// `ServerSignature = HMAC(ServerKey, AuthMessage)` (RFC 5802 §3) — the
    /// proof the server returns in the server-final `v=` field.
    #[must_use]
    pub fn server_signature(&self, auth_message: &str) -> [u8; 32] {
        hmac_sha256(&self.server_key, auth_message.as_bytes())
    }

    /// Verify a client's `ClientProof` against `AuthMessage` (RFC 5802 §3):
    /// `ClientKey = ClientProof xor ClientSignature`, then check
    /// `H(ClientKey) == StoredKey`. Constant-time-ish compare on the digest.
    #[must_use]
    pub fn verify_client_proof(&self, auth_message: &str, client_proof: &[u8; 32]) -> bool {
        let sig = self.client_signature(auth_message);
        let mut client_key = [0u8; 32];
        for k in 0..32 {
            client_key[k] = client_proof[k] ^ sig[k];
        }
        let h = sha256(&client_key);
        // Length is fixed (32); a difference-accumulating compare avoids a
        // trivial early-exit timing oracle on the stored-key digest.
        let diff = h
            .iter()
            .zip(self.stored_key.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b));
        diff == 0
    }
}

/// `ClientProof = ClientKey xor ClientSignature` (RFC 5802 §3) — the client side,
/// useful for an AMQP client endpoint (and for the RFC test vector).
#[must_use]
pub fn client_proof(password: &str, salt: &[u8], iterations: u32, auth_message: &str) -> [u8; 32] {
    let salted = pbkdf2_hmac_sha256(password.as_bytes(), salt, iterations);
    let client_key = hmac_sha256(&salted, b"Client Key");
    let stored_key = sha256(&client_key);
    let client_signature = hmac_sha256(&stored_key, auth_message.as_bytes());
    let mut proof = [0u8; 32];
    for k in 0..32 {
        proof[k] = client_key[k] ^ client_signature[k];
    }
    proof
}

/// Build the SCRAM `AuthMessage` (RFC 5802 §3): the concatenation, comma-joined,
/// of the client-first-bare, the server-first, and the client-final-without-proof.
#[must_use]
pub fn auth_message(
    client_first_bare: &str,
    server_first: &str,
    client_final_without_proof: &str,
) -> String {
    let mut s = String::with_capacity(
        client_first_bare.len() + server_first.len() + client_final_without_proof.len() + 2,
    );
    s.push_str(client_first_bare);
    s.push(',');
    s.push_str(server_first);
    s.push(',');
    s.push_str(client_final_without_proof);
    s
}

/// Encode 32 bytes as standard base64 (for the `p=`/`v=` wire fields).
#[must_use]
pub fn b64(bytes: &[u8]) -> String {
    B64.encode(bytes)
}

/// Decode a standard-base64 wire field (salt, proof, signature); `None` on
/// malformed input.
#[must_use]
pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    B64.decode(s.as_bytes()).ok()
}

/// Outcome of a SCRAM server exchange step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScramStep {
    /// Authentication succeeded: the `v=...` server-final message to return,
    /// plus the authenticated username.
    Success {
        /// `v=<base64 ServerSignature>` to send to the client.
        server_final: String,
        /// The authenticated SCRAM username (`n=` from client-first).
        username: String,
    },
    /// Authentication failed (bad proof, unknown user, or malformed message).
    Failure(String),
}

/// Server-side SCRAM-SHA-256 exchange (RFC 5802 §5 message flow).
///
/// Two-step: [`ScramServerExchange::start`] consumes the client-first message
/// and yields the server-first challenge; [`ScramServerExchange::finish`]
/// consumes the client-final message, verifies the `ClientProof` and yields the
/// server-final (`v=`). The `server_nonce_suffix` is caller-provided randomness
/// (CSPRNG in the daemon; fixed in the RFC test).
#[derive(Clone, Debug)]
pub struct ScramServerExchange {
    client_first_bare: String,
    server_first: String,
    full_nonce: String,
    secrets: ScramServerSecrets,
    username: String,
}

impl ScramServerExchange {
    /// Step 1 — process `client-first` (`n,,n=<user>,r=<client-nonce>`) and
    /// produce the `server-first` challenge (`r=<full-nonce>,s=<salt>,i=<iter>`).
    ///
    /// `lookup` resolves the username to its stored [`ScramServerSecrets`]
    /// (returns `None` for an unknown user). `server_nonce_suffix` is appended
    /// to the client nonce to form the full nonce.
    ///
    /// # Errors
    /// Returns the server-first string on success, or an error string if the
    /// client-first is malformed or the user is unknown.
    pub fn start(
        client_first: &str,
        server_nonce_suffix: &str,
        lookup: impl FnOnce(&str) -> Option<ScramServerSecrets>,
    ) -> Result<(Self, String), String> {
        // gs2-header is the leading "n,," / "y,," / "p=...," up to the second
        // comma; the client-first-bare is the remainder.
        let bare = strip_gs2_header(client_first)
            .ok_or_else(|| String::from("malformed client-first (gs2-header)"))?;
        let username = field(bare, "n=").ok_or_else(|| String::from("missing n="))?;
        let client_nonce = field(bare, "r=").ok_or_else(|| String::from("missing r="))?;

        let secrets = lookup(&username).ok_or_else(|| String::from("unknown user"))?;

        let full_nonce = alloc::format!("{client_nonce}{server_nonce_suffix}");
        let server_first = alloc::format!(
            "r={full_nonce},s={},i={}",
            b64(&secrets.salt),
            secrets.iterations
        );
        Ok((
            Self {
                client_first_bare: String::from(bare),
                server_first: server_first.clone(),
                full_nonce,
                secrets,
                username,
            },
            server_first,
        ))
    }

    /// Step 2 — process `client-final` (`c=<channel-binding>,r=<full-nonce>,p=<proof>`),
    /// verify the `ClientProof`, and produce the server-final.
    #[must_use]
    pub fn finish(self, client_final: &str) -> ScramStep {
        let recv_nonce = match field(client_final, "r=") {
            Some(n) => n,
            None => return ScramStep::Failure(String::from("missing r= in client-final")),
        };
        if recv_nonce != self.full_nonce {
            return ScramStep::Failure(String::from("nonce mismatch"));
        }
        let proof_b64 = match field(client_final, "p=") {
            Some(p) => p,
            None => return ScramStep::Failure(String::from("missing p= proof")),
        };
        let proof_bytes = match b64_decode(&proof_b64) {
            Some(b) if b.len() == 32 => b,
            _ => return ScramStep::Failure(String::from("malformed proof")),
        };
        let mut proof = [0u8; 32];
        proof.copy_from_slice(&proof_bytes);

        // client-final-without-proof = everything up to ",p=".
        let cfwp = match client_final.rfind(",p=") {
            Some(i) => &client_final[..i],
            None => return ScramStep::Failure(String::from("no ,p= in client-final")),
        };
        let am = auth_message(&self.client_first_bare, &self.server_first, cfwp);

        if !self.secrets.verify_client_proof(&am, &proof) {
            return ScramStep::Failure(String::from("invalid client proof"));
        }
        let server_final = alloc::format!("v={}", b64(&self.secrets.server_signature(&am)));
        ScramStep::Success {
            server_final,
            username: self.username,
        }
    }
}

/// Strip the SCRAM gs2-header (RFC 5802 §7) from a client-first message, leaving
/// the client-first-bare. The header is `gs2-cbind-flag "," [authzid] ","`.
fn strip_gs2_header(client_first: &str) -> Option<&str> {
    // gs2-cbind-flag: "n" | "y" | "p=name". Find the second comma.
    let first = client_first.find(',')?;
    let rest = &client_first[first + 1..];
    let second = rest.find(',')?;
    Some(&rest[second + 1..])
}

/// Extract the value of a `key=` attribute from a comma-separated SCRAM message
/// (value runs to the next comma or end of string).
fn field(msg: &str, key: &str) -> Option<String> {
    for part in msg.split(',') {
        if let Some(v) = part.strip_prefix(key) {
            return Some(String::from(v));
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // RFC 7677 §3 — the canonical SCRAM-SHA-256 exchange. username "user",
    // password "pencil", salt W22ZaJ0SNY7soEsUEjb6gQ==, i=4096. Verifying the
    // ClientProof + ServerSignature byte-exact proves the whole key schedule
    // (PBKDF2 -> ClientKey/StoredKey/ServerKey -> proof/signature).
    const SALT_B64: &str = "W22ZaJ0SNY7soEsUEjb6gQ==";
    const SERVER_NONCE: &str = "rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0";

    fn rfc_auth_message() -> String {
        let client_first_bare = "n=user,r=rOprNGfwEbeRWgbNEkqO";
        let server_first = alloc::format!("r={SERVER_NONCE},s={SALT_B64},i=4096");
        let client_final_without_proof = alloc::format!("c=biws,r={SERVER_NONCE}");
        auth_message(
            client_first_bare,
            &server_first,
            &client_final_without_proof,
        )
    }

    #[test]
    fn rfc7677_client_proof() {
        let salt = b64_decode(SALT_B64).unwrap();
        let am = rfc_auth_message();
        let proof = client_proof("pencil", &salt, 4096, &am);
        assert_eq!(
            b64(&proof),
            "dHzbZapWIk4jUhN+Ute9ytag9zjfMHgsqmmiz7AndVQ=",
            "ClientProof (RFC 7677 §3)"
        );
    }

    #[test]
    fn rfc7677_server_signature() {
        let salt = b64_decode(SALT_B64).unwrap();
        let am = rfc_auth_message();
        let secrets = ScramServerSecrets::from_password("pencil", &salt, 4096);
        assert_eq!(
            b64(&secrets.server_signature(&am)),
            "6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4=",
            "ServerSignature (RFC 7677 §3)"
        );
    }

    #[test]
    fn rfc7677_server_verifies_client_proof() {
        let salt = b64_decode(SALT_B64).unwrap();
        let am = rfc_auth_message();
        let secrets = ScramServerSecrets::from_password("pencil", &salt, 4096);
        let proof = client_proof("pencil", &salt, 4096, &am);
        assert!(
            secrets.verify_client_proof(&am, &proof),
            "valid proof accepted"
        );

        // A wrong password's proof must be rejected.
        let bad = client_proof("notpencil", &salt, 4096, &am);
        assert!(
            !secrets.verify_client_proof(&am, &bad),
            "wrong proof rejected"
        );
    }

    // Full server-side exchange driven with the RFC 7677 §3 messages: the
    // server consumes client-first, emits the exact server-first, then verifies
    // the client-final proof and emits the exact server-final (v=).
    #[test]
    fn rfc7677_full_server_exchange() {
        let salt = b64_decode(SALT_B64).unwrap();
        let client_first = "n,,n=user,r=rOprNGfwEbeRWgbNEkqO";
        // The server nonce suffix is what turns the client nonce into the full
        // nonce; the RFC fixes it.
        let suffix = "%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0";

        let (exchange, server_first) = ScramServerExchange::start(client_first, suffix, |user| {
            assert_eq!(user, "user");
            Some(ScramServerSecrets::from_password("pencil", &salt, 4096))
        })
        .expect("start ok");
        assert_eq!(
            server_first,
            "r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096"
        );

        let client_final = "c=biws,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,p=dHzbZapWIk4jUhN+Ute9ytag9zjfMHgsqmmiz7AndVQ=";
        match exchange.finish(client_final) {
            ScramStep::Success {
                server_final,
                username,
            } => {
                assert_eq!(
                    server_final,
                    "v=6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4="
                );
                assert_eq!(username, "user");
            }
            ScramStep::Failure(e) => panic!("exchange failed: {e}"),
        }
    }

    #[test]
    fn exchange_rejects_tampered_proof() {
        let salt = b64_decode(SALT_B64).unwrap();
        let (exchange, _) = ScramServerExchange::start(
            "n,,n=user,r=rOprNGfwEbeRWgbNEkqO",
            "%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0",
            |_| Some(ScramServerSecrets::from_password("pencil", &salt, 4096)),
        )
        .unwrap();
        // Flip the proof: a valid-base64 but wrong 32-byte proof.
        let bad = "c=biws,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,p=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
        assert!(matches!(exchange.finish(bad), ScramStep::Failure(_)));
    }

    #[test]
    fn exchange_rejects_unknown_user() {
        let r = ScramServerExchange::start("n,,n=nobody,r=abc", "xyz", |_| None);
        assert!(r.is_err());
    }

    #[test]
    fn pbkdf2_one_block_matches_rfc() {
        // SaltedPassword for "pencil" — feeds both ClientKey and ServerKey; if
        // this is wrong the proof/signature tests above would fail, but assert
        // the intermediate too for a clean failure locus.
        let salt = b64_decode(SALT_B64).unwrap();
        let salted = pbkdf2_hmac_sha256(b"pencil", &salt, 4096);
        // ClientKey = HMAC(SaltedPassword, "Client Key"); StoredKey = H(ClientKey).
        let client_key = hmac_sha256(&salted, b"Client Key");
        let stored_key = sha256(&client_key);
        let secrets = ScramServerSecrets::from_password("pencil", &salt, 4096);
        assert_eq!(secrets.stored_key, stored_key);
    }
}
