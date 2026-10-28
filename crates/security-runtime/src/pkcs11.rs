// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `pkcs11:` URIs (RFC 7512) for the DDS-Security identity.
//!
//! DDS-Security 1.2 §9.3.1 Table 44 allows a `pkcs11:` URI for
//! `dds.sec.auth.identity_ca`, `dds.sec.auth.identity_certificate` and
//! `dds.sec.auth.private_key`; §9.4.1.2 Table 46 allows it for the
//! Access-Control material. The URI names an object on a PKCS#11 token
//! and the module that serves it:
//!
//! ```text
//! pkcs11:token=ZeroDDS;object=identity;id=%01%02;type=cert?module-path=/usr/lib/softhsm/libsofthsm2.so&pin-value=1234
//! ```
//!
//! The parser here reads the path attributes RFC 7512 §2.3 defines —
//! `token`, `object`, `id` (percent-encoded) and `type` — and the query
//! attributes §2.4 defines — `module-path`, `pin-value` and `pin-source`.
//! An attribute this stack does not implement is refused by name, never
//! ignored: a silently dropped `slot-id` would open the wrong token.
//!
//! The certificate material is read from the object's `CKA_VALUE`; the
//! private key stays on the token and signs through `C_Sign`
//! (DDS-Security 1.2 §9.3.2.5). Both need the crate's `pkcs11` feature,
//! which pulls in the `cryptoki` crate; without it a `pkcs11:` value is
//! refused with a message naming the feature.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use std::path::PathBuf;

#[cfg(feature = "pkcs11")]
use alloc::format;
#[cfg(feature = "pkcs11")]
use zerodds_security_pki::CertKeyAlgo;

use crate::qos_properties::percent_decode;

/// The object class a `type=` attribute names (RFC 7512 §2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pkcs11ObjectType {
    /// `type=cert` — a certificate object (`CKO_CERTIFICATE`).
    Cert,
    /// `type=private` — a private key object (`CKO_PRIVATE_KEY`).
    Private,
}

impl Pkcs11ObjectType {
    /// The `type=` spelling.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cert => "cert",
            Self::Private => "private",
        }
    }
}

/// Where the token PIN comes from (RFC 7512 §2.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pkcs11Pin {
    /// `pin-value=` — the PIN itself.
    Value(String),
    /// `pin-source=` — a `file:` URI or a plain path whose content is
    /// the PIN (trailing newlines stripped).
    Source(String),
}

/// A parsed `pkcs11:` URI (RFC 7512 §2.3/§2.4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pkcs11Uri {
    /// `token=` — the token label. `None` selects the first token with a
    /// token present.
    pub token: Option<String>,
    /// `object=` — the `CKA_LABEL` of the object.
    pub object: Option<String>,
    /// `id=` — the `CKA_ID` of the object, percent-decoded.
    pub id: Option<Vec<u8>>,
    /// `type=` — the object class.
    pub object_type: Option<Pkcs11ObjectType>,
    /// `module-path=` — the PKCS#11 module to load.
    pub module_path: Option<PathBuf>,
    /// `pin-value=` / `pin-source=`.
    pub pin: Option<Pkcs11Pin>,
}

/// Why a `pkcs11:` URI does not parse (RFC 7512 §2.3/§2.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pkcs11UriError {
    /// The value does not begin `pkcs11:`.
    MissingScheme,
    /// A path or query segment has no `=`.
    MalformedAttribute(String),
    /// A `%` not followed by two hexadecimal digits.
    PercentEscape {
        /// The attribute whose value carries the escape.
        attribute: String,
        /// The value as written.
        value: String,
    },
    /// A decoded string attribute that is not UTF-8.
    NotUtf8(String),
    /// A path attribute this stack does not implement.
    UnknownPathAttribute(String),
    /// A query attribute this stack does not implement.
    UnknownQueryAttribute(String),
    /// A `type=` value other than `cert` or `private`.
    UnknownObjectType(String),
    /// `pin-value` and `pin-source` both set, or one of them twice.
    DuplicatePin,
}

impl core::fmt::Display for Pkcs11UriError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingScheme => f.write_str("a pkcs11: URI must begin with 'pkcs11:'"),
            Self::MalformedAttribute(a) => {
                write!(f, "the pkcs11: attribute '{a}' has no '='")
            }
            Self::PercentEscape { attribute, value } => write!(
                f,
                "the pkcs11: attribute '{attribute}' = '{value}' has a '%' that does not open a \
                 two-digit hexadecimal escape (RFC 7512 §2.3)"
            ),
            Self::NotUtf8(attribute) => {
                write!(f, "the pkcs11: attribute '{attribute}' is not UTF-8")
            }
            Self::UnknownPathAttribute(a) => write!(
                f,
                "the pkcs11: path attribute '{a}' is not implemented (RFC 7512 §2.3: token, object, \
                 id, type)"
            ),
            Self::UnknownQueryAttribute(a) => write!(
                f,
                "the pkcs11: query attribute '{a}' is not implemented (RFC 7512 §2.4: module-path, \
                 pin-value, pin-source)"
            ),
            Self::UnknownObjectType(t) => write!(
                f,
                "the pkcs11: type attribute '{t}' is not implemented (RFC 7512 §2.3: cert, private)"
            ),
            Self::DuplicatePin => f.write_str(
                "the pkcs11: URI sets both pin-value and pin-source, or one of them twice",
            ),
        }
    }
}

impl std::error::Error for Pkcs11UriError {}

/// Parses a `pkcs11:` URI (RFC 7512 §2.3/§2.4).
///
/// # Errors
/// [`Pkcs11UriError`] for a missing scheme, an unknown attribute (named),
/// an unknown `type`, a broken percent escape or a duplicate PIN source.
pub fn parse(value: &str) -> Result<Pkcs11Uri, Pkcs11UriError> {
    let rest = value
        .strip_prefix("pkcs11:")
        .ok_or(Pkcs11UriError::MissingScheme)?;
    let (path, query) = match rest.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (rest, None),
    };

    let mut uri = Pkcs11Uri::default();
    if !path.is_empty() {
        for attribute in path.split(';') {
            let (name, raw) = attribute
                .split_once('=')
                .ok_or_else(|| Pkcs11UriError::MalformedAttribute(attribute.to_string()))?;
            match name {
                "token" => uri.token = Some(decode_string(name, raw)?),
                "object" => uri.object = Some(decode_string(name, raw)?),
                "id" => uri.id = Some(decode_bytes(name, raw)?),
                "type" => {
                    uri.object_type = Some(match decode_string(name, raw)?.as_str() {
                        "cert" => Pkcs11ObjectType::Cert,
                        "private" => Pkcs11ObjectType::Private,
                        other => return Err(Pkcs11UriError::UnknownObjectType(other.to_string())),
                    });
                }
                other => return Err(Pkcs11UriError::UnknownPathAttribute(other.to_string())),
            }
        }
    }

    if let Some(query) = query {
        if !query.is_empty() {
            for attribute in query.split('&') {
                let (name, raw) = attribute
                    .split_once('=')
                    .ok_or_else(|| Pkcs11UriError::MalformedAttribute(attribute.to_string()))?;
                match name {
                    "module-path" => {
                        uri.module_path = Some(PathBuf::from(decode_string(name, raw)?));
                    }
                    "pin-value" => {
                        if uri.pin.is_some() {
                            return Err(Pkcs11UriError::DuplicatePin);
                        }
                        uri.pin = Some(Pkcs11Pin::Value(decode_string(name, raw)?));
                    }
                    "pin-source" => {
                        if uri.pin.is_some() {
                            return Err(Pkcs11UriError::DuplicatePin);
                        }
                        uri.pin = Some(Pkcs11Pin::Source(decode_string(name, raw)?));
                    }
                    other => return Err(Pkcs11UriError::UnknownQueryAttribute(other.to_string())),
                }
            }
        }
    }

    Ok(uri)
}

fn decode_bytes(attribute: &str, raw: &str) -> Result<Vec<u8>, Pkcs11UriError> {
    percent_decode(raw).ok_or_else(|| Pkcs11UriError::PercentEscape {
        attribute: attribute.to_string(),
        value: raw.to_string(),
    })
}

fn decode_string(attribute: &str, raw: &str) -> Result<String, Pkcs11UriError> {
    let bytes = decode_bytes(attribute, raw)?;
    String::from_utf8(bytes).map_err(|_| Pkcs11UriError::NotUtf8(attribute.to_string()))
}

/// What went wrong reading a certificate from, or signing with, a
/// PKCS#11 token.
#[derive(Debug)]
pub enum Pkcs11Error {
    /// The URI does not parse (RFC 7512 §2.3/§2.4).
    Uri(Pkcs11UriError),
    /// The crate was built without the `pkcs11` feature.
    FeatureDisabled,
    /// The URI names no `module-path`.
    MissingModulePath,
    /// The URI's `type=` names the other object class.
    WrongObjectType {
        /// What the operation needs.
        expected: &'static str,
        /// What the URI said.
        found: &'static str,
    },
    /// Loading or initializing the PKCS#11 module failed.
    Module(String),
    /// No token matches the URI's `token` attribute.
    TokenNotFound(String),
    /// No token is present at all.
    NoToken,
    /// No object matches the URI's `object`/`id`/`type` attributes.
    ObjectNotFound(String),
    /// More than one object matches — an ambiguous URI is refused rather
    /// than resolved to a default.
    ObjectAmbiguous {
        /// How many objects matched.
        count: usize,
        /// The URI, for the message.
        uri: String,
    },
    /// The certificate object carries no `CKA_VALUE`.
    NoValue(String),
    /// Opening the session or logging in failed.
    Session(String),
    /// The PIN source could not be read.
    PinSource(String),
    /// Signing in the token failed.
    Sign(String),
}

impl core::fmt::Display for Pkcs11Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Uri(e) => write!(f, "{e}"),
            Self::FeatureDisabled => f.write_str(
                "a pkcs11: URI (RFC 7512, DDS-Security 1.2 §9.3.1 Table 44) needs the crate's \
                 `pkcs11` feature, which this build does not have",
            ),
            Self::MissingModulePath => f.write_str(
                "the pkcs11: URI names no `module-path`, so no PKCS#11 module can be loaded \
                 (RFC 7512 §2.4)",
            ),
            Self::WrongObjectType { expected, found } => write!(
                f,
                "the pkcs11: URI says type={found}, but {expected} was needed (RFC 7512 §2.3)"
            ),
            Self::Module(m) => write!(f, "PKCS#11 module: {m}"),
            Self::TokenNotFound(label) => write!(f, "no PKCS#11 token with label '{label}'"),
            Self::NoToken => f.write_str("no PKCS#11 token is present"),
            Self::ObjectNotFound(uri) => write!(f, "no PKCS#11 object matches '{uri}'"),
            Self::ObjectAmbiguous { count, uri } => write!(
                f,
                "{count} PKCS#11 objects match '{uri}'; the URI must be made unambiguous"
            ),
            Self::NoValue(uri) => write!(f, "the PKCS#11 object '{uri}' carries no CKA_VALUE"),
            Self::Session(m) => write!(f, "PKCS#11 session: {m}"),
            Self::PinSource(m) => write!(f, "PKCS#11 pin-source: {m}"),
            Self::Sign(m) => write!(f, "PKCS#11 C_Sign: {m}"),
        }
    }
}

impl std::error::Error for Pkcs11Error {}

impl From<Pkcs11UriError> for Pkcs11Error {
    fn from(e: Pkcs11UriError) -> Self {
        Self::Uri(e)
    }
}

/// Reads the DER of the certificate object a `pkcs11:` URI names.
///
/// # Errors
/// [`Pkcs11Error`] when the URI does not parse, the module or token is
/// absent, the object is missing or ambiguous, or it has no `CKA_VALUE`.
#[cfg(feature = "pkcs11")]
pub fn read_certificate_der(uri: &Pkcs11Uri) -> Result<Vec<u8>, Pkcs11Error> {
    require_type(uri, Pkcs11ObjectType::Cert)?;
    let session = open_session(uri)?;
    let object = find_object(&session, uri, ObjectClass::CERTIFICATE)?;
    let attributes = session
        .get_attributes(object, &[AttributeType::Value])
        .map_err(|e| Pkcs11Error::Module(format!("C_GetAttributeValue(CKA_VALUE): {e}")))?;
    for attribute in attributes {
        if let Attribute::Value(value) = attribute {
            if !value.is_empty() {
                return Ok(value);
            }
        }
    }
    Err(Pkcs11Error::NoValue(describe(uri)))
}

/// Without the `pkcs11` feature no module can be loaded.
///
/// # Errors
/// Always [`Pkcs11Error::FeatureDisabled`].
#[cfg(not(feature = "pkcs11"))]
pub fn read_certificate_der(_uri: &Pkcs11Uri) -> Result<Vec<u8>, Pkcs11Error> {
    Err(Pkcs11Error::FeatureDisabled)
}

/// A private key that stays on a PKCS#11 token and signs through
/// `C_Sign` (DDS-Security 1.2 §9.3.2.5).
///
/// The token session is kept open for the signer's lifetime; the key is
/// addressed by the object handle the URI resolved to.
#[cfg(feature = "pkcs11")]
pub struct Pkcs11Signer {
    session: std::sync::Mutex<Session>,
    key: ObjectHandle,
    algo: CertKeyAlgo,
}

#[cfg(feature = "pkcs11")]
impl Pkcs11Signer {
    /// Opens the token the URI names and resolves its private-key object.
    ///
    /// `algo` is the identity certificate's public-key algorithm (from
    /// [`zerodds_security_pki::cert_key_algo`]); it picks the `C_Sign`
    /// mechanism so that the signature is in the encoding the in-memory
    /// path produces for the same key type.
    ///
    /// # Errors
    /// [`Pkcs11Error`] when the URI does not parse, the module or token
    /// is absent, the object is missing or ambiguous, or login fails.
    pub fn open(uri: &Pkcs11Uri, algo: CertKeyAlgo) -> Result<Self, Pkcs11Error> {
        require_type(uri, Pkcs11ObjectType::Private)?;
        let session = open_session(uri)?;
        let key = find_object(&session, uri, ObjectClass::PRIVATE_KEY)?;
        Ok(Self {
            session: std::sync::Mutex::new(session),
            key,
            algo,
        })
    }
}

#[cfg(feature = "pkcs11")]
impl zerodds_security_pki::IdentitySigner for Pkcs11Signer {
    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, zerodds_security_pki::PkiError> {
        use zerodds_security_pki::PkiError;
        let session = self
            .session
            .lock()
            .map_err(|_| PkiError::SigningFailed("pkcs11 session lock is poisoned".to_string()))?;
        match self.algo {
            CertKeyAlgo::EcdsaP256Sha256 => {
                // `CKM_ECDSA` signs a pre-computed digest; the peer
                // verifies with ECDSA-P256-SHA256 and expects ASN.1/DER,
                // which is what the in-memory path writes.
                let digest = Sha256::digest(msg);
                let raw = session
                    .sign(&Mechanism::Ecdsa, self.key, digest.as_slice())
                    .map_err(|e| PkiError::SigningFailed(format!("CKM_ECDSA: {e}")))?;
                ecdsa_raw_to_der(&raw).map_err(PkiError::SigningFailed)
            }
            CertKeyAlgo::EcdsaP384Sha384 => {
                // `CKM_ECDSA` signs a pre-computed digest: SHA-384 for
                // ECDSA+P384+SHA384 (DDS-Security 1.2 §8.2 Table 25).
                let digest = Sha384::digest(msg);
                let raw = session
                    .sign(&Mechanism::Ecdsa, self.key, digest.as_slice())
                    .map_err(|e| PkiError::SigningFailed(format!("CKM_ECDSA: {e}")))?;
                ecdsa_raw_to_der(&raw).map_err(PkiError::SigningFailed)
            }
            CertKeyAlgo::RsaPssSha256 => {
                // The in-memory path signs RSA with RSA-PSS/SHA-256
                // (salt length = digest length), so the token does too.
                let params = PkcsPssParams {
                    hash_alg: MechanismType::SHA256,
                    mgf: PkcsMgfType::MGF1_SHA256,
                    s_len: 32.into(),
                };
                session
                    .sign(&Mechanism::Sha256RsaPkcsPss(params), self.key, msg)
                    .map_err(|e| PkiError::SigningFailed(format!("CKM_SHA256_RSA_PKCS_PSS: {e}")))
            }
            CertKeyAlgo::Unknown => Err(PkiError::SigningFailed(
                "the certificate's key algorithm is not one this stack signs with".to_string(),
            )),
        }
    }
}

/// RFC 7512 §2.3 `type=` against the class an operation needs.
#[cfg(feature = "pkcs11")]
fn require_type(uri: &Pkcs11Uri, expected: Pkcs11ObjectType) -> Result<(), Pkcs11Error> {
    match uri.object_type {
        None => Ok(()),
        Some(found) if found == expected => Ok(()),
        Some(found) => Err(Pkcs11Error::WrongObjectType {
            expected: expected.name(),
            found: found.name(),
        }),
    }
}

/// Loads and initializes the module named by `module-path=`, caching one
/// context per path so a process that reads a certificate and opens a
/// signer does not initialize the same module twice.
#[cfg(feature = "pkcs11")]
fn load_module(uri: &Pkcs11Uri) -> Result<Pkcs11, Pkcs11Error> {
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};

    static MODULES: OnceLock<Mutex<BTreeMap<PathBuf, Pkcs11>>> = OnceLock::new();
    let path = uri
        .module_path
        .clone()
        .ok_or(Pkcs11Error::MissingModulePath)?;
    let modules = MODULES.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = modules
        .lock()
        .map_err(|_| Pkcs11Error::Module("module cache lock is poisoned".to_string()))?;
    if let Some(existing) = guard.get(&path) {
        return Ok(existing.clone());
    }
    let pkcs11 =
        Pkcs11::new(&path).map_err(|e| Pkcs11Error::Module(format!("{}: {e}", path.display())))?;
    match pkcs11.initialize(CInitializeArgs::OsThreads) {
        Ok(()) | Err(CryptokiError::Pkcs11(RvError::CryptokiAlreadyInitialized, _)) => {}
        Err(e) => return Err(Pkcs11Error::Module(format!("{}: {e}", path.display()))),
    }
    guard.insert(path, pkcs11.clone());
    Ok(pkcs11)
}

/// Opens a read-write session on the token the URI names, logging in
/// with the URI's PIN when one is given.
#[cfg(feature = "pkcs11")]
fn open_session(uri: &Pkcs11Uri) -> Result<Session, Pkcs11Error> {
    let pkcs11 = load_module(uri)?;
    let slots = pkcs11
        .get_slots_with_token()
        .map_err(|e| Pkcs11Error::Module(format!("C_GetSlotList: {e}")))?;
    let slot = select_slot(&pkcs11, &slots, uri)?;
    let session = pkcs11
        .open_rw_session(slot)
        .map_err(|e| Pkcs11Error::Session(format!("C_OpenSession: {e}")))?;
    if let Some(pin) = &uri.pin {
        let value = pin_value(pin)?;
        session
            .login(UserType::User, Some(&AuthPin::new(value)))
            .map_err(|e| Pkcs11Error::Session(format!("C_Login: {e}")))?;
    }
    Ok(session)
}

#[cfg(feature = "pkcs11")]
fn select_slot(pkcs11: &Pkcs11, slots: &[Slot], uri: &Pkcs11Uri) -> Result<Slot, Pkcs11Error> {
    match &uri.token {
        None => slots.first().copied().ok_or(Pkcs11Error::NoToken),
        Some(label) => {
            for slot in slots {
                if let Ok(info) = pkcs11.get_token_info(*slot) {
                    if info.label() == label {
                        return Ok(*slot);
                    }
                }
            }
            Err(Pkcs11Error::TokenNotFound(label.clone()))
        }
    }
}

/// Resolves the object class and the `object`/`id` attributes to exactly
/// one object. Labels are compared after stripping the token's trailing
/// NUL/space padding; an ambiguous match is an error.
#[cfg(feature = "pkcs11")]
fn find_object(
    session: &Session,
    uri: &Pkcs11Uri,
    class: ObjectClass,
) -> Result<ObjectHandle, Pkcs11Error> {
    let objects = session
        .find_objects(&[Attribute::Class(class)])
        .map_err(|e| Pkcs11Error::Module(format!("C_FindObjects: {e}")))?;
    let mut matches = Vec::new();
    for object in objects {
        let attributes = session
            .get_attributes(object, &[AttributeType::Label, AttributeType::Id])
            .map_err(|e| Pkcs11Error::Module(format!("C_GetAttributeValue: {e}")))?;
        let mut label = None;
        let mut id = None;
        for attribute in attributes {
            match attribute {
                Attribute::Label(value) => label = Some(value),
                Attribute::Id(value) => id = Some(value),
                _ => {}
            }
        }
        if label_matches(uri.object.as_deref(), label.as_deref())
            && id_matches(uri.id.as_deref(), id.as_deref())
        {
            matches.push(object);
        }
    }
    match matches.len() {
        1 => Ok(matches[0]),
        0 => Err(Pkcs11Error::ObjectNotFound(describe(uri))),
        count => Err(Pkcs11Error::ObjectAmbiguous {
            count,
            uri: describe(uri),
        }),
    }
}

#[cfg(feature = "pkcs11")]
fn label_matches(wanted: Option<&str>, actual: Option<&[u8]>) -> bool {
    let Some(wanted) = wanted else {
        return true;
    };
    let Some(actual) = actual else {
        return false;
    };
    String::from_utf8_lossy(actual).trim_end_matches(['\0', ' ']) == wanted
}

#[cfg(feature = "pkcs11")]
fn id_matches(wanted: Option<&[u8]>, actual: Option<&[u8]>) -> bool {
    match wanted {
        None => true,
        Some(wanted) => actual == Some(wanted),
    }
}

#[cfg(feature = "pkcs11")]
fn pin_value(pin: &Pkcs11Pin) -> Result<String, Pkcs11Error> {
    match pin {
        Pkcs11Pin::Value(value) => Ok(value.clone()),
        Pkcs11Pin::Source(source) => {
            let path = pin_source_path(source);
            let raw = std::fs::read(&path)
                .map_err(|e| Pkcs11Error::PinSource(format!("{}: {e}", path.display())))?;
            let text = String::from_utf8(raw)
                .map_err(|_| Pkcs11Error::PinSource(format!("{}: not UTF-8", path.display())))?;
            Ok(text.trim_end_matches(['\r', '\n']).to_string())
        }
    }
}

#[cfg(feature = "pkcs11")]
fn pin_source_path(source: &str) -> PathBuf {
    if source.starts_with("file://") {
        PathBuf::from(crate::profile::strip_file_url(source))
    } else if let Some(rest) = source.strip_prefix("file:") {
        PathBuf::from(rest)
    } else {
        PathBuf::from(source)
    }
}

#[cfg(feature = "pkcs11")]
fn describe(uri: &Pkcs11Uri) -> String {
    let mut out = String::from("pkcs11:");
    if let Some(token) = &uri.token {
        out.push_str(&format!("token={token};"));
    }
    if let Some(object) = &uri.object {
        out.push_str(&format!("object={object};"));
    }
    if let Some(id) = &uri.id {
        out.push_str("id=");
        for byte in id {
            out.push_str(&format!("{byte:02x}"));
        }
    }
    out.trim_end_matches(';').to_string()
}

/// Converts the raw `r||s` a PKCS#11 `CKM_ECDSA` returns into the
/// ASN.1/DER `SEQUENCE { INTEGER r, INTEGER s }` the peer verifies.
#[cfg(feature = "pkcs11")]
fn ecdsa_raw_to_der(raw: &[u8]) -> Result<Vec<u8>, String> {
    if raw.is_empty() || raw.len() % 2 != 0 {
        return Err(format!(
            "CKM_ECDSA returned {} bytes, not a raw r||s pair",
            raw.len()
        ));
    }
    let (r, s) = raw.split_at(raw.len() / 2);
    let r = der_integer(r);
    let s = der_integer(s);
    let mut body = Vec::with_capacity(r.len() + s.len());
    body.extend_from_slice(&r);
    body.extend_from_slice(&s);
    let mut out = Vec::with_capacity(body.len() + 4);
    out.push(0x30);
    der_length(&mut out, body.len());
    out.extend_from_slice(&body);
    Ok(out)
}

#[cfg(feature = "pkcs11")]
fn der_integer(bytes: &[u8]) -> Vec<u8> {
    let mut start = 0;
    while start + 1 < bytes.len() && bytes[start] == 0 {
        start += 1;
    }
    let value = &bytes[start..];
    let needs_pad = value.first().is_some_and(|byte| byte & 0x80 != 0);
    let mut out = Vec::with_capacity(value.len() + 3);
    out.push(0x02);
    der_length(&mut out, value.len() + usize::from(needs_pad));
    if needs_pad {
        out.push(0x00);
    }
    out.extend_from_slice(value);
    out
}

#[cfg(feature = "pkcs11")]
fn der_length(out: &mut Vec<u8>, len: usize) {
    if len < 0x80 {
        out.push(len as u8);
        return;
    }
    let mut bytes = Vec::new();
    let mut remaining = len;
    while remaining > 0 {
        bytes.push((remaining & 0xff) as u8);
        remaining >>= 8;
    }
    bytes.reverse();
    out.push(0x80 | bytes.len() as u8);
    out.extend_from_slice(&bytes);
}

#[cfg(feature = "pkcs11")]
use cryptoki::context::{CInitializeArgs, Pkcs11};
#[cfg(feature = "pkcs11")]
use cryptoki::error::{Error as CryptokiError, RvError};
#[cfg(feature = "pkcs11")]
use cryptoki::mechanism::Mechanism;
#[cfg(feature = "pkcs11")]
use cryptoki::mechanism::MechanismType;
#[cfg(feature = "pkcs11")]
use cryptoki::mechanism::rsa::{PkcsMgfType, PkcsPssParams};
#[cfg(feature = "pkcs11")]
use cryptoki::object::{Attribute, AttributeType, ObjectClass, ObjectHandle};
#[cfg(feature = "pkcs11")]
use cryptoki::session::{Session, UserType};
#[cfg(feature = "pkcs11")]
use cryptoki::slot::Slot;
#[cfg(feature = "pkcs11")]
use cryptoki::types::AuthPin;
#[cfg(feature = "pkcs11")]
use sha2::{Digest, Sha256, Sha384};

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn every_path_attribute_parses() {
        let uri = parse("pkcs11:token=ZeroDDS;object=identity;id=%01%02;type=cert").unwrap();
        assert_eq!(uri.token.as_deref(), Some("ZeroDDS"));
        assert_eq!(uri.object.as_deref(), Some("identity"));
        assert_eq!(uri.id.as_deref(), Some(&[0x01, 0x02][..]));
        assert_eq!(uri.object_type, Some(Pkcs11ObjectType::Cert));
    }

    #[test]
    fn private_type_and_an_empty_uri_parse() {
        assert_eq!(
            parse("pkcs11:type=private").unwrap().object_type,
            Some(Pkcs11ObjectType::Private)
        );
        assert_eq!(parse("pkcs11:").unwrap(), Pkcs11Uri::default());
    }

    #[test]
    fn percent_encoding_is_decoded_in_strings_and_ids() {
        let uri = parse("pkcs11:token=The%20Soft%23token;object=a%2Fb;id=%DE%AD%BE%EF").unwrap();
        assert_eq!(uri.token.as_deref(), Some("The Soft#token"));
        assert_eq!(uri.object.as_deref(), Some("a/b"));
        assert_eq!(uri.id.as_deref(), Some(&[0xDE, 0xAD, 0xBE, 0xEF][..]));
    }

    #[test]
    fn every_query_attribute_parses() {
        let uri =
            parse("pkcs11:token=t?module-path=/usr/lib/softhsm/libsofthsm2.so&pin-value=1234")
                .unwrap();
        assert_eq!(
            uri.module_path.as_deref(),
            Some(std::path::Path::new("/usr/lib/softhsm/libsofthsm2.so"))
        );
        assert_eq!(uri.pin, Some(Pkcs11Pin::Value("1234".to_string())));

        let uri = parse("pkcs11:token=t?pin-source=/etc/zerodds/pin").unwrap();
        assert_eq!(
            uri.pin,
            Some(Pkcs11Pin::Source("/etc/zerodds/pin".to_string()))
        );
    }

    #[test]
    fn an_unknown_path_attribute_is_refused_by_name() {
        let err = parse("pkcs11:slot-id=0").expect_err("slot-id is not implemented");
        match err {
            Pkcs11UriError::UnknownPathAttribute(name) => assert_eq!(name, "slot-id"),
            other => panic!("expected an unknown path attribute, got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_query_attribute_is_refused_by_name() {
        let err = parse("pkcs11:token=t?library-manufacturer=x")
            .expect_err("library-manufacturer is not implemented");
        match err {
            Pkcs11UriError::UnknownQueryAttribute(name) => assert_eq!(name, "library-manufacturer"),
            other => panic!("expected an unknown query attribute, got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_type_is_refused_by_value() {
        let err = parse("pkcs11:type=public").expect_err("type=public is not implemented");
        match err {
            Pkcs11UriError::UnknownObjectType(value) => assert_eq!(value, "public"),
            other => panic!("expected an unknown object type, got {other:?}"),
        }
    }

    #[test]
    fn a_broken_percent_escape_is_refused() {
        assert!(matches!(
            parse("pkcs11:id=%0"),
            Err(Pkcs11UriError::PercentEscape { .. })
        ));
        assert!(matches!(
            parse("pkcs11:id=%zz"),
            Err(Pkcs11UriError::PercentEscape { .. })
        ));
    }

    #[test]
    fn a_duplicate_pin_source_is_refused() {
        assert_eq!(
            parse("pkcs11:token=t?pin-value=1&pin-source=/etc/pin"),
            Err(Pkcs11UriError::DuplicatePin)
        );
    }

    #[test]
    fn a_missing_scheme_and_a_malformed_attribute_are_refused() {
        assert_eq!(parse("token=t"), Err(Pkcs11UriError::MissingScheme));
        assert!(matches!(
            parse("pkcs11:token"),
            Err(Pkcs11UriError::MalformedAttribute(_))
        ));
    }

    #[cfg(feature = "pkcs11")]
    #[test]
    fn ecdsa_raw_is_rewritten_as_der() {
        // r with the high bit set must gain a leading zero; s with a
        // leading zero byte must lose it.
        let mut raw = Vec::new();
        raw.extend_from_slice(&[0x80; 32]);
        raw.extend_from_slice(&[0x00; 32]);
        let der = ecdsa_raw_to_der(&raw).unwrap();
        assert_eq!(der[0], 0x30);
        assert_eq!(der[2], 0x02);
        assert_eq!(der[3], 33, "r is padded to stay positive");
        assert_eq!(der[4], 0x00);
        assert_eq!(der[5], 0x80);
        let s_at = 4 + 33;
        assert_eq!(der[s_at], 0x02);
        assert_eq!(der[s_at + 1], 1, "s is trimmed to its significant byte");
        assert_eq!(der[s_at + 2], 0x00);
    }

    #[cfg(feature = "pkcs11")]
    #[test]
    fn an_odd_raw_signature_is_refused() {
        assert!(ecdsa_raw_to_der(&[0u8; 63]).is_err());
        assert!(ecdsa_raw_to_der(&[]).is_err());
    }
}
