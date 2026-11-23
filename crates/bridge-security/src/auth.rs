// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! §7.2 auth modes — `none|bearer|jwt|mtls|sasl`.
//!
//! Entry point: [`AuthMode::validate`] → [`AuthSubject`] or
//! [`AuthError`]. In each bridge, the call is hooked into the relevant
//! connection phase:
//!
//! * HTTP-based (ws/grpc): after reading the request headers.
//! * MQTT: during CONNECT packet evaluation.
//! * AMQP: after SASL-PLAIN negotiation.
//! * CORBA: from CSIv2 token parsing or the TLS layer (mtls).

use std::collections::HashMap;

/// Auth subject — passed to subsequent ACL checks (`§7.3`) after a
/// successful [`AuthMode::validate`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthSubject {
    /// Stable Identity (e.g. JWT `sub`, Bearer-Mapping, mTLS-CN, SASL-User).
    pub name: String,
    /// Group memberships (e.g. JWT `groups` claim).
    pub groups: Vec<String>,
    /// Free-form claims (any string value).
    pub claims: HashMap<String, String>,
    /// mTLS: `mtls:<SHA-256 of the client certificate, hex>`. An ACL entry
    /// may name it instead of the subject DN in [`Self::name`], to pin one
    /// certificate.
    pub fingerprint: Option<String>,
}

impl AuthSubject {
    /// Anonymous subject for `AuthMode::None`.
    #[must_use]
    pub fn anonymous() -> Self {
        Self::new("anonymous")
    }

    /// Plain constructor.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            groups: Vec::new(),
            claims: HashMap::new(),
            fingerprint: None,
        }
    }

    /// Fluent add group.
    #[must_use]
    pub fn with_group(mut self, g: impl Into<String>) -> Self {
        self.groups.push(g.into());
        self
    }

    /// Fluent add claim.
    #[must_use]
    pub fn with_claim(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.claims.insert(k.into(), v.into());
        self
    }
}

/// Error during auth validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// Header / frame is entirely missing (e.g. no `Authorization`).
    MissingCredentials,
    /// Header format invalid (e.g. `Authorization` without `Bearer `).
    MalformedCredentials(String),
    /// Token not accepted (bearer mismatch / JWT signature fail / ...).
    Rejected(String),
    /// Configuration broken (e.g. JWT public key cannot be loaded).
    Misconfigured(String),
}

impl core::fmt::Display for AuthError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingCredentials => f.write_str("missing credentials"),
            Self::MalformedCredentials(m) => write!(f, "malformed credentials: {m}"),
            Self::Rejected(m) => write!(f, "credentials rejected: {m}"),
            Self::Misconfigured(m) => write!(f, "auth misconfigured: {m}"),
        }
    }
}

impl std::error::Error for AuthError {}

/// A token that is verified (signature, time, issuer, audience) and lacks
/// the claim `auth.jwt.required_claim` asks for: its bearer is
/// authenticated and not authorised. Over HTTP that is 403 with
/// `insufficient_scope` (RFC 6750 §3.1), where every other refusal is 401.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingClaim {
    /// Who the token names.
    pub subject: AuthSubject,
    /// The claim that is required.
    pub claim: String,
    /// The value the claim has to carry.
    pub value: String,
}

impl core::fmt::Display for MissingClaim {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "jwt: claim {} lacks {}", self.claim, self.value)
    }
}

/// Why [`AuthMode::validate_scoped`] refuses credentials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The credentials do not authenticate anyone.
    Unauthenticated(AuthError),
    /// They authenticate their bearer, who lacks the required claim.
    MissingClaim(Box<MissingClaim>),
}

impl core::fmt::Display for Refusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unauthenticated(e) => e.fmt(f),
            Self::MissingClaim(m) => write!(f, "credentials rejected: {m}"),
        }
    }
}

impl std::error::Error for Refusal {}

impl From<Refusal> for AuthError {
    /// The refusal as [`AuthMode::validate`] reports it.
    fn from(refusal: Refusal) -> Self {
        match refusal {
            Refusal::Unauthenticated(e) => e,
            Refusal::MissingClaim(m) => Self::Rejected(m.to_string()),
        }
    }
}

/// Auth mode per daemon (CLI `--auth-mode <MODE>`).
#[derive(Debug, Clone)]
pub enum AuthMode {
    /// No auth. Subject = "anonymous".
    None,
    /// Bearer token comparison (HTTP header `Authorization: Bearer …`).
    /// Multi-token via map `token → subject`.
    Bearer {
        /// Mapping from the token string to the resulting subject.
        tokens: HashMap<String, AuthSubject>,
    },
    /// JWT — RS256 or ES256 signature validation (RFC 7518 §3.3, §3.4)
    /// against the public key; the `alg` of a token has to fit the key.
    Jwt {
        /// The public key the signature is verified with.
        key: JwtKey,
        /// Optional issuer match (`iss` claim).
        expected_issuer: Option<String>,
        /// Optional audience match (`aud` claim, a string or an array
        /// that has to contain it; RFC 7519 §4.1.3).
        expected_audience: Option<String>,
        /// Optional `(claim, value)` the token has to carry. The claim
        /// matches when it equals the value or, as a space-separated list
        /// such as OAuth `scope` (RFC 8693 §4.2), contains it.
        required_claim: Option<(String, String)>,
    },
    /// mTLS — identity is supplied by the TLS layer (client cert), not
    /// by this function. [`AuthMode::validate`] with `Mtls` expects
    /// `presented_subject = Some(...)`.
    Mtls,
    /// SASL-PLAIN — `username\0username\0password` frame, with map
    /// `username → password`.
    SaslPlain {
        /// Mapping `user → password`.
        users: HashMap<String, String>,
    },
}

/// The public key of [`AuthMode::Jwt`]; its kind names the one `alg` a
/// token may carry (RFC 7518 §3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JwtKey {
    /// RS256 (RSASSA-PKCS1-v1_5 with SHA-256, RFC 7518 §3.3): the PKCS#1
    /// `RSAPublicKey` DER (RFC 8017 §A.1.1).
    Rs256(Vec<u8>),
    /// ES256 (ECDSA P-256 with SHA-256, RFC 7518 §3.4): the uncompressed
    /// point `04 || X || Y` (SEC 1 §2.3.3), 65 octets.
    Es256(Vec<u8>),
}

impl JwtKey {
    /// The `alg` value of a token this key verifies.
    #[must_use]
    pub fn alg(&self) -> &'static str {
        match self {
            Self::Rs256(_) => "RS256",
            Self::Es256(_) => "ES256",
        }
    }
}

/// What the caller (daemon) supplies for `validate`.
#[derive(Debug, Clone, Default)]
pub struct AuthInput<'a> {
    /// Bearer/JWT: content of the `Authorization` header (complete).
    pub authorization_header: Option<&'a str>,
    /// SASL-PLAIN: raw frame bytes (`user\0user\0pass`).
    pub sasl_plain_blob: Option<&'a [u8]>,
    /// mTLS: subject already validated by the rustls connection (e.g.
    /// cert CN). If mTLS but no cert was presented: `None`.
    pub mtls_subject: Option<AuthSubject>,
}

impl AuthMode {
    /// Validate. Returns the subject or an [`AuthError`].
    ///
    /// # Errors
    /// [`AuthError`] with sub-variant.
    pub fn validate(&self, input: &AuthInput<'_>) -> Result<AuthSubject, AuthError> {
        self.validate_scoped(input).map_err(AuthError::from)
    }

    /// [`Self::validate`], telling a verified token without the required
    /// claim apart from credentials that authenticate no one.
    ///
    /// # Errors
    /// [`Refusal::MissingClaim`] for a JWT whose signature, time, issuer
    /// and audience hold and whose required claim is absent or carries
    /// another value; [`Refusal::Unauthenticated`] for everything else
    /// `validate` refuses.
    pub fn validate_scoped(&self, input: &AuthInput<'_>) -> Result<AuthSubject, Refusal> {
        let unauthenticated = Refusal::Unauthenticated;
        match self {
            Self::None => Ok(AuthSubject::anonymous()),
            Self::Bearer { tokens } => {
                let hdr = input
                    .authorization_header
                    .ok_or(unauthenticated(AuthError::MissingCredentials))?;
                let token = strip_bearer(hdr).map_err(unauthenticated)?;
                tokens.get(token).cloned().ok_or_else(|| {
                    unauthenticated(AuthError::Rejected("unknown bearer token".to_string()))
                })
            }
            Self::Jwt {
                key,
                expected_issuer,
                expected_audience,
                required_claim,
            } => {
                let hdr = input
                    .authorization_header
                    .ok_or(unauthenticated(AuthError::MissingCredentials))?;
                let token = strip_bearer(hdr).map_err(unauthenticated)?;
                validate_jwt(
                    token,
                    key,
                    &JwtChecks {
                        issuer: expected_issuer.as_deref(),
                        audience: expected_audience.as_deref(),
                        required_claim: required_claim
                            .as_ref()
                            .map(|(k, v)| (k.as_str(), v.as_str())),
                        now: unix_now(),
                    },
                )
                .map_err(unauthenticated)?
                .map_err(|m| Refusal::MissingClaim(Box::new(m)))
            }
            Self::Mtls => input.mtls_subject.clone().ok_or_else(|| {
                unauthenticated(AuthError::Rejected("mTLS expected client cert".to_string()))
            }),
            Self::SaslPlain { users } => {
                let blob = input
                    .sasl_plain_blob
                    .ok_or(unauthenticated(AuthError::MissingCredentials))?;
                let (user, pass) = parse_sasl_plain(blob).map_err(unauthenticated)?;
                let stored = users.get(user).ok_or_else(|| {
                    unauthenticated(AuthError::Rejected("unknown user".to_string()))
                })?;
                if stored == pass {
                    Ok(AuthSubject::new(user))
                } else {
                    Err(unauthenticated(AuthError::Rejected(
                        "password mismatch".to_string(),
                    )))
                }
            }
        }
    }
}

fn strip_bearer(hdr: &str) -> Result<&str, AuthError> {
    let trimmed = hdr.trim();
    let prefix = "Bearer ";
    if trimmed.len() < prefix.len()
        || !trimmed
            .get(..prefix.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(prefix))
    {
        return Err(AuthError::MalformedCredentials(
            "expected `Bearer …`".to_string(),
        ));
    }
    Ok(trimmed[prefix.len()..].trim())
}

fn parse_sasl_plain(blob: &[u8]) -> Result<(&str, &str), AuthError> {
    // Format: `[authzid]\0authcid\0passwd` (RFC 4616).
    let mut parts = blob.splitn(3, |b| *b == 0);
    let _authzid = parts
        .next()
        .ok_or(AuthError::MalformedCredentials("sasl-plain empty".into()))?;
    let authcid = parts
        .next()
        .ok_or(AuthError::MalformedCredentials("sasl-plain no user".into()))?;
    let passwd = parts
        .next()
        .ok_or(AuthError::MalformedCredentials("sasl-plain no pass".into()))?;
    let user = core::str::from_utf8(authcid)
        .map_err(|_| AuthError::MalformedCredentials("sasl-plain user utf8".into()))?;
    let pass = core::str::from_utf8(passwd)
        .map_err(|_| AuthError::MalformedCredentials("sasl-plain pass utf8".into()))?;
    if user.is_empty() {
        return Err(AuthError::MalformedCredentials(
            "sasl-plain empty user".into(),
        ));
    }
    Ok((user, pass))
}

// ---------- JWT RS256 ----------------------------------------------------

/// Clock skew accepted on `exp` and `nbf`, in seconds. RFC 7519 §4.1.4
/// allows "some small leeway"; 60 s is the default of the `jsonwebtoken`
/// crate's `Validation`.
pub const JWT_CLOCK_LEEWAY_SECS: u64 = 60;

/// The claim checks after the signature.
struct JwtChecks<'a> {
    issuer: Option<&'a str>,
    audience: Option<&'a str>,
    required_claim: Option<(&'a str, &'a str)>,
    /// Seconds since the epoch.
    now: u64,
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A NumericDate claim (RFC 7519 §2): seconds, possibly fractional.
fn numeric_date(payload: &str, key: &str) -> Result<Option<u64>, AuthError> {
    let Some(raw) = json_field(payload, key) else {
        return Ok(None);
    };
    let whole = raw.split('.').next().unwrap_or("");
    whole
        .parse::<u64>()
        .map(Some)
        .map_err(|_| AuthError::MalformedCredentials(format!("jwt: {key} is not a NumericDate")))
}

/// The subject of a token that verifies; `Ok(Err(_))` for one that
/// verifies and lacks the required claim, the last of the checks.
fn validate_jwt(
    token: &str,
    key: &JwtKey,
    checks: &JwtChecks<'_>,
) -> Result<Result<AuthSubject, MissingClaim>, AuthError> {
    use base64::Engine as _;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;

    let mut parts = token.split('.');
    let h_b64 = parts
        .next()
        .ok_or_else(|| AuthError::MalformedCredentials("jwt: no header".into()))?;
    let p_b64 = parts
        .next()
        .ok_or_else(|| AuthError::MalformedCredentials("jwt: no payload".into()))?;
    let s_b64 = parts
        .next()
        .ok_or_else(|| AuthError::MalformedCredentials("jwt: no sig".into()))?;
    if parts.next().is_some() {
        return Err(AuthError::MalformedCredentials(
            "jwt: too many segments".into(),
        ));
    }

    let header_bytes = engine
        .decode(h_b64)
        .map_err(|e| AuthError::MalformedCredentials(format!("jwt header b64: {e}")))?;
    let payload_bytes = engine
        .decode(p_b64)
        .map_err(|e| AuthError::MalformedCredentials(format!("jwt payload b64: {e}")))?;
    let sig_bytes = engine
        .decode(s_b64)
        .map_err(|e| AuthError::MalformedCredentials(format!("jwt sig b64: {e}")))?;

    // RFC 7515 §4.1.1 / RFC 8725 §3.1: the header's `alg` has to be the
    // one algorithm of the configured key; `none`, HS256 and the other
    // kind of key are refused before any signature is looked at.
    let header_str = core::str::from_utf8(&header_bytes)
        .map_err(|_| AuthError::MalformedCredentials("jwt header utf8".into()))?;
    if !json_field_eq(header_str, "alg", key.alg()) {
        return Err(AuthError::Rejected(format!(
            "jwt: alg must be {} for the configured key",
            key.alg()
        )));
    }
    verify_jws_signature(key, h_b64, p_b64, &sig_bytes)?;

    // Extract payload claims.
    let payload_str = core::str::from_utf8(&payload_bytes)
        .map_err(|_| AuthError::MalformedCredentials("jwt payload utf8".into()))?;
    let sub = json_field(payload_str, "sub")
        .ok_or_else(|| AuthError::Rejected("jwt: no sub claim".into()))?;

    // RFC 7519 §4.1.4 / §4.1.5: a token past `exp` or before `nbf` is
    // not accepted. Both are optional; a present one is enforced.
    if let Some(exp) = numeric_date(payload_str, "exp")?
        && checks.now >= exp.saturating_add(JWT_CLOCK_LEEWAY_SECS)
    {
        return Err(AuthError::Rejected("jwt: expired".into()));
    }
    if let Some(nbf) = numeric_date(payload_str, "nbf")?
        && checks.now.saturating_add(JWT_CLOCK_LEEWAY_SECS) < nbf
    {
        return Err(AuthError::Rejected("jwt: not yet valid".into()));
    }

    if let Some(expected) = checks.issuer {
        let iss = json_field(payload_str, "iss")
            .ok_or_else(|| AuthError::Rejected("jwt: no iss claim".into()))?;
        if iss != expected {
            return Err(AuthError::Rejected(format!("jwt: iss != {expected}")));
        }
    }
    if let Some(expected) = checks.audience {
        let matches = match json_array(payload_str, "aud") {
            Some(list) => list.iter().any(|a| a == expected),
            None => json_field(payload_str, "aud").is_some_and(|a| a == expected),
        };
        if !matches {
            return Err(AuthError::Rejected(format!("jwt: aud lacks {expected}")));
        }
    }
    let mut subj = AuthSubject::new(sub);
    if let Some(groups_raw) = json_array(payload_str, "groups") {
        for g in groups_raw {
            subj.groups.push(g);
        }
    }
    if let Some((claim, value)) = checks.required_claim {
        let matches = match json_array(payload_str, claim) {
            Some(list) => list.iter().any(|v| v == value),
            None => json_field(payload_str, claim)
                .is_some_and(|v| v == value || v.split(' ').any(|p| p == value)),
        };
        if !matches {
            return Ok(Err(MissingClaim {
                subject: subj,
                claim: claim.to_string(),
                value: value.to_string(),
            }));
        }
    }
    Ok(Ok(subj))
}

/// Verifies the JWS signature over the signing input `<h_b64>.<p_b64>`
/// (RFC 7515 §5.2): RSASSA-PKCS1-v1_5 SHA-256 for RS256, ECDSA P-256
/// SHA-256 with the 64-octet `R || S` signature for ES256 (RFC 7518 §3.4).
fn verify_jws_signature(
    key: &JwtKey,
    h_b64: &str,
    p_b64: &str,
    sig: &[u8],
) -> Result<(), AuthError> {
    use crate::backend::signature;
    let mut signed = Vec::with_capacity(h_b64.len() + 1 + p_b64.len());
    signed.extend_from_slice(h_b64.as_bytes());
    signed.push(b'.');
    signed.extend_from_slice(p_b64.as_bytes());
    let verified = match key {
        JwtKey::Rs256(der) => {
            signature::UnparsedPublicKey::new(&signature::RSA_PKCS1_2048_8192_SHA256, der)
                .verify(&signed, sig)
        }
        JwtKey::Es256(point) => {
            // RFC 7518 §3.4: a signature of another length is no ES256
            // signature (an ASN.1 DER one included).
            if sig.len() != 64 {
                return Err(AuthError::Rejected(
                    "jwt: an ES256 signature is 64 octets".into(),
                ));
            }
            signature::UnparsedPublicKey::new(&signature::ECDSA_P256_SHA256_FIXED, point)
                .verify(&signed, sig)
        }
    };
    verified.map_err(|_| AuthError::Rejected("jwt: signature invalid".into()))
}

/// The key [`AuthMode::Jwt`] verifies with, out of a PEM file's text:
/// an RSA key (as [`jwt_public_key_from_pem`] reads it) is an RS256 key,
/// a `-----BEGIN PUBLIC KEY-----` block with the id-ecPublicKey algorithm
/// on the curve P-256 (RFC 5480 §2) an ES256 key.
///
/// # Errors
/// A message naming what the text lacks; an EC key on another curve and a
/// compressed point are refused.
pub fn jwt_key_from_pem(pem: &str) -> Result<JwtKey, String> {
    if let Some(point) = ec_p256_point_from_pem(pem)? {
        return Ok(JwtKey::Es256(point));
    }
    jwt_public_key_from_pem(pem).map(JwtKey::Rs256)
}

/// The P-256 point of an SPKI PEM with the id-ecPublicKey algorithm;
/// `Ok(None)` when the text holds no such key.
fn ec_p256_point_from_pem(pem: &str) -> Result<Option<Vec<u8>>, String> {
    use base64::Engine as _;
    const ID_EC_PUBLIC_KEY: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
    const PRIME256V1: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];
    let Some(b64) = pem_block(pem, "PUBLIC KEY") else {
        return Ok(None);
    };
    let spki = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("pem base64: {e}"))?;
    let Some((0x30, outer, _)) = der_tlv(&spki) else {
        return Ok(None);
    };
    let Some((0x30, alg, rest)) = der_tlv(outer) else {
        return Ok(None);
    };
    let Some((0x06, oid, params)) = der_tlv(alg) else {
        return Ok(None);
    };
    if oid != ID_EC_PUBLIC_KEY {
        return Ok(None);
    }
    match der_tlv(params) {
        Some((0x06, curve, _)) if curve == PRIME256V1 => {}
        _ => return Err("spki: an EC key for ES256 is on the curve P-256 (prime256v1)".into()),
    }
    match der_tlv(rest) {
        Some((0x03, bits, _)) => match bits.split_first() {
            Some((0, point)) if point.len() == 65 && point[0] == 0x04 => Ok(Some(point.to_vec())),
            _ => Err("spki: the EC point is not an uncompressed P-256 point".into()),
        },
        _ => Err("spki: no BIT STRING with the key".into()),
    }
}

/// The base64 text between the BEGIN and END lines of `label`.
fn pem_block(pem: &str, label: &str) -> Option<String> {
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let start = pem.find(&begin)? + begin.len();
    let stop = pem[start..].find(&end)? + start;
    Some(pem[start..stop].split_whitespace().collect())
}

/// The PKCS#1 `RSAPublicKey` DER that [`JwtKey::Rs256`] verifies with,
/// out of a PEM file's text.
///
/// Accepts `-----BEGIN RSA PUBLIC KEY-----` (PKCS#1, RFC 8017 §A.1.1) and
/// `-----BEGIN PUBLIC KEY-----` (X.509 `SubjectPublicKeyInfo`, RFC 5280
/// §4.1.2.7, as `openssl rsa -pubout` writes it) with the rsaEncryption
/// algorithm; the latter is unwrapped to the key inside its BIT STRING.
///
/// # Errors
/// A message naming what the text lacks.
pub fn jwt_public_key_from_pem(pem: &str) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    let block = pem_block;
    let decode = |b64: String| {
        base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| format!("pem base64: {e}"))
    };
    if let Some(b64) = block(pem, "RSA PUBLIC KEY") {
        return decode(b64);
    }
    let Some(b64) = block(pem, "PUBLIC KEY") else {
        return Err("no RSA PUBLIC KEY or PUBLIC KEY block".into());
    };
    let spki = decode(b64)?;
    // SEQUENCE { SEQUENCE { OID rsaEncryption, NULL }, BIT STRING }
    let (tag, outer, _) = der_tlv(&spki).ok_or("spki: bad outer TLV")?;
    if tag != 0x30 {
        return Err("spki: not a SEQUENCE".into());
    }
    let (tag, alg, rest) = der_tlv(outer).ok_or("spki: bad algorithm TLV")?;
    if tag != 0x30 {
        return Err("spki: no AlgorithmIdentifier".into());
    }
    let (tag, oid, _) = der_tlv(alg).ok_or("spki: bad OID TLV")?;
    const RSA_ENCRYPTION: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01];
    if tag != 0x06 || oid != RSA_ENCRYPTION {
        return Err("spki: algorithm is not rsaEncryption".into());
    }
    let (tag, bits, _) = der_tlv(rest).ok_or("spki: bad BIT STRING TLV")?;
    match (tag, bits.split_first()) {
        (0x03, Some((0, key))) => Ok(key.to_vec()),
        _ => Err("spki: no BIT STRING with the key".into()),
    }
}

/// One DER TLV: tag, contents, what follows.
fn der_tlv(input: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let (&tag, rest) = input.split_first()?;
    let (&first, rest) = rest.split_first()?;
    let (len, rest) = if first < 0x80 {
        (usize::from(first), rest)
    } else {
        let n = usize::from(first & 0x7f);
        if n == 0 || n > 4 || rest.len() < n {
            return None;
        }
        let len = rest[..n]
            .iter()
            .fold(0usize, |acc, b| (acc << 8) | usize::from(*b));
        (len, &rest[n..])
    };
    if rest.len() < len {
        return None;
    }
    Some((tag, &rest[..len], &rest[len..]))
}

/// Mini JSON field extractor for the JWT header and payload maps.
/// Accepts flat-structured JSON as JWT libraries emit it. Not a full
/// JSON parser implementation — Spec §7.2 says a JWT lib may use a
/// compact subset.
pub(crate) fn json_field(src: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let pos = src.find(&pat)?;
    let after = &src[pos + pat.len()..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();
    if let Some(stripped) = rest.strip_prefix('"') {
        let end = stripped.find('"')?;
        Some(stripped[..end].to_string())
    } else {
        // Numeric / bool — take up to the next , or }.
        let end = rest
            .find(|c: char| c == ',' || c == '}' || c.is_whitespace())
            .unwrap_or(rest.len());
        Some(rest[..end].to_string())
    }
}

fn json_field_eq(src: &str, key: &str, expected: &str) -> bool {
    json_field(src, key).is_some_and(|v| v == expected)
}

fn json_array(src: &str, key: &str) -> Option<Vec<String>> {
    let pat = format!("\"{key}\"");
    let pos = src.find(&pat)?;
    let after = &src[pos + pat.len()..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();
    let stripped = rest.strip_prefix('[')?;
    let end = stripped.find(']')?;
    let inside = &stripped[..end];
    let mut out = Vec::new();
    for piece in inside.split(',') {
        let p = piece.trim().trim_matches('"');
        if !p.is_empty() {
            out.push(p.to_string());
        }
    }
    Some(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn none_mode_yields_anonymous() {
        let m = AuthMode::None;
        let s = m.validate(&AuthInput::default()).unwrap();
        assert_eq!(s.name, "anonymous");
    }

    #[test]
    fn bearer_valid_token_accepted() {
        let mut tokens = HashMap::new();
        tokens.insert("secret123".to_string(), AuthSubject::new("alice"));
        let m = AuthMode::Bearer { tokens };
        let s = m
            .validate(&AuthInput {
                authorization_header: Some("Bearer secret123"),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(s.name, "alice");
    }

    #[test]
    fn bearer_invalid_token_rejected() {
        let m = AuthMode::Bearer {
            tokens: HashMap::new(),
        };
        let err = m
            .validate(&AuthInput {
                authorization_header: Some("Bearer wrong"),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(err, AuthError::Rejected(_)));
    }

    #[test]
    fn bearer_missing_header_returns_missing() {
        let m = AuthMode::Bearer {
            tokens: HashMap::new(),
        };
        let err = m.validate(&AuthInput::default()).unwrap_err();
        assert!(matches!(err, AuthError::MissingCredentials));
    }

    #[test]
    fn bearer_malformed_header_returns_malformed() {
        let m = AuthMode::Bearer {
            tokens: HashMap::new(),
        };
        let err = m
            .validate(&AuthInput {
                authorization_header: Some("Basic xx"),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(err, AuthError::MalformedCredentials(_)));
    }

    #[test]
    fn mtls_with_subject_accepted() {
        let m = AuthMode::Mtls;
        let s = m
            .validate(&AuthInput {
                mtls_subject: Some(AuthSubject::new("CN=alice")),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(s.name, "CN=alice");
    }

    #[test]
    fn mtls_without_subject_rejected() {
        let m = AuthMode::Mtls;
        let err = m.validate(&AuthInput::default()).unwrap_err();
        assert!(matches!(err, AuthError::Rejected(_)));
    }

    #[test]
    fn sasl_plain_valid_pair_accepted() {
        let mut users = HashMap::new();
        users.insert("alice".to_string(), "wonderland".to_string());
        let m = AuthMode::SaslPlain { users };
        let blob = b"\0alice\0wonderland";
        let s = m
            .validate(&AuthInput {
                sasl_plain_blob: Some(blob),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(s.name, "alice");
    }

    #[test]
    fn sasl_plain_wrong_password_rejected() {
        let mut users = HashMap::new();
        users.insert("alice".to_string(), "wonderland".to_string());
        let m = AuthMode::SaslPlain { users };
        let blob = b"\0alice\0wrong";
        let err = m
            .validate(&AuthInput {
                sasl_plain_blob: Some(blob),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(err, AuthError::Rejected(_)));
    }

    #[test]
    fn json_field_extracts_string() {
        let s = r#"{"alg":"RS256","typ":"JWT"}"#;
        assert_eq!(json_field(s, "alg").as_deref(), Some("RS256"));
    }

    #[test]
    fn json_array_extracts_groups() {
        let s = r#"{"sub":"a","groups":["eng","ops"]}"#;
        let g = json_array(s, "groups").unwrap();
        assert_eq!(g, vec!["eng".to_string(), "ops".to_string()]);
    }

    #[test]
    fn jwt_invalid_signature_rejected() {
        // Bogus pubkey + bogus token. We only test the reject path — a
        // fully-signed round-trip test would need an RSA key-gen lib
        // (rcgen can't do that for JWT). The code path itself is driven
        // against `crate::backend::signature::UnparsedPublicKey::verify`.
        let m = AuthMode::Jwt {
            key: JwtKey::Rs256(vec![0u8; 32]),
            expected_issuer: None,
            expected_audience: None,
            required_claim: None,
        };
        // base64url("{\"alg\":\"RS256\"}") = "eyJhbGciOiJSUzI1NiJ9"
        // base64url("{\"sub\":\"a\"}") = "eyJzdWIiOiJhIn0"
        let token = "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJhIn0.AAAA";
        let err = m
            .validate(&AuthInput {
                authorization_header: Some(&format!("Bearer {token}")),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(err, AuthError::Rejected(_)));
    }

    #[test]
    fn jwt_wrong_alg_rejected() {
        let m = AuthMode::Jwt {
            key: JwtKey::Rs256(vec![0u8; 32]),
            expected_issuer: None,
            expected_audience: None,
            required_claim: None,
        };
        // base64url("{\"alg\":\"HS256\"}") = "eyJhbGciOiJIUzI1NiJ9"
        let token = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhIn0.AAAA";
        let err = m
            .validate(&AuthInput {
                authorization_header: Some(&format!("Bearer {token}")),
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(err, AuthError::Rejected(_)));
    }

    /// An RSA key from `openssl genpkey`: the private half as PKCS#8 DER
    /// for signing, the public half as the SPKI PEM `openssl` writes.
    fn rsa_key() -> (Vec<u8>, String) {
        use std::process::Command;
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("zd-jwt-{}-{seq}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let key = dir.join("key.pem");
        let run = |args: &[&str]| {
            let out = Command::new("openssl")
                .args(args)
                .output()
                .expect("openssl on PATH: the JWT round trip needs a real RSA key");
            assert!(out.status.success(), "openssl {args:?}: {out:?}");
            out.stdout
        };
        run(&[
            "genpkey",
            "-algorithm",
            "RSA",
            "-pkeyopt",
            "rsa_keygen_bits:2048",
            "-out",
            key.to_str().unwrap(),
        ]);
        let der = run(&[
            "pkcs8",
            "-topk8",
            "-nocrypt",
            "-outform",
            "DER",
            "-in",
            key.to_str().unwrap(),
        ]);
        let spki = run(&["pkey", "-pubout", "-in", key.to_str().unwrap()]);
        let _ = std::fs::remove_dir_all(&dir);
        (der, String::from_utf8(spki).unwrap())
    }

    fn sign(pkcs8: &[u8], payload: &str) -> String {
        use crate::backend::{rand, signature};
        use base64::Engine as _;
        let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
        let signed = format!(
            "{}.{}",
            b64(br#"{"alg":"RS256","typ":"JWT"}"#),
            b64(payload.as_bytes())
        );
        let pair = signature::RsaKeyPair::from_pkcs8(pkcs8).unwrap();
        let mut sig = vec![0u8; 256]; // rsa_keygen_bits:2048
        pair.sign(
            &signature::RSA_PKCS1_SHA256,
            &rand::SystemRandom::new(),
            signed.as_bytes(),
            &mut sig,
        )
        .unwrap();
        format!("{signed}.{}", b64(&sig))
    }

    /// A token signed with a real key, checked with its SPKI PEM: the
    /// signature, `exp`, `nbf`, `iss`, `aud` and a required claim.
    #[test]
    fn grpcbridge_7_2_a_signed_jwt_is_checked_for_time_audience_and_claim() {
        let (pkcs8, spki_pem) = rsa_key();
        let mode = AuthMode::Jwt {
            key: JwtKey::Rs256(jwt_public_key_from_pem(&spki_pem).unwrap()),
            expected_issuer: Some("zerodds-test".into()),
            expected_audience: Some("zerodds-grpc".into()),
            required_claim: Some(("scope".into(), "dds.read".into())),
        };
        let now = unix_now();
        let check = |payload: String| {
            mode.validate(&AuthInput {
                authorization_header: Some(&format!("Bearer {}", sign(&pkcs8, &payload))),
                ..Default::default()
            })
        };
        let token = |exp: u64, nbf: u64, aud: &str, scope: &str| {
            format!(
                r#"{{"sub":"alice","iss":"zerodds-test","aud":{aud},"scope":"{scope}","exp":{exp},"nbf":{nbf}}}"#
            )
        };
        let ok = check(token(
            now + 600,
            now - 10,
            r#"["x","zerodds-grpc"]"#,
            "dds.write dds.read",
        ));
        assert_eq!(ok.unwrap().name, "alice");
        let late = now - JWT_CLOCK_LEEWAY_SECS - 1;
        for (why, payload) in [
            (
                "expired",
                token(late, now - 900, r#""zerodds-grpc""#, "dds.read"),
            ),
            (
                "not yet valid",
                token(
                    now + 900,
                    now + JWT_CLOCK_LEEWAY_SECS + 60,
                    r#""zerodds-grpc""#,
                    "dds.read",
                ),
            ),
            ("aud", token(now + 600, now, r#""other""#, "dds.read")),
            (
                "claim",
                token(now + 600, now, r#""zerodds-grpc""#, "dds.write"),
            ),
        ] {
            let err = check(payload).unwrap_err();
            assert!(matches!(err, AuthError::Rejected(_)), "{why}: {err:?}");
        }
        // A signature by another key is refused.
        let (other, _) = rsa_key();
        let forged = format!(
            "Bearer {}",
            sign(
                &other,
                &token(now + 600, now, r#""zerodds-grpc""#, "dds.read")
            )
        );
        assert!(
            mode.validate(&AuthInput {
                authorization_header: Some(&forged),
                ..Default::default()
            })
            .is_err()
        );
    }

    /// RFC 6750 §3.1: a token that verifies and lacks the required claim
    /// is told apart from one that authenticates no one (`insufficient_scope`
    /// against `invalid_token`); `validate` reports both as `Rejected`.
    #[test]
    fn rfc_6750_3_1_a_verified_token_without_the_required_claim_is_told_apart() {
        let (pkcs8, spki_pem) = rsa_key();
        let mode = AuthMode::Jwt {
            key: JwtKey::Rs256(jwt_public_key_from_pem(&spki_pem).unwrap()),
            expected_issuer: None,
            expected_audience: Some("zerodds-ws".into()),
            required_claim: Some(("scope".into(), "dds.read".into())),
        };
        let now = unix_now();
        let header = |exp: u64, aud: &str, scope: &str| {
            format!(
                "Bearer {}",
                sign(
                    &pkcs8,
                    &format!(
                        r#"{{"sub":"alice","aud":"{aud}","groups":["ops"],{scope}"exp":{exp}}}"#
                    )
                )
            )
        };
        let scoped = |header: &str| {
            mode.validate_scoped(&AuthInput {
                authorization_header: Some(header),
                ..Default::default()
            })
        };
        // The claim is there, alone or in a space-separated list.
        for scope in [r#""scope":"dds.read","#, r#""scope":"dds.write dds.read","#] {
            assert_eq!(
                scoped(&header(now + 600, "zerodds-ws", scope))
                    .unwrap()
                    .name,
                "alice"
            );
        }
        // Another value, and no claim at all: the bearer is known.
        for scope in [r#""scope":"dds.write","#, ""] {
            let token = header(now + 600, "zerodds-ws", scope);
            let mut alice = AuthSubject::new("alice");
            alice.groups.push("ops".into());
            assert_eq!(
                scoped(&token),
                Err(Refusal::MissingClaim(Box::new(MissingClaim {
                    subject: alice,
                    claim: "scope".into(),
                    value: "dds.read".into(),
                }))),
                "{scope}"
            );
            // `validate` keeps its answer.
            let plain = mode.validate(&AuthInput {
                authorization_header: Some(&token),
                ..Default::default()
            });
            assert_eq!(
                plain,
                Err(AuthError::Rejected(
                    "jwt: claim scope lacks dds.read".into()
                ))
            );
        }
        // A token that does not verify authenticates no one, whatever its
        // claim says: expired, another audience, another key, no token.
        let (other, _) = rsa_key();
        let forged = format!(
            "Bearer {}",
            sign(
                &other,
                &format!(
                    r#"{{"sub":"alice","aud":"zerodds-ws","exp":{}}}"#,
                    now + 600
                )
            )
        );
        for token in [
            header(now - 3600, "zerodds-ws", r#""scope":"dds.write","#),
            header(now + 600, "elsewhere", r#""scope":"dds.write","#),
            forged,
        ] {
            assert!(
                matches!(
                    scoped(&token),
                    Err(Refusal::Unauthenticated(AuthError::Rejected(_)))
                ),
                "{token}"
            );
        }
        assert_eq!(
            mode.validate_scoped(&AuthInput::default()),
            Err(Refusal::Unauthenticated(AuthError::MissingCredentials))
        );
    }

    /// An EC key from `openssl genpkey` on `curve`: the private half as
    /// PKCS#8 DER, the public half as the SPKI PEM `openssl` writes.
    fn ec_key(curve: &str) -> (Vec<u8>, String) {
        use std::process::Command;
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("zd-jwt-ec-{}-{seq}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let key = dir.join("key.pem");
        let run = |args: &[&str]| {
            let out = Command::new("openssl")
                .args(args)
                .output()
                .expect("openssl on PATH: the ES256 round trip needs a real EC key");
            assert!(out.status.success(), "openssl {args:?}: {out:?}");
            out.stdout
        };
        let curve_opt = format!("ec_paramgen_curve:{curve}");
        run(&[
            "genpkey",
            "-algorithm",
            "EC",
            "-pkeyopt",
            &curve_opt,
            "-out",
            key.to_str().unwrap(),
        ]);
        let der = run(&[
            "pkcs8",
            "-topk8",
            "-nocrypt",
            "-outform",
            "DER",
            "-in",
            key.to_str().unwrap(),
        ]);
        let spki = run(&["pkey", "-pubout", "-in", key.to_str().unwrap()]);
        let _ = std::fs::remove_dir_all(&dir);
        (der, String::from_utf8(spki).unwrap())
    }

    /// A token with the header `{"alg":"ES256"}`, signed with the P-256
    /// key `pkcs8` (the 64-octet `R || S` of RFC 7518 §3.4).
    fn sign_es256(pkcs8: &[u8], payload: &str) -> String {
        use crate::backend::{rand, signature};
        use base64::Engine as _;
        let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
        let signed = format!(
            "{}.{}",
            b64(br#"{"alg":"ES256","typ":"JWT"}"#),
            b64(payload.as_bytes())
        );
        let rng = rand::SystemRandom::new();
        #[cfg(feature = "aws-lc")]
        let pair =
            signature::EcdsaKeyPair::from_pkcs8(&signature::ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8)
                .unwrap();
        #[cfg(not(feature = "aws-lc"))]
        let pair = signature::EcdsaKeyPair::from_pkcs8(
            &signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            pkcs8,
            &rng,
        )
        .unwrap();
        let sig = pair.sign(&rng, signed.as_bytes()).unwrap();
        format!("{signed}.{}", b64(sig.as_ref()))
    }

    fn bearer(mode: &AuthMode, token: &str) -> Result<AuthSubject, AuthError> {
        mode.validate(&AuthInput {
            authorization_header: Some(&format!("Bearer {token}")),
            ..Default::default()
        })
    }

    /// RFC 7515 Appendix A.3: the ES256 example JWS, its key (x, y) and
    /// its signature `R || S`, taken from the RFC text, verify; one octet
    /// changed in the signature or in the signing input does not.
    #[test]
    fn jwt_es256_rfc_7515_a_3_example_verifies() {
        use base64::Engine as _;
        let d = |s: &str| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(s)
                .unwrap()
        };
        let mut point = vec![0x04];
        point.extend(d("f83OJ3D2xF1Bg8vub9tLe1gHMzV76e8Tus9uPHvRVEU"));
        point.extend(d("x_FEzRu9m36HLN_tue659LNpXW6pCyStikYjKIWI5a0"));
        let key = JwtKey::Es256(point);
        let header = "eyJhbGciOiJFUzI1NiJ9";
        let payload = "eyJpc3MiOiJqb2UiLA0KICJleHAiOjEzMDA4MTkzODAsDQogImh0dHA6Ly9leGFtcGxlLmNvbS9pc19yb290Ijp0cnVlfQ";
        let sig = d(
            "DtEhU3ljbEg8L38VWAfUAqOyKAM6-Xx-F4GawxaepmXFCgfTjDxw5djxLa8ISlSApmWQxfKTUJqPP3-Kg6NU1Q",
        );
        assert_eq!(sig.len(), 64);
        verify_jws_signature(&key, header, payload, &sig).expect("RFC 7515 A.3 verifies");
        let mut bad = sig.clone();
        bad[10] ^= 0x01;
        assert!(verify_jws_signature(&key, header, payload, &bad).is_err());
        let other_payload = payload.replacen('J', "K", 1);
        assert!(verify_jws_signature(&key, header, &other_payload, &sig).is_err());
        // The same token through `validate_jwt` before its `exp`: the
        // signature holds, and the token is refused for its missing `sub`.
        let token = format!(
            "{header}.{payload}.{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&sig)
        );
        let checks = JwtChecks {
            issuer: Some("joe"),
            audience: None,
            required_claim: None,
            now: 1_300_819_000,
        };
        assert_eq!(
            validate_jwt(&token, &key, &checks).unwrap_err(),
            AuthError::Rejected("jwt: no sub claim".into())
        );
    }

    /// An ES256 token signed with a P-256 key is checked with the key's
    /// SPKI PEM: subject, groups and time; another key's signature, a
    /// signature in ASN.1 DER form and a P-384 key are refused.
    #[test]
    fn jwt_es256_is_verified() {
        let (pkcs8, spki_pem) = ec_key("P-256");
        let key = jwt_key_from_pem(&spki_pem).unwrap();
        assert!(matches!(&key, JwtKey::Es256(p) if p.len() == 65 && p[0] == 4));
        let mode = AuthMode::Jwt {
            key,
            expected_issuer: None,
            expected_audience: Some("zerodds-ws".into()),
            required_claim: None,
        };
        let now = unix_now();
        let claims = |exp: u64| {
            format!(r#"{{"sub":"alice","aud":"zerodds-ws","groups":["engineers"],"exp":{exp}}}"#)
        };
        let ok = bearer(&mode, &sign_es256(&pkcs8, &claims(now + 600))).unwrap();
        assert_eq!(ok.name, "alice");
        assert_eq!(ok.groups, vec!["engineers".to_string()]);
        assert!(matches!(
            bearer(&mode, &sign_es256(&pkcs8, &claims(now - 3600))),
            Err(AuthError::Rejected(_))
        ));
        let (other, _) = ec_key("P-256");
        assert_eq!(
            bearer(&mode, &sign_es256(&other, &claims(now + 600))),
            Err(AuthError::Rejected("jwt: signature invalid".into()))
        );
        // A DER-encoded ECDSA signature (SEQUENCE { r, s }) is no ES256
        // signature (RFC 7518 §3.4).
        use base64::Engine as _;
        let token = sign_es256(&pkcs8, &claims(now + 600));
        let (signing_input, _) = token.rsplit_once('.').unwrap();
        let der_sig = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0x30u8; 70]);
        assert_eq!(
            bearer(&mode, &format!("{signing_input}.{der_sig}")),
            Err(AuthError::Rejected(
                "jwt: an ES256 signature is 64 octets".into()
            ))
        );
        let (_, p384) = ec_key("P-384");
        assert!(jwt_key_from_pem(&p384).unwrap_err().contains("P-256"));
    }

    /// The `alg` of a token has to be the algorithm of the configured key
    /// (RFC 8725 §3.1): an RS256 token against an ES256 key and an ES256
    /// token against an RS256 key are refused, whatever their signature.
    #[test]
    fn jwt_alg_must_match_the_key_type() {
        let (rsa_pkcs8, rsa_pem) = rsa_key();
        let (ec_pkcs8, ec_pem) = ec_key("P-256");
        let rs = jwt_key_from_pem(&rsa_pem).unwrap();
        assert!(matches!(rs, JwtKey::Rs256(_)));
        let mode = |key| AuthMode::Jwt {
            key,
            expected_issuer: None,
            expected_audience: None,
            required_claim: None,
        };
        let rs_mode = mode(rs);
        let es_mode = mode(jwt_key_from_pem(&ec_pem).unwrap());
        let claims = format!(r#"{{"sub":"alice","exp":{}}}"#, unix_now() + 600);
        let rs_token = sign(&rsa_pkcs8, &claims);
        let es_token = sign_es256(&ec_pkcs8, &claims);
        assert_eq!(bearer(&rs_mode, &rs_token).unwrap().name, "alice");
        assert_eq!(bearer(&es_mode, &es_token).unwrap().name, "alice");
        assert_eq!(
            bearer(&es_mode, &rs_token),
            Err(AuthError::Rejected(
                "jwt: alg must be ES256 for the configured key".into()
            ))
        );
        assert_eq!(
            bearer(&rs_mode, &es_token),
            Err(AuthError::Rejected(
                "jwt: alg must be RS256 for the configured key".into()
            ))
        );
    }

    #[test]
    fn a_pkcs1_pem_and_an_spki_pem_yield_the_same_key() {
        let (_, spki_pem) = rsa_key();
        let from_spki = jwt_public_key_from_pem(&spki_pem).unwrap();
        use base64::Engine as _;
        let pkcs1_pem = format!(
            "-----BEGIN RSA PUBLIC KEY-----\n{}\n-----END RSA PUBLIC KEY-----\n",
            base64::engine::general_purpose::STANDARD.encode(&from_spki)
        );
        assert_eq!(jwt_public_key_from_pem(&pkcs1_pem).unwrap(), from_spki);
        assert!(jwt_public_key_from_pem("no key here").is_err());
    }
}
