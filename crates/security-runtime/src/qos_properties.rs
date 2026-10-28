// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The security-event logger is a plug-in boundary; `Arc<dyn LoggingPlugin>` is
// the intended heterogeneous sink type (mirrors `RuntimeConfig.security_logger`).
// zerodds-lint: allow no_dyn_in_safe
//! DDS-Security configuration read off the participant's
//! [`PropertyQosPolicy`] — the spec's own way of selecting and
//! configuring the plugins.
//!
//! DDS-Security 1.2 §9.1.2 leaves how plugins are selected to the
//! implementation; the builtin plugins read their configuration from the
//! `PropertyQosPolicy` of the `DomainParticipantQos` (§7.3.21), in the
//! `dds.sec.` namespace §7.3.2 reserves. ZeroDDS selects and configures
//! the plugins from that list. The two property tables this module reads
//! are:
//!
//! - **§10.3.1, Table 50** — the builtin Authentication plugin:
//!   `dds.sec.auth.identity_ca`, `dds.sec.auth.identity_certificate`,
//!   `dds.sec.auth.private_key`, `dds.sec.auth.password` and
//!   `dds.sec.auth.key_establishment_algorithm`.
//! - **§10.4.1, Table 63** — the builtin Access Control plugin:
//!   `dds.sec.access.permissions_ca`, `dds.sec.access.governance` and
//!   `dds.sec.access.permissions`.
//!
//! The spec writes every one of those values as a **URI** — `file:`,
//! `data:` or `pkcs11:`. [`resolve_file_uri`] covers the three `file:`
//! spellings that occur in real deployments, and [`resolve_material`]
//! reads `data:,` as the content itself and `pkcs11:` (RFC 7512) as an
//! object on a PKCS#11 token. A `pkcs11:` value needs the crate's
//! `pkcs11` feature; without it, [`resolve_material`] refuses naming the
//! feature rather than opening the URI as if it were a path.
//!
//! ## Why this lives in a crate and not in a client
//!
//! Before this module, nothing in `crates/dcps` read these properties.
//! `RuntimeConfig::with_security_log_properties` read the `dds.sec.log.*`
//! subset and there was no counterpart for the rest: a participant
//! created with a complete and correct property list came up **without**
//! a `SecurityBundle` — plaintext RTPS, with nothing on either side
//! saying so. That is the silent-drop failure class this tree names in
//! `docs/silent-drop-sweep.md`, applied to the whole security stack.
//!
//! Hence [`security_bundle_from_properties`]: one function, one
//! [`SecurityProfile`], used by `DomainParticipant::new_with_runtime`.
//! The **one** profile matters. `profile.pki` is simultaneously the
//! crypto gate's `SharedSecretProvider` and the handshake driver's
//! plugin, and a shared secret is only resolvable in the instance that
//! derived it. Two profiles built from the same six files give the gate
//! one PKI and the handshake driver another: the handshake runs to
//! completion, the gate then finds no secret for the peer, and secured
//! DATA carries no keys — again with nothing saying so. This function
//! therefore builds the profile once and hands it out only inside the
//! [`SecurityBundle`], from which the caller reads the very same `Arc`
//! back out.

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use std::path::PathBuf;

use zerodds_qos::PropertyQosPolicy;
use zerodds_security::logging::LoggingPlugin;
use zerodds_security::properties::PropertyList;
use zerodds_security_pki::{CertPolicy, KxSuite, property_enables_deviation};

use crate::bundle::SecurityBundle;
use crate::profile::{
    Material, SecurityProfile, SecurityProfileConfig, SecurityProfileError, strip_file_url,
};

/// DDS-Security 1.2 §10.3.1 Table 50 — the identity CA (trust anchor for
/// remote identity certificates).
pub const PROP_IDENTITY_CA: &str = "dds.sec.auth.identity_ca";
/// DDS-Security 1.2 §10.3.1 Table 50 — this participant's identity certificate.
pub const PROP_IDENTITY_CERT: &str = "dds.sec.auth.identity_certificate";
/// DDS-Security 1.2 §10.3.1 Table 50 — this participant's private key.
pub const PROP_IDENTITY_KEY: &str = "dds.sec.auth.private_key";
/// DDS-Security 1.2 §10.3.1 Table 50 — the password of an encrypted
/// [`PROP_IDENTITY_KEY`]: "the Base64 encoding of the AES-128 key that shall
/// be used to decrypt the private_key using AES128-CBC". Also read as an
/// OpenSSL passphrase when [`PROP_PASSWORD_READING`] says so. It does not
/// apply to a `pkcs11:` key. A `?password=` on a `file:` key URI stands in
/// for it.
pub const PROP_IDENTITY_KEY_PASSWORD: &str = "dds.sec.auth.password";

/// What a certificate gets whose revocation status OCSP could not determine:
/// `fail` (default) or `accept` (DDS-Security 1.2 §10.3.3 Table 59 asks for
/// the check; `accept` passes the certificate and logs a warning).
pub const PROP_OCSP_UNREACHABLE: &str = "org.zerodds.sec.auth.ocsp_unreachable";
/// How long a query to an OCSP responder may take, in milliseconds (default
/// 2000).
pub const PROP_OCSP_TIMEOUT_MS: &str = "org.zerodds.sec.auth.ocsp_timeout_ms";
/// How `dds.sec.auth.password` is read: `spec` (default, the Base64 AES-128
/// key of DDS-Security 1.2 §10.3.1 Table 50) or `openssl_passphrase` (what
/// Cyclone DDS and Fast DDS do besides: also an OpenSSL passphrase).
pub const PROP_PASSWORD_READING: &str = "org.zerodds.sec.auth.password_reading";

/// The certificate revocation list(s) of the identity CA, a `file:` or
/// `data:` URI with PEM or DER (DDS-Security 1.2 §10.3.3 Table 59: "The
/// operation shall check a CRL"; the specification names no property). The
/// name is the one Cyclone DDS reads.
pub const PROP_IDENTITY_CRL: &str = "org.zerodds.sec.auth.crl";
/// The name Fast DDS reads for the same list.
pub const PROP_IDENTITY_CRL_FASTDDS: &str = "dds.sec.auth.builtin.PKI-DH.identity_crl";

/// The key width of the builtin crypto plugin (DDS-Security 1.2 §10.5.1
/// Table 71): `AUTO`, or one of the names of Table 22; `AUTO` and a missing
/// property are `AES256-GCM`.
pub const PROP_SYMMETRIC_CIPHER: &str = "dds.sec.crypto.symmetric_cipher_algorithm";

/// `max_blocks_per_session` of the builtin crypto plugin (DDS-Security 1.2
/// §10.5.3.3.2 Table 81: "A configurable property"; the specification
/// names none). The name is the one Fast DDS reads.
pub const PROP_MAX_BLOCKS_PER_SESSION: &str = "org.zerodds.sec.crypto.max_blocks_per_session";
/// How the partitions of a permissions allow rule match: `subset` (default)
/// or `legacy` (DDS-Security 1.2 §10.4.1.5.3.4.1.4: "DDS-Security
/// implementations shall provide a way to select an alternative 'legacy
/// matching' behavior"). The specification names no property; this one is
/// ZeroDDS's.
pub const PROP_PARTITION_MATCHING: &str = "org.zerodds.sec.access.partition_matching";
/// `true` takes plaintext RTPS messages over loopback on an RTPS-protected
/// domain. DDS-Security 1.2 §9.4.2.4 makes no exception for loopback; the
/// default `false` follows it. ZeroDDS's property.
pub const PROP_PLAINTEXT_LOOPBACK: &str = "org.zerodds.sec.plaintext_loopback";
/// ZeroDDS — whether the participant announces
/// `participant_security_optional_attributes` (DDS-Security 1.2 §7.3.23,
/// Table 35): `send` (the specification, the default) or `omit`, for a peer
/// that reads only the two masks of version 1.1 and refuses a longer
/// `PID_PARTICIPANT_SECURITY_INFO`.
pub const PROP_OPTIONAL_ATTRIBUTES: &str = "org.zerodds.sec.optional_attributes";

/// Whether participants and endpoints with an incompatible configuration are
/// kept apart (DDS-Security 1.2 §7.3.10 to §7.3.24): `spec` (the default) or
/// `ignore`, for a vendor whose announced masks differ.
pub const PROP_COMPATIBILITY_CHECK: &str = "org.zerodds.sec.compatibility_check";

/// Whether a submessage or payload that no key of its sender opens is tried
/// with the Kx key as well: `spec` (the default, DDS-Security 1.2 §10.5.2.1.2
/// gives that key to the volatile channel only) or `accept`, for a vendor that
/// protects discovery or user data with it.
pub const PROP_KX_KEY_FALLBACK: &str = "org.zerodds.sec.kx_key_fallback";

/// How the algorithm names `c.dsign_algo` and `c.kagree_algo` of a handshake
/// token are written: `spec` (the default, with the NUL, DDS-Security 1.2
/// §7.3.3.1), `omit` (without it, the form of Fast DDS), `mirror` (with it, a
/// replier in the form of the request) or `by_vendor` (with it
/// for an OpenDDS peer only). A name of the vendor namespace of ZeroDDS
/// (§7.3.2: the reversed domain name).
pub const PROP_ALGO_NAME_NUL: &str = "org.zerodds.sec.algo_name_nul";
/// The spelling of `c.dsign_algo` and `c.kagree_algo` as handshake initiator
/// (DDS-Security 1.2 §8.2 Table 25, §8.3 Table 26): `1.2` (the default, the
/// names of the tables), `1.1` (the deprecated names Cyclone DDS, Fast DDS and
/// OpenDDS read), or `auto` (the names of 1.2 to a peer whose IdentityToken
/// names version 1.2, those of 1.1 to a peer of 1.0).
pub const PROP_ALGORITHM_NAMES: &str = "org.zerodds.sec.algorithm_names";
/// The environment variable that gives [`PROP_ALGORITHM_NAMES`] its value when
/// the property list has none.
pub const ENV_ALGORITHM_NAMES: &str = "ZERODDS_SEC_ALGORITHM_NAMES";
/// The form of the subject names in the IdentityToken (`dds.cert.sn`,
/// `dds.ca.sn`) and the PermissionsToken (`dds.perm_ca.sn`): `rfc4514` (the
/// default, DDS-Security 1.2 §9.4.1.2.3), `x509_name_oneline` (OpenSSL
/// `X509_NAME_oneline`: Fast DDS compares it, Cyclone DDS writes it) or
/// `xn_flag_oneline` (OpenSSL `XN_FLAG_ONELINE`: OpenDDS compares it).
pub const PROP_SUBJECT_NAME_FORM: &str = "org.zerodds.sec.subject_name_form";
/// The environment variable that gives [`PROP_SUBJECT_NAME_FORM`] its value
/// when the property list has none.
pub const ENV_SUBJECT_NAME_FORM: &str = "ZERODDS_SEC_SUBJECT_NAME_FORM";
/// What a participant whose governance asks for origin authentication (RTPS
/// or Metadata Protection Kind `*_WITH_ORIGIN_AUTHENTICATION`) does with an
/// RTPS message or submessage under a key without a receiver-specific key:
/// `spec` (the default) refuses it, as DDS-Security 1.2 §10.5.3.3 Table 80
/// `decode_rtps_message` asks; `accept` verifies it with the common MAC alone,
/// as Table 80 does for a key whose receiver_specific_key_id is zero. The
/// form of a sender that makes no receiver-specific keys although its
/// governance asks for them (OpenDDS 3.34).
pub const PROP_ORIGIN_AUTHENTICATION_FALLBACK: &str =
    "org.zerodds.sec.origin_authentication_fallback";
/// The environment variable that gives [`PROP_ORIGIN_AUTHENTICATION_FALLBACK`]
/// its value when the property list has none.
pub const ENV_ORIGIN_AUTHENTICATION_FALLBACK: &str = "ZERODDS_SEC_ORIGIN_AUTHENTICATION_FALLBACK";
/// DDS-Security 1.2 §10.3.1 Table 50 — the key agreement to announce as
/// handshake initiator.
pub const PROP_KEY_ESTABLISHMENT: &str = "dds.sec.auth.key_establishment_algorithm";
/// DDS-Security 1.2 §10.4.1 Table 63 — the permissions CA (signs
/// governance and permissions).
pub const PROP_PERMISSIONS_CA: &str = "dds.sec.access.permissions_ca";
/// DDS-Security 1.2 §10.4.1 Table 63 — the CMS-signed governance document.
pub const PROP_GOVERNANCE: &str = "dds.sec.access.governance";
/// DDS-Security 1.2 §10.4.1 Table 63 — the CMS-signed permissions document.
pub const PROP_PERMISSIONS: &str = "dds.sec.access.permissions";

/// The property-name prefixes that request security: DDS-Security 1.2
/// §7.3.2 reserves `dds.sec.` for the plugins, §10.3.1 puts the
/// Authentication plugin under `dds.sec.auth.` and §10.4.1 the Access
/// Control plugin under `dds.sec.access.`.
///
/// `dds.sec.log.*` (§8.6, the Logging plugin) is deliberately **not**
/// here: a logger configures observability of a participant that may
/// have no other security at all, and it has had its own path
/// (`RuntimeConfig::with_security_log_properties`) since before this
/// module existed.
const SECURITY_REQUESTING_PREFIXES: [&str; 2] = ["dds.sec.auth.", "dds.sec.access."];

/// What went wrong turning a `dds.sec.*` property list into security.
///
/// Every variant is a **refusal to create the participant**. None of
/// them falls back to plaintext: a property list that asks for security
/// and does not get it must fail loudly, because the alternative is an
/// unencrypted participant that nobody was told about.
#[derive(Debug)]
pub enum SecurityPropertyError {
    /// A required `dds.sec.*` property is absent from the list.
    MissingProperty(&'static str),
    /// A property's value is a URI whose scheme this stack does not open
    /// (DDS-Security 1.2 allows `file:`, `data:` and `pkcs11:`; `file:`,
    /// plain paths and `data:,` are implemented).
    UnsupportedUriScheme {
        /// The property whose value carries the scheme.
        property: &'static str,
        /// The scheme, as written.
        scheme: String,
    },
    /// A `pkcs11:` URI (RFC 7512) was given but the crate was built
    /// without the `pkcs11` feature, so no token can be opened.
    Pkcs11FeatureDisabled {
        /// The property whose value carries the URI.
        property: &'static str,
    },
    /// A `pkcs11:` URI (RFC 7512, DDS-Security 1.2 §9.3.1 Table 44) that
    /// does not parse, or a token operation that failed.
    Pkcs11 {
        /// The property whose value carries the URI.
        property: &'static str,
        /// What went wrong.
        error: crate::pkcs11::Pkcs11Error,
    },
    /// A `data:` URI (RFC 2397) that does not decode.
    MalformedDataUri {
        /// The property whose value carries it.
        property: &'static str,
        /// What is wrong with it.
        why: &'static str,
    },
    /// [`PROP_KEY_ESTABLISHMENT`] named a key agreement this stack cannot run.
    UnsupportedKeyEstablishment(String),
    /// [`PROP_MAX_BLOCKS_PER_SESSION`] is not a positive whole number.
    InvalidMaxBlocksPerSession(String),
    /// [`PROP_SYMMETRIC_CIPHER`] names none of the algorithms of Table 22.
    InvalidSymmetricCipher(String),
    /// [`PROP_PARTITION_MATCHING`] is neither `subset` nor `legacy`.
    InvalidPartitionMatching(String),
    /// [`PROP_ALGORITHM_NAMES`] is none of `auto`, `1.2` and `1.1`.
    InvalidAlgorithmNames(String),
    /// [`PROP_SUBJECT_NAME_FORM`] is none of `rfc4514`, `x509_name_oneline`
    /// and `xn_flag_oneline`.
    InvalidSubjectNameForm(String),
    /// [`PROP_PASSWORD_READING`] is neither `spec` nor `openssl_passphrase`.
    InvalidPasswordReading(String),
    /// A property of `DDS:Crypto:PSK` (DDS-Security 1.2 §10.9.1 Table 86) is
    /// not what the table says; the text names which.
    PskCrypto(String),
    /// [`PROP_OCSP_UNREACHABLE`] or [`PROP_OCSP_TIMEOUT_MS`] has a value
    /// that is not one of its spellings.
    InvalidOcspSetting {
        /// The property.
        property: &'static str,
        /// Its value.
        value: String,
    },
    /// [`PROP_PLAINTEXT_LOOPBACK`] is neither `true` nor `false`.
    InvalidPlaintextLoopback(String),
    /// [`PROP_COMPATIBILITY_CHECK`] is neither `spec` nor `ignore`.
    InvalidCompatibilityCheck(String),
    /// [`PROP_KX_KEY_FALLBACK`] is neither `spec` nor `accept`.
    InvalidKxKeyFallback(String),
    /// [`PROP_ORIGIN_AUTHENTICATION_FALLBACK`] is neither `spec` nor `accept`.
    InvalidOriginAuthenticationFallback(String),
    /// [`PROP_OPTIONAL_ATTRIBUTES`] is neither `send` nor `omit`.
    InvalidOptionalAttributes(String),
    /// [`PROP_ALGO_NAME_NUL`] is none of `spec`, `omit`, `by_vendor`.
    InvalidAlgoNameNul(String),
    /// Reading, verifying or parsing the named material failed.
    Profile(SecurityProfileError),
    /// DDS-Security 1.2 §9.4.2.9.3 `check_create_participant` denied the
    /// join: access control is enabled and no permissions grant covers
    /// this domain.
    JoinDenied {
        /// The domain the participant tried to join.
        domain_id: u32,
    },
    /// The AccessControl plugin could not be armed or refused the
    /// participant (DDS-Security 1.2 §9.4.2.9.1, §9.4.2.9.3, Table 70): its
    /// error, as the gate reports it.
    AccessControl(String),
    /// The PKI plugin's lock was poisoned before the profile could be
    /// finished.
    PkiLockPoisoned,
}

impl core::fmt::Display for SecurityPropertyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingProperty(name) => write!(
                f,
                "the participant QoS asks for DDS-Security and the property '{name}' is not set \
                 (DDS-Security 1.2 §10.3.1 Table 50 / §10.4.1 Table 63)"
            ),
            Self::UnsupportedUriScheme { property, scheme } => write!(
                f,
                "'{property}': the URI scheme '{scheme}:' is not implemented in this form — this \
                 stack opens 'file:' URIs, plain paths and 'data:,<content>' (DDS-Security 1.2 \
                 §10.3.1 also allows 'pkcs11:')"
            ),
            Self::Pkcs11FeatureDisabled { property } => write!(
                f,
                "'{property}': a pkcs11: URI (RFC 7512, DDS-Security 1.2 §9.3.1 Table 44) needs \
                 this crate's `pkcs11` feature, which this build does not have"
            ),
            Self::Pkcs11 { property, error } => write!(
                f,
                "'{property}': {error} (DDS-Security 1.2 §9.3.1 Table 44, RFC 7512)"
            ),
            Self::MalformedDataUri { property, why } => write!(
                f,
                "'{property}': a data: URI (RFC 2397, DDS-Security 1.2 §10.3.1 Table 50) with {why}"
            ),
            Self::UnsupportedKeyEstablishment(algo) => write!(
                f,
                "'{PROP_KEY_ESTABLISHMENT}' = '{algo}': this stack runs AUTO, ECDHE-CEUM+P256, \
                 ECDHE-CEUM+P384, DHE+MODP-2048-256 and the X25519 vendor extension"
            ),
            Self::InvalidSymmetricCipher(why) => f.write_str(why),
            Self::InvalidMaxBlocksPerSession(value) => write!(
                f,
                "'{PROP_MAX_BLOCKS_PER_SESSION}' = '{value}': max_blocks_per_session is a positive \
                 whole number (DDS-Security 1.2 §10.5.3.3.2 Table 81)"
            ),
            Self::InvalidOcspSetting { property, value } => write!(
                f,
                "'{property}' = '{value}': {}",
                if *property == PROP_OCSP_UNREACHABLE {
                    "fail or accept"
                } else {
                    "a positive whole number of milliseconds"
                }
            ),
            Self::PskCrypto(why) => write!(f, "DDS:Crypto:PSK (§10.9.1 Table 86): {why}"),
            Self::InvalidPasswordReading(value) => write!(
                f,
                "'{PROP_PASSWORD_READING}' = '{value}': spec or openssl_passphrase"
            ),
            Self::InvalidKxKeyFallback(value) => {
                write!(f, "'{PROP_KX_KEY_FALLBACK}' = '{value}': spec or accept")
            }
            Self::InvalidOriginAuthenticationFallback(value) => write!(
                f,
                "'{PROP_ORIGIN_AUTHENTICATION_FALLBACK}' = '{value}': spec or accept (DDS-Security \
                 1.2 §10.5.3.3 Table 80)"
            ),
            Self::InvalidOptionalAttributes(value) => write!(
                f,
                "'{PROP_OPTIONAL_ATTRIBUTES}' = '{value}': send or omit (DDS-Security 1.2 §7.3.23)"
            ),
            Self::InvalidAlgoNameNul(value) => write!(
                f,
                "'{PROP_ALGO_NAME_NUL}' = '{value}': spec, omit, mirror or by_vendor"
            ),
            Self::InvalidCompatibilityCheck(value) => write!(
                f,
                "'{PROP_COMPATIBILITY_CHECK}' = '{value}': spec or ignore"
            ),
            Self::InvalidPlaintextLoopback(value) => {
                write!(f, "'{PROP_PLAINTEXT_LOOPBACK}' = '{value}': true or false")
            }
            Self::InvalidPartitionMatching(value) => write!(
                f,
                "'{PROP_PARTITION_MATCHING}' = '{value}': subset or legacy (DDS-Security 1.2 \
                 §10.4.1.5.3.4.1.4)"
            ),
            Self::InvalidAlgorithmNames(value) => write!(
                f,
                "'{PROP_ALGORITHM_NAMES}' = '{value}': auto, 1.2 or 1.1 (DDS-Security 1.2 §8.2, §8.3)"
            ),
            Self::InvalidSubjectNameForm(value) => write!(
                f,
                "'{PROP_SUBJECT_NAME_FORM}' = '{value}': rfc4514, x509_name_oneline or \
                 xn_flag_oneline (DDS-Security 1.2 §9.4.1.2.3)"
            ),
            Self::Profile(e) => write!(f, "{e}"),
            Self::JoinDenied { domain_id } => write!(
                f,
                "check_create_participant denied for domain {domain_id}: join access control is \
                 enabled and no permissions grant covers this domain (DDS-Security 1.2 §9.4.2.9.3)"
            ),
            Self::AccessControl(why) => write!(f, "AccessControl plugin: {why}"),
            Self::PkiLockPoisoned => f.write_str("the PKI plugin's lock is poisoned"),
        }
    }
}

impl std::error::Error for SecurityPropertyError {}

impl From<SecurityProfileError> for SecurityPropertyError {
    fn from(e: SecurityProfileError) -> Self {
        Self::Profile(e)
    }
}

/// `true` when the property list asks for DDS-Security.
///
/// The rule is the namespace, not a specific property: any
/// `dds.sec.auth.*` or `dds.sec.access.*` entry means somebody
/// configured the Authentication or Access Control plugin
/// (DDS-Security 1.2 §7.3.21 / Tables 50, 63, 71), and an incomplete list is then an
/// error rather than a reason to run plaintext.
#[must_use]
pub fn requests_security(props: &PropertyQosPolicy) -> bool {
    props.iter().any(|(name, _)| {
        SECURITY_REQUESTING_PREFIXES
            .iter()
            .any(|p| name.starts_with(p))
    })
}

/// Resolves a DDS-Security property value to a path.
///
/// DDS-Security 1.2 §10.3.1 Table 50 writes these values as URIs. The
/// three `file:` spellings that occur in real deployments:
///
/// - `file:///abs/path` and `file://host/path` — delegated to
///   [`strip_file_url`], which is where that knowledge already lives.
/// - `file:relative/path` — the scheme with no authority at all, which
///   is what the OMG `dds-security` interoperability suite and
///   DDS-Security 1.2's own Table 63 examples write.
///   [`strip_file_url`] returns that string unchanged, so an open would
///   fail on a path literally beginning `file:`.
/// - a plain path, which every vendor also accepts.
///
/// # Errors
/// [`SecurityPropertyError::UnsupportedUriScheme`] for `data:` and
/// `pkcs11:`: neither is a path. A `data:` value is content and
/// [`resolve_material`] reads it; a `pkcs11:` value names a token object
/// and [`resolve_material`] parses it (RFC 7512). Both are named rather
/// than opened as a relative path that does not exist.
pub fn resolve_file_uri(
    property: &'static str,
    value: &str,
) -> Result<PathBuf, SecurityPropertyError> {
    if value.starts_with("file://") {
        return Ok(PathBuf::from(strip_file_url(value)));
    }
    if let Some(rest) = value.strip_prefix("file:") {
        return Ok(PathBuf::from(rest));
    }
    for scheme in ["data", "pkcs11"] {
        if value.starts_with(scheme) && value[scheme.len()..].starts_with(':') {
            return Err(SecurityPropertyError::UnsupportedUriScheme {
                property,
                scheme: scheme.to_string(),
            });
        }
    }
    Ok(PathBuf::from(value))
}

/// Resolves a DDS-Security property value to the material it names.
///
/// A `data:` URI is the content itself — DDS-Security 1.2 §10.3.1
/// Table 50: "URI 'data' schema follows IETF RFC 2397":
///
/// - `data:,<content>` — the form the table's own examples and every
///   vendor write — is the text after the comma, unchanged, newlines
///   included. Cyclone DDS takes it the same way
///   (`DDS_Security_get_conf_item_type`, which also skips leading
///   blanks), and so does this.
/// - `data:<mediatype>;base64,<payload>` is the payload, base64-decoded.
/// - `data:<mediatype>,<payload>` is the payload, percent-decoded, which
///   is RFC 2397's rule for a non-base64 payload.
///
/// A `pkcs11:` URI (RFC 7512) — DDS-Security 1.2 §9.3.1 Table 44 allows
/// one for `identity_ca`, `identity_certificate` and `private_key` — is
/// parsed into a [`crate::pkcs11::Pkcs11Uri`]. The certificate material
/// is then read from the token's `CKA_VALUE`; a private key stays on the
/// token and is signed with through `C_Sign`. This needs the crate's
/// `pkcs11` feature; without it the value is refused naming the feature.
///
/// Every other value is a path, resolved by [`resolve_file_uri`].
///
/// # Errors
/// [`SecurityPropertyError::MalformedDataUri`] for a `data:` URI with no
/// comma, a payload that is not base64 where `;base64` says it is, or a
/// broken percent escape; [`SecurityPropertyError::Pkcs11FeatureDisabled`]
/// or [`SecurityPropertyError::Pkcs11`] for a `pkcs11:` value; otherwise
/// as [`resolve_file_uri`].
pub fn resolve_material(
    property: &'static str,
    value: &str,
) -> Result<Material, SecurityPropertyError> {
    let value = value.trim_start_matches([' ', '\t']);
    if value.starts_with("pkcs11:") {
        #[cfg(not(feature = "pkcs11"))]
        {
            return Err(SecurityPropertyError::Pkcs11FeatureDisabled { property });
        }
        #[cfg(feature = "pkcs11")]
        {
            let uri =
                crate::pkcs11::parse(value).map_err(|error| SecurityPropertyError::Pkcs11 {
                    property,
                    error: crate::pkcs11::Pkcs11Error::Uri(error),
                })?;
            return Ok(Material::Pkcs11(uri));
        }
    }
    let Some(uri) = value.strip_prefix("data:") else {
        return resolve_file_uri(property, value).map(Material::File);
    };
    let malformed = |why: &'static str| SecurityPropertyError::MalformedDataUri { property, why };
    let (header, payload) = uri
        .split_once(',')
        .ok_or_else(|| malformed("no comma between the media type and the data"))?;
    if header.is_empty() {
        return Ok(Material::Inline(payload.as_bytes().to_vec()));
    }
    if header
        .rsplit(';')
        .next()
        .is_some_and(|last| last.eq_ignore_ascii_case("base64"))
    {
        use base64::Engine as _;
        return base64::engine::general_purpose::STANDARD
            .decode(payload)
            .map(Material::Inline)
            .map_err(|_| malformed("`;base64` and a payload that is not base64"));
    }
    percent_decode(payload)
        .map(Material::Inline)
        .ok_or_else(|| malformed("a `%` not followed by two hexadecimal digits"))
}

/// Resolves the `private_key` property: [`resolve_material`], with the
/// password a `file:` URI carries in its query, `file:key.pem?password=…`
/// (DDS-Security 1.2 §10.3.1 Table 50, example). The query is not part of
/// the path; its `password` is percent-decoded and returned beside the
/// material. A `data:` and a `pkcs11:` value keep their `?` (a `pkcs11:`
/// one carries `pin-value`, RFC 7512 §2.3).
///
/// # Errors
/// As [`resolve_material`]; [`SecurityPropertyError::MalformedDataUri`]
/// for a password with a broken percent escape.
pub fn resolve_private_key(
    property: &'static str,
    value: &str,
) -> Result<(Material, Option<String>), SecurityPropertyError> {
    let trimmed = value.trim_start_matches([' ', '\t']);
    let is_file = !trimmed.starts_with("data:") && !trimmed.starts_with("pkcs11:");
    let Some((path, query)) = trimmed.split_once('?').filter(|_| is_file) else {
        return Ok((resolve_material(property, value)?, None));
    };
    let mut password = None;
    for pair in query.split('&') {
        if let Some(raw) = pair.strip_prefix("password=") {
            let bytes = percent_decode(raw).ok_or(SecurityPropertyError::MalformedDataUri {
                property,
                why: "a `password` with a `%` not followed by two hexadecimal digits",
            })?;
            password = Some(String::from_utf8_lossy(&bytes).into_owned());
        }
    }
    Ok((resolve_material(property, path)?, password))
}

/// RFC 3986 §2.1 percent-decoding, which RFC 2397 applies to a `data:`
/// payload that is not base64 and RFC 7512 §2.3 to a `pkcs11:` attribute
/// value. `None` for a `%` that does not open a two-digit escape.
pub(crate) fn percent_decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        if byte == b'%' {
            let digits = bytes.get(index + 1..index + 3)?;
            let text = core::str::from_utf8(digits).ok()?;
            out.push(u8::from_str_radix(text, 16).ok()?);
            index += 3;
        } else {
            out.push(byte);
            index += 1;
        }
    }
    Some(out)
}

/// Reads a [`SecurityProfileConfig`] out of the property list.
///
/// The list is the single source: every path comes from a `dds.sec.*`
/// property. There is no default for any of the six — a participant that
/// cannot name its own identity has nothing to authenticate with, and a
/// fallback would authenticate as somebody else.
///
/// # Errors
/// [`SecurityPropertyError::MissingProperty`] naming the first absent
/// property, or [`SecurityPropertyError::UnsupportedUriScheme`] /
/// [`SecurityPropertyError::MalformedDataUri`].
pub fn profile_config_from_properties(
    props: &PropertyQosPolicy,
    domain_id: u32,
) -> Result<SecurityProfileConfig, SecurityPropertyError> {
    profile_config_from_properties_with_env(props, domain_id, &|name| std::env::var(name).ok())
}

/// The environment variable that gives [`PROP_KX_KEY_FALLBACK`] its value when
/// the property list has none (`spec` or `accept`).
pub const ENV_KX_KEY_FALLBACK: &str = "ZERODDS_SEC_KX_KEY_FALLBACK";

/// The environment variable that gives [`PROP_COMPATIBILITY_CHECK`] its value
/// when the property list has none (`spec` or `ignore`).
pub const ENV_COMPATIBILITY_CHECK: &str = "ZERODDS_SEC_COMPATIBILITY_CHECK";

fn parse_kx_key_fallback(value: Option<&str>) -> Result<bool, SecurityPropertyError> {
    match value.map(str::trim) {
        None | Some("spec") => Ok(false),
        Some("accept") => Ok(true),
        Some(other) => Err(SecurityPropertyError::InvalidKxKeyFallback(
            other.to_string(),
        )),
    }
}

fn parse_origin_authentication_fallback(
    value: Option<&str>,
) -> Result<bool, SecurityPropertyError> {
    match value.map(str::trim) {
        None | Some("spec") => Ok(false),
        Some("accept") => Ok(true),
        Some(other) => Err(SecurityPropertyError::InvalidOriginAuthenticationFallback(
            other.to_string(),
        )),
    }
}

fn parse_algo_name_nul(
    value: Option<&str>,
) -> Result<zerodds_security::AlgoNameNul, SecurityPropertyError> {
    match value.map(str::trim) {
        None => Ok(zerodds_security::AlgoNameNul::default()),
        Some(text) => zerodds_security::AlgoNameNul::parse(text)
            .ok_or_else(|| SecurityPropertyError::InvalidAlgoNameNul(text.to_string())),
    }
}

fn parse_compatibility_checks(value: Option<&str>) -> Result<bool, SecurityPropertyError> {
    match value.map(str::trim) {
        None | Some("spec") => Ok(true),
        Some("ignore") => Ok(false),
        Some(other) => Err(SecurityPropertyError::InvalidCompatibilityCheck(
            other.to_string(),
        )),
    }
}

/// The spelling of the algorithm names as initiator: the value of
/// [`PROP_ALGORITHM_NAMES`], else of [`ENV_ALGORITHM_NAMES`]; `1.2` without
/// either.
///
/// # Errors
/// [`SecurityPropertyError::InvalidAlgorithmNames`] for any other value.
pub fn algorithm_names_from_properties(
    props: &PropertyQosPolicy,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<zerodds_security_pki::AlgorithmNames, SecurityPropertyError> {
    let value = props
        .get(PROP_ALGORITHM_NAMES)
        .map(ToString::to_string)
        .or_else(|| env(ENV_ALGORITHM_NAMES));
    match value {
        None => Ok(zerodds_security_pki::AlgorithmNames::V1_2),
        Some(value) => zerodds_security_pki::AlgorithmNames::parse(&value)
            .map_err(|_| SecurityPropertyError::InvalidAlgorithmNames(value)),
    }
}

/// The form of the subject names in the tokens: the value of
/// [`PROP_SUBJECT_NAME_FORM`], else of [`ENV_SUBJECT_NAME_FORM`]; RFC 4514
/// without either.
///
/// # Errors
/// [`SecurityPropertyError::InvalidSubjectNameForm`] for any other value.
pub fn subject_name_form_from_properties(
    props: &PropertyQosPolicy,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<zerodds_security::SubjectNameForm, SecurityPropertyError> {
    let value = props
        .get(PROP_SUBJECT_NAME_FORM)
        .map(ToString::to_string)
        .or_else(|| env(ENV_SUBJECT_NAME_FORM));
    match value.as_deref().map(str::trim) {
        None => Ok(zerodds_security::SubjectNameForm::default()),
        Some(text) => zerodds_security::SubjectNameForm::parse(text)
            .ok_or_else(|| SecurityPropertyError::InvalidSubjectNameForm(text.to_string())),
    }
}

/// The environment variable that gives [`PROP_ALGO_NAME_NUL`] its value when
/// the property list has none.
pub const ENV_ALGO_NAME_NUL: &str = "ZERODDS_SEC_ALGO_NAME_NUL";

/// The vendor-form settings of a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VendorFormSettings {
    /// [`PROP_KX_KEY_FALLBACK`] is `accept`.
    pub kx_key_fallback: bool,
    /// [`PROP_COMPATIBILITY_CHECK`] is not `ignore`.
    pub compatibility_checks: bool,
    /// [`PROP_ALGO_NAME_NUL`].
    pub algo_name_nul: zerodds_security::AlgoNameNul,
    /// [`PROP_ALGORITHM_NAMES`].
    pub algorithm_names: zerodds_security_pki::AlgorithmNames,
    /// [`PROP_SUBJECT_NAME_FORM`].
    pub subject_name_form: zerodds_security::SubjectNameForm,
    /// [`PROP_ORIGIN_AUTHENTICATION_FALLBACK`] is `accept`.
    pub origin_authentication_fallback: bool,
}

/// The vendor-form settings from the environment alone, for a profile that
/// has no property list (the enclave directory of SROS2): the values of
/// [`ENV_KX_KEY_FALLBACK`] (`spec`, `accept`), [`ENV_COMPATIBILITY_CHECK`]
/// (`spec`, `ignore`) and [`ENV_ALGO_NAME_NUL`] (`spec`, `omit`,
/// `by_vendor`); absent is `spec` in each.
///
/// # Errors
/// `InvalidKxKeyFallback`, `InvalidCompatibilityCheck` or `InvalidAlgoNameNul`
/// for any other value.
pub fn vendor_form_settings_from_env(
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<VendorFormSettings, SecurityPropertyError> {
    Ok(VendorFormSettings {
        kx_key_fallback: parse_kx_key_fallback(env(ENV_KX_KEY_FALLBACK).as_deref())?,
        compatibility_checks: parse_compatibility_checks(env(ENV_COMPATIBILITY_CHECK).as_deref())?,
        algo_name_nul: parse_algo_name_nul(env(ENV_ALGO_NAME_NUL).as_deref())?,
        algorithm_names: algorithm_names_from_properties(&PropertyQosPolicy::new(), env)?,
        subject_name_form: subject_name_form_from_properties(&PropertyQosPolicy::new(), env)?,
        origin_authentication_fallback: parse_origin_authentication_fallback(
            env(ENV_ORIGIN_AUTHENTICATION_FALLBACK).as_deref(),
        )?,
    })
}

/// [`profile_config_from_properties`] with the environment given: for the two
/// vendor-form settings [`PROP_KX_KEY_FALLBACK`] and
/// [`PROP_COMPATIBILITY_CHECK`] the property wins, and the variable
/// ([`ENV_KX_KEY_FALLBACK`], [`ENV_COMPATIBILITY_CHECK`]) is the value when the
/// list has none, so that a test run can switch them on without a change of
/// the profile files.
///
/// # Errors
/// As [`profile_config_from_properties`].
pub fn profile_config_from_properties_with_env(
    props: &PropertyQosPolicy,
    domain_id: u32,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<SecurityProfileConfig, SecurityPropertyError> {
    let kx_fallback_value = props
        .get(PROP_KX_KEY_FALLBACK)
        .map(ToString::to_string)
        .or_else(|| env(ENV_KX_KEY_FALLBACK));
    let algo_name_value = props
        .get(PROP_ALGO_NAME_NUL)
        .map(ToString::to_string)
        .or_else(|| env(ENV_ALGO_NAME_NUL));
    let compatibility_value = props
        .get(PROP_COMPATIBILITY_CHECK)
        .map(ToString::to_string)
        .or_else(|| env(ENV_COMPATIBILITY_CHECK));
    let origin_fallback_value = props
        .get(PROP_ORIGIN_AUTHENTICATION_FALLBACK)
        .map(ToString::to_string)
        .or_else(|| env(ENV_ORIGIN_AUTHENTICATION_FALLBACK));
    let path = |name: &'static str| -> Result<Material, SecurityPropertyError> {
        match props.get(name) {
            Some(value) => resolve_material(name, value),
            None => Err(SecurityPropertyError::MissingProperty(name)),
        }
    };
    let private_key = match props.get(PROP_IDENTITY_KEY) {
        Some(value) => resolve_private_key(PROP_IDENTITY_KEY, value)?,
        None => return Err(SecurityPropertyError::MissingProperty(PROP_IDENTITY_KEY)),
    };
    Ok(SecurityProfileConfig {
        domain_id,
        identity_ca_pem: path(PROP_IDENTITY_CA)?,
        identity_cert_pem: path(PROP_IDENTITY_CERT)?,
        identity_key_pem: private_key.0,
        permissions_ca_pem: path(PROP_PERMISSIONS_CA)?,
        governance_p7s: path(PROP_GOVERNANCE)?,
        permissions_p7s: path(PROP_PERMISSIONS)?,
        // §10.3.1 Table 50 `dds.sec.auth.password`. An empty one is the
        // absent one.
        identity_key_password: props
            .get(PROP_IDENTITY_KEY_PASSWORD)
            .filter(|password| !password.is_empty())
            .map(ToString::to_string)
            .or(private_key.1),
        identity_key_password_reading: match props.get(PROP_PASSWORD_READING) {
            None | Some("spec") => zerodds_security_pki::encrypted_key::PasswordReading::Clause,
            Some("openssl_passphrase") => {
                zerodds_security_pki::encrypted_key::PasswordReading::ClauseThenPassphrase
            }
            Some(other) => {
                return Err(SecurityPropertyError::InvalidPasswordReading(
                    other.to_string(),
                ));
            }
        },
        identity_crl: props
            .get(PROP_IDENTITY_CRL)
            .map(|value| (PROP_IDENTITY_CRL, value))
            .or_else(|| {
                props
                    .get(PROP_IDENTITY_CRL_FASTDDS)
                    .map(|value| (PROP_IDENTITY_CRL_FASTDDS, value))
            })
            .map(|(name, value)| resolve_material(name, value))
            .transpose()?,
        symmetric_cipher_bits: zerodds_security_crypto::symmetric_cipher_key_bits(
            PROP_SYMMETRIC_CIPHER,
            props.get(PROP_SYMMETRIC_CIPHER),
        )
        .map_err(|e| SecurityPropertyError::InvalidSymmetricCipher(e.to_string()))?,
        max_blocks_per_session: props
            .get(PROP_MAX_BLOCKS_PER_SESSION)
            .map(|value| {
                value
                    .trim()
                    .parse::<u64>()
                    .ok()
                    .filter(|blocks| *blocks > 0)
                    .ok_or_else(|| {
                        SecurityPropertyError::InvalidMaxBlocksPerSession(value.to_string())
                    })
            })
            .transpose()?,
        partition_matching: match props.get(PROP_PARTITION_MATCHING).map(str::trim) {
            None | Some("subset") => zerodds_security_permissions::PartitionMatching::Subset,
            Some("legacy") => zerodds_security_permissions::PartitionMatching::Legacy,
            Some(other) => {
                return Err(SecurityPropertyError::InvalidPartitionMatching(
                    other.to_string(),
                ));
            }
        },
        kx_key_fallback: parse_kx_key_fallback(kx_fallback_value.as_deref())?,
        algo_name_nul: parse_algo_name_nul(algo_name_value.as_deref())?,
        algorithm_names: algorithm_names_from_properties(props, env)?,
        subject_name_form: subject_name_form_from_properties(props, env)?,
        origin_authentication_fallback: parse_origin_authentication_fallback(
            origin_fallback_value.as_deref(),
        )?,
        compatibility_checks: parse_compatibility_checks(compatibility_value.as_deref())?,
        plaintext_loopback: match props.get(PROP_PLAINTEXT_LOOPBACK).map(str::trim) {
            None | Some("false") => false,
            Some("true") => true,
            Some(other) => {
                return Err(SecurityPropertyError::InvalidPlaintextLoopback(
                    other.to_string(),
                ));
            }
        },
    })
}

/// The identity-certificate validation policy the property list asks for.
///
/// Read out of the list like everything else here, so that what the
/// participant announces and what its PKI plugin does cannot disagree.
/// An absent property is [`CertPolicy::spec`], which is DDS-Security 1.2
/// §10.3.1.3.
#[must_use]
pub fn cert_policy_from_properties(props: &PropertyQosPolicy) -> CertPolicy {
    CertPolicy {
        accept_x509v1_identity_certificate: props
            .get(zerodds_security_pki::PROP_ACCEPT_X509V1_IDENTITY)
            .is_some_and(property_enables_deviation),
        expected_subject: None,
        // The CRLs come with the profile config (`identity_crl`), read when
        // the profile is built.
        crls_der: Vec::new(),
        // Read by `ocsp_policy_from_properties`, which can refuse a value.
        ocsp: zerodds_security_pki::OcspPolicy::default(),
        // Set by the profile from `algorithm_names`.
        algorithm_naming: Default::default(),
    }
}

/// How the OCSP check of the identity certificates behaves (DDS-Security 1.2
/// §10.3.3 Table 59, which asks for the check and names no setting):
/// [`PROP_OCSP_UNREACHABLE`] says what a certificate whose status could not
/// be determined gets, [`PROP_OCSP_TIMEOUT_MS`] bounds a query to a
/// responder. Defaults: refuse, 2000 ms.
///
/// # Errors
/// [`SecurityPropertyError::InvalidOcspSetting`] for a value that is not
/// one of the two spellings or a whole number of milliseconds: a setting
/// nobody can read must not turn the check off.
pub fn ocsp_policy_from_properties(
    props: &PropertyQosPolicy,
) -> Result<zerodds_security_pki::OcspPolicy, SecurityPropertyError> {
    let mut policy = zerodds_security_pki::OcspPolicy::default();
    if let Some(value) = props.get(PROP_OCSP_UNREACHABLE) {
        policy.unreachable = match value {
            "fail" => zerodds_security_pki::OcspUnreachable::Fail,
            "accept" => zerodds_security_pki::OcspUnreachable::Accept,
            other => {
                return Err(SecurityPropertyError::InvalidOcspSetting {
                    property: PROP_OCSP_UNREACHABLE,
                    value: other.to_string(),
                });
            }
        };
    }
    if let Some(value) = props.get(PROP_OCSP_TIMEOUT_MS) {
        let ms = value
            .trim()
            .parse::<u64>()
            .ok()
            .filter(|ms| *ms > 0)
            .ok_or_else(|| SecurityPropertyError::InvalidOcspSetting {
                property: PROP_OCSP_TIMEOUT_MS,
                value: value.to_string(),
            })?;
        policy.timeout = core::time::Duration::from_millis(ms);
    }
    Ok(policy)
}

/// Maps [`PROP_KEY_ESTABLISHMENT`] onto the key agreement this stack runs.
///
/// DDS-Security 1.2 §8.3 Table 26 spells the P-256 suite
/// `ECDHE-CEUM+P256`. What goes on the wire as `c.kagree_algo` is a
/// separate question and not this property's to answer:
/// `crates/security-pki` emits `ECDH+prime256v1-CEUM` there, because
/// that is the string CycloneDDS and Fast DDS were captured sending. So
/// the property selects the *suite* and the plugin keeps the spelling it
/// measured; both spellings are accepted here, plus the in-tree legacy
/// one, so no operator has to know which is which.
///
/// An absent property is `ECDHE-CEUM+P256` — Table 26's default.
///
/// # Errors
/// [`SecurityPropertyError::UnsupportedKeyEstablishment`] for anything
/// else. A named algorithm this stack cannot run is refused rather than
/// silently replaced with P-256, which would produce a handshake nobody
/// asked for.
pub fn kx_suite_from_properties(
    props: &PropertyQosPolicy,
) -> Result<KxSuite, SecurityPropertyError> {
    match props.get(PROP_KEY_ESTABLISHMENT) {
        // DDS-Security 1.2 §10.3.1 Table 50: omitted or "AUTO" leaves the choice
        // to the plugin, which takes the key agreement every vendor runs.
        None
        | Some("AUTO")
        | Some("ECDHE-CEUM+P256")
        | Some("ECDH+prime256v1-CEUM")
        | Some("ECDHE-CEUM-P256") => Ok(KxSuite::EcdhP256),
        Some("ECDHE-CEUM+P384") => Ok(KxSuite::EcdhP384),
        Some("DHE+MODP-2048-256") | Some("DH+MODP-2048-256") => Ok(KxSuite::DhModp2048_256),
        Some("X25519") => Ok(KxSuite::X25519),
        Some(other) => Err(SecurityPropertyError::UnsupportedKeyEstablishment(
            other.to_string(),
        )),
    }
}

/// The `PropertyQosPolicy` of the participant as the `PropertyList` the
/// AccessControl plugin reads its settings from.
fn participant_properties(props: &PropertyQosPolicy) -> PropertyList {
    let mut list = PropertyList::new();
    for (name, value) in props.iter() {
        list.set(zerodds_security::properties::Property::local(
            name.to_string(),
            value.to_string(),
        ));
    }
    list
}

/// Arms the gate of `profile` with its AccessControl plugin and asks it
/// `check_create_participant` (DDS-Security 1.2 §9.4.2.9.3) for
/// `domain_id`.
fn check_join(
    profile: &SecurityProfile,
    domain_id: u32,
    properties: &PropertyList,
) -> Result<(), SecurityPropertyError> {
    crate::endpoint_access::arm_endpoint_access_control(profile)
        .map_err(|e| SecurityPropertyError::AccessControl(e.to_string()))?;
    let allowed = profile
        .gate
        .check_create_participant(properties)
        .map_err(|e| SecurityPropertyError::AccessControl(e.to_string()))?;
    if allowed {
        Ok(())
    } else {
        Err(SecurityPropertyError::JoinDenied { domain_id })
    }
}

/// Builds a participant's [`SecurityBundle`] from its own
/// `PropertyQosPolicy` — the whole DDS-Security 1.2 §7.3.21 / Tables 50, 63, 71 path in
/// one call.
///
/// `candidate_guid` is the 16-byte GUID the participant would use
/// without security; [`SecurityProfile::from_files_with_cert_policy`]
/// replaces its prefix with the identity-derived one (§9.3.3) and
/// `RuntimeConfig::with_security_bundle` carries that adjusted prefix
/// into the runtime. `logger` is the security-event sink (§8.6) the
/// participant already has, wired into the PKI plugin so that a remote
/// peer's rejected identity is reported where the operator is looking.
///
/// The returned bundle carries **the** [`SecurityProfile`] — read it
/// back out with [`SecurityBundle::security_profile`] to arm the
/// handshake driver on the same instance the gate holds. Building a
/// second profile from the same files instead is the failure this
/// module's header describes.
///
/// # Errors
/// [`SecurityPropertyError`] — every variant refuses to create the
/// participant. This function never returns a bundle without a profile.
pub fn security_bundle_from_properties(
    props: &PropertyQosPolicy,
    domain_id: u32,
    candidate_guid: [u8; 16],
    logger: Option<Arc<dyn LoggingPlugin>>,
) -> Result<SecurityBundle, SecurityPropertyError> {
    security_bundle_from_properties_with_env(props, domain_id, candidate_guid, logger, &|name| {
        std::env::var(name).ok()
    })
}

/// [`security_bundle_from_properties`] with the vendor-form variables
/// (`ZERODDS_SEC_*`) given as a lookup: the runtime configuration resolves
/// them over the environment and the configuration files (`security.*`).
///
/// # Errors
/// As [`security_bundle_from_properties`].
pub fn security_bundle_from_properties_with_env(
    props: &PropertyQosPolicy,
    domain_id: u32,
    candidate_guid: [u8; 16],
    logger: Option<Arc<dyn LoggingPlugin>>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<SecurityBundle, SecurityPropertyError> {
    let cfg = profile_config_from_properties_with_env(props, domain_id, env)?;
    let mut cert_policy = cert_policy_from_properties(props);
    cert_policy.ocsp = ocsp_policy_from_properties(props)?;
    let suite = kx_suite_from_properties(props)?;
    let profile = SecurityProfile::from_files_with_cert_policy(&cfg, candidate_guid, cert_policy)?;
    // §10.5.1 Table 71, §10.8.1 Table 84: `rtps_psk_secret_passphrase` enables
    // the protection of RTPS messages with a pre-shared key.
    if let Some(psk) = rtps_psk_from_properties(
        props,
        domain_id,
        profile
            .governance
            .find_domain_rule(domain_id)
            .map(|rule| rule.rtps_psk_protection_kind),
    )? {
        profile
            .gate
            .set_rtps_psk(psk)
            .map_err(|e| SecurityPropertyError::PskCrypto(alloc::format!("{e}")))?;
    }

    // DDS-Security 1.2 §9.4.2.9.3 `check_create_participant`: the
    // AccessControl plugin (governance and permissions together) decides
    // whether this participant may join this domain at all. Denying here
    // rather than at the first rejected endpoint turns a permissions
    // mismatch into a one-line failure instead of a silent domain with no
    // matches in it.
    let properties = participant_properties(props);
    match props.get(PROP_OPTIONAL_ATTRIBUTES).map(str::trim) {
        None | Some("send") => {}
        Some("omit") => profile
            .gate
            .set_optional_attributes(false)
            .map_err(|e| SecurityPropertyError::AccessControl(e.to_string()))?,
        Some(other) => {
            return Err(SecurityPropertyError::InvalidOptionalAttributes(
                other.to_string(),
            ));
        }
    }
    if let Some(class) = properties.get(zerodds_security_permissions::PROP_TOKEN_CLASS_ID) {
        profile
            .gate
            .set_permissions_token_class(class)
            .map_err(|e| SecurityPropertyError::AccessControl(e.to_string()))?;
    }
    check_join(&profile, domain_id, &properties)?;

    match profile.pki.lock() {
        Ok(mut pki) => {
            pki.set_preferred_kx_suite(suite);
            // §8.6: the same sink the participant's runtime got. The
            // *local* identity was validated inside
            // `from_files_with_cert_policy` above, before this slot
            // existed, and its event went to the plugin's own stderr
            // fallback — visible either way, which is the property that
            // matters.
            if let Some(logger) = logger {
                pki.set_security_logger(logger);
            }
        }
        Err(_) => return Err(SecurityPropertyError::PkiLockPoisoned),
    }

    Ok(SecurityBundle::builder().security_profile(profile).build())
}

/// The bundle of an SROS2 enclave directory (`security.enclave_dir` of the
/// runtime configuration, env `ZERODDS_SECURITY_DIR`), with the same join
/// check and logger wiring as [`security_bundle_from_properties`]. Every
/// Rust participant goes through it when its configuration names an
/// enclave; before, only the ROS 2 RMW and the C-API's secured
/// constructor read the setting, and any other program accepted it and
/// started unsecured.
///
/// # Errors
/// [`SecurityPropertyError`] — a file missing or invalid, the join denied.
pub fn security_bundle_from_enclave_dir(
    enclave_dir: &std::path::Path,
    domain_id: u32,
    candidate_guid: [u8; 16],
    logger: Option<Arc<dyn LoggingPlugin>>,
) -> Result<SecurityBundle, SecurityPropertyError> {
    security_bundle_from_enclave_dir_with_env(
        enclave_dir,
        domain_id,
        candidate_guid,
        logger,
        &|name| std::env::var(name).ok(),
    )
}

/// [`security_bundle_from_enclave_dir`] with the vendor-form variables
/// (`ZERODDS_SEC_*`) given as a lookup, see
/// [`security_bundle_from_properties_with_env`].
///
/// # Errors
/// As [`security_bundle_from_enclave_dir`].
pub fn security_bundle_from_enclave_dir_with_env(
    enclave_dir: &std::path::Path,
    domain_id: u32,
    candidate_guid: [u8; 16],
    logger: Option<Arc<dyn LoggingPlugin>>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<SecurityBundle, SecurityPropertyError> {
    let profile =
        SecurityProfile::from_enclave_dir_with_env(enclave_dir, domain_id, candidate_guid, env)?;
    check_join(&profile, domain_id, &PropertyList::new())?;
    match profile.pki.lock() {
        Ok(mut pki) => {
            if let Some(logger) = logger {
                pki.set_security_logger(logger);
            }
        }
        Err(_) => return Err(SecurityPropertyError::PkiLockPoisoned),
    }
    Ok(SecurityBundle::builder().security_profile(profile).build())
}

/// The pre-shared-key protection of the participant, `None` for none
/// (DDS-Security 1.2 §10.5.1 Table 71, §10.8.1 Table 84): the passphrase
/// property "enables pre-shared-key (PSK) protection", and
/// `dds.sec.access.rtps_psk_protection_kind` (`NONE`, `SIGN`, or `ENCRYPT`,
/// the default) says how. The DomainId and DomainTag are the gate's.
///
/// # Errors
/// [`SecurityPropertyError::MissingProperty`] for a protection kind named
/// SIGN or ENCRYPT without a passphrase; [`SecurityPropertyError::PskCrypto`]
/// as [`psk_crypto_config_from_properties`] and for another kind.
/// `governance` is the `rtps_psk_protection_kind` of the domain rule.
pub fn rtps_psk_from_properties(
    props: &PropertyQosPolicy,
    domain_id: u32,
    governance: Option<zerodds_security_permissions::ProtectionKind>,
) -> Result<Option<zerodds_security_crypto::psk_rtps::PskConfig>, SecurityPropertyError> {
    use zerodds_security_crypto::psk_rtps::{
        PROP_PASSPHRASE, PROP_PASSPHRASE_TABLE_76, PskProtection,
    };
    use zerodds_security_permissions::ProtectionKind;
    use zerodds_security_permissions::psk_access::{
        PROP_RTPS_PSK_PROTECTION_KIND, RtpsPskProtection,
    };
    // §10.4.1.2.5.8: `<rtps_psk_protection_kind>` of the governance decides
    // `is_rtps_psk_protected` and `is_rtps_psk_encrypted`; the property of
    // the access control plugin (Table 84) says so where the governance names
    // none (NONE).
    let named = match governance {
        Some(ProtectionKind::Sign) => Some("SIGN"),
        Some(ProtectionKind::Encrypt) => Some("ENCRYPT"),
        _ => props.get(PROP_RTPS_PSK_PROTECTION_KIND),
    };
    let kind = match named {
        None => RtpsPskProtection::Encrypt,
        Some(value) => RtpsPskProtection::parse(value)
            .map_err(|e| SecurityPropertyError::PskCrypto(e.detail.to_string()))?,
    };
    let has_passphrase =
        props.get(PROP_PASSPHRASE).is_some() || props.get(PROP_PASSPHRASE_TABLE_76).is_some();
    if !has_passphrase {
        return if named.is_some() && kind != RtpsPskProtection::None {
            Err(SecurityPropertyError::MissingProperty(PROP_PASSPHRASE))
        } else {
            Ok(None)
        };
    }
    let protection = match kind {
        RtpsPskProtection::None => return Ok(None),
        RtpsPskProtection::Sign => PskProtection::Sign,
        RtpsPskProtection::Encrypt => PskProtection::Encrypt,
    };
    psk_crypto_config_from_properties(props, domain_id, "", protection).map(Some)
}

/// The configuration of `DDS:Crypto:PSK` out of the property list of the
/// participant (DDS-Security 1.2 §10.9.1 Table 86):
/// `dds.sec.crypto.rtps_psk_secret_passphrase` (mandatory; the spelling of
/// Table 76, `dds.sec.crypto.psk.secret_passphrase`, is read too),
/// `..._alt` (optional list) and `rtps_psk_symmetric_cipher_algorithm`
/// (optional, "AUTO" is AES256-GCM). The URIs are `file:` and `data:`.
/// `protection` is the RTPS protection kind the access control plugin gives
/// (`dds.sec.access.rtps_psk_protection_kind`).
///
/// # Errors
/// [`SecurityPropertyError::MissingProperty`] without the passphrase;
/// [`SecurityPropertyError::PskCrypto`] for a passphrase, a list or a cipher
/// name that Table 86 does not allow, or a scheme other than `file` and
/// `data`.
pub fn psk_crypto_config_from_properties(
    props: &PropertyQosPolicy,
    domain_id: u32,
    domain_tag: &str,
    protection: zerodds_security_crypto::psk_rtps::PskProtection,
) -> Result<zerodds_security_crypto::psk_rtps::PskConfig, SecurityPropertyError> {
    use zerodds_security_crypto::psk_rtps::{
        PROP_CIPHER, PROP_PASSPHRASE, PROP_PASSPHRASE_ALT, PROP_PASSPHRASE_TABLE_76, Passphrase,
        PskConfig, key_bits_from_cipher, parse_passphrase_list,
    };
    let read = |name: &'static str, value: &str| -> Result<String, SecurityPropertyError> {
        let material = resolve_material(name, value)?;
        if matches!(material, Material::Pkcs11(_)) {
            return Err(SecurityPropertyError::PskCrypto(alloc::format!(
                "{name}: the supported URI schemas are file and data"
            )));
        }
        let bytes = material.read().map_err(SecurityPropertyError::Profile)?;
        String::from_utf8(bytes)
            .map_err(|_| SecurityPropertyError::PskCrypto(alloc::format!("{name}: not text")))
    };
    let (name, value) = match props.get(PROP_PASSPHRASE) {
        Some(value) => (PROP_PASSPHRASE, value),
        None => match props.get(PROP_PASSPHRASE_TABLE_76) {
            Some(value) => (PROP_PASSPHRASE_TABLE_76, value),
            None => return Err(SecurityPropertyError::MissingProperty(PROP_PASSPHRASE)),
        },
    };
    let text = read(name, value)?;
    let passphrase = Passphrase::parse(text.trim_end_matches(['\r', '\n']))
        .map_err(|e| SecurityPropertyError::PskCrypto(alloc::format!("{name}: {}", e.detail)))?;
    let alternatives = match props.get(PROP_PASSPHRASE_ALT) {
        Some(value) => parse_passphrase_list(&read(PROP_PASSPHRASE_ALT, value)?).map_err(|e| {
            SecurityPropertyError::PskCrypto(alloc::format!("{PROP_PASSPHRASE_ALT}: {}", e.detail))
        })?,
        None => Vec::new(),
    };
    let key_bits = key_bits_from_cipher(props.get(PROP_CIPHER))
        .map_err(|e| SecurityPropertyError::PskCrypto(e.detail.to_string()))?;
    Ok(PskConfig {
        domain_id,
        domain_tag: domain_tag.to_string(),
        passphrase,
        alternatives,
        key_bits,
        protection,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// The names are the spec's, written out as literals so that
    /// renaming a constant cannot quietly rename what a peer's
    /// access-control plugin looks for. DDS-Security 1.2 §10.3.1 Table 50
    /// and §10.4.1 Table 63.
    #[test]
    fn the_property_names_are_the_spec_strings() {
        assert_eq!(PROP_IDENTITY_CA, "dds.sec.auth.identity_ca");
        assert_eq!(PROP_IDENTITY_CERT, "dds.sec.auth.identity_certificate");
        assert_eq!(PROP_IDENTITY_KEY, "dds.sec.auth.private_key");
        assert_eq!(PROP_IDENTITY_KEY_PASSWORD, "dds.sec.auth.password");
        assert_eq!(
            PROP_KEY_ESTABLISHMENT,
            "dds.sec.auth.key_establishment_algorithm"
        );
        assert_eq!(PROP_PERMISSIONS_CA, "dds.sec.access.permissions_ca");
        assert_eq!(PROP_GOVERNANCE, "dds.sec.access.governance");
        assert_eq!(PROP_PERMISSIONS, "dds.sec.access.permissions");
    }

    /// Tables 50/63: an `auth`/`access` property is what asks for
    /// security. `dds.sec.log.*` alone does not — a logger is not a
    /// security configuration, and treating it as one would refuse every
    /// plaintext participant that wants security-event observability.
    #[test]
    fn the_auth_and_access_namespaces_request_security_and_the_log_one_does_not() {
        assert!(!requests_security(&PropertyQosPolicy::new()));
        assert!(!requests_security(
            &PropertyQosPolicy::new().with("dds.sec.log.plugin", "stderr")
        ));
        assert!(requests_security(
            &PropertyQosPolicy::new().with(PROP_IDENTITY_CA, "ca.pem")
        ));
        assert!(requests_security(
            &PropertyQosPolicy::new().with(PROP_GOVERNANCE, "gov.p7s")
        ));
    }

    /// All three `file:` spellings, plus the plain path.
    #[test]
    fn every_file_uri_form_resolves_to_a_path() {
        assert_eq!(
            resolve_file_uri(PROP_GOVERNANCE, "file:../TESTONLY_governance_signed.p7s").unwrap(),
            PathBuf::from("../TESTONLY_governance_signed.p7s")
        );
        assert_eq!(
            resolve_file_uri(PROP_IDENTITY_CA, "file:///etc/dds/ca.pem").unwrap(),
            PathBuf::from("/etc/dds/ca.pem")
        );
        assert_eq!(
            resolve_file_uri(PROP_IDENTITY_CA, "file://etc/dds/ca.pem").unwrap(),
            PathBuf::from("etc/dds/ca.pem")
        );
        assert_eq!(
            resolve_file_uri(PROP_IDENTITY_CA, "../plain/path.pem").unwrap(),
            PathBuf::from("../plain/path.pem")
        );
        assert_eq!(
            strip_file_url("file:../TESTONLY_governance_signed.p7s"),
            "file:../TESTONLY_governance_signed.p7s",
            "the shared helper does not handle the authority-less form — that is \
             why resolve_file_uri exists"
        );
    }

    /// `data:` and `pkcs11:` are not paths: [`resolve_file_uri`] names
    /// them rather than opening a relative path that will never exist.
    /// [`resolve_material`] then reads `data:,` as content and parses a
    /// `pkcs11:` URI (RFC 7512); without the `pkcs11` feature it refuses
    /// naming the feature.
    #[test]
    fn the_uri_schemes_that_are_not_paths_are_named_not_opened_as_paths() {
        for scheme in ["pkcs11", "data"] {
            let value = if scheme == "pkcs11" {
                "pkcs11:token=hsm;object=key"
            } else {
                "data:,inline-pem"
            };
            let err =
                resolve_file_uri(PROP_IDENTITY_KEY, value).expect_err("neither scheme is a path");
            match &err {
                SecurityPropertyError::UnsupportedUriScheme {
                    property,
                    scheme: got,
                } => {
                    assert_eq!(*property, PROP_IDENTITY_KEY);
                    assert_eq!(got, scheme);
                }
                other => panic!("expected an unsupported scheme, got {other:?}"),
            }
            assert!(err.to_string().contains(scheme));
        }

        #[cfg(feature = "pkcs11")]
        {
            let material = resolve_material(PROP_IDENTITY_KEY, "pkcs11:token=hsm;object=key")
                .expect("a pkcs11: URI parses with the feature on");
            match material {
                Material::Pkcs11(uri) => {
                    assert_eq!(uri.token.as_deref(), Some("hsm"));
                    assert_eq!(uri.object.as_deref(), Some("key"));
                }
                other => panic!("expected a token URI, got {other:?}"),
            }
        }
        #[cfg(not(feature = "pkcs11"))]
        {
            let err = resolve_material(PROP_IDENTITY_KEY, "pkcs11:token=hsm;object=key")
                .expect_err("without the feature a pkcs11: URI is refused");
            assert!(matches!(
                err,
                SecurityPropertyError::Pkcs11FeatureDisabled {
                    property: PROP_IDENTITY_KEY
                }
            ));
            assert!(err.to_string().contains("pkcs11"), "{err}");
        }
    }

    /// Every one of the six is named when it is missing, in the order
    /// the profile needs them.
    #[test]
    fn each_missing_property_is_reported_by_its_own_name() {
        let full = complete_property_list();
        for expected in [
            PROP_IDENTITY_CA,
            PROP_IDENTITY_CERT,
            PROP_IDENTITY_KEY,
            PROP_PERMISSIONS_CA,
            PROP_GOVERNANCE,
            PROP_PERMISSIONS,
        ] {
            let mut without = PropertyQosPolicy::new();
            for (name, value) in full.iter() {
                if name != expected {
                    without.set(name, value);
                }
            }
            match profile_config_from_properties(&without, 0) {
                Err(SecurityPropertyError::MissingProperty(name)) => assert_eq!(name, expected),
                other => panic!("dropping {expected} must be reported by name, got {other:?}"),
            }
        }
    }

    /// The message names the property, so an operator reading the
    /// participant-create failure knows which line of their config is
    /// wrong.
    #[test]
    fn the_missing_property_message_carries_the_property_name() {
        let text = SecurityPropertyError::MissingProperty(PROP_PERMISSIONS).to_string();
        assert!(text.contains("dds.sec.access.permissions"), "{text}");
        assert!(text.contains("§10.4.1"), "{text}");
    }

    /// §10.3.1 Table 50, example `file:identity_ca_private_key.pem?password=OpenSesame`:
    /// the query is the password and not part of the path; the `password`
    /// property wins when both are given; a `data:` value keeps its `?`.
    #[test]
    fn a_password_in_the_key_uri_is_not_part_of_the_path() {
        let (material, password) =
            resolve_private_key(PROP_IDENTITY_KEY, "file:key.pem?password=Open%20Sesame").unwrap();
        assert!(matches!(&material, Material::File(p) if p == std::path::Path::new("key.pem")));
        assert_eq!(password.as_deref(), Some("Open Sesame"));
        let (material, password) = resolve_private_key(PROP_IDENTITY_KEY, "file:key.pem").unwrap();
        assert!(matches!(&material, Material::File(p) if p == std::path::Path::new("key.pem")));
        assert_eq!(password, None);
        let (material, password) =
            resolve_private_key(PROP_IDENTITY_KEY, "data:,-----BEGIN-----?password=x").unwrap();
        assert!(matches!(&material, Material::Inline(b) if b.ends_with(b"?password=x")));
        assert_eq!(password, None);
        assert!(resolve_private_key(PROP_IDENTITY_KEY, "file:key.pem?password=%zz").is_err());

        let props =
            complete_property_list().with(PROP_IDENTITY_KEY, "file:key.pem?password=fromuri");
        assert_eq!(
            profile_config_from_properties(&props, 0)
                .unwrap()
                .identity_key_password
                .as_deref(),
            Some("fromuri")
        );
        let props = props.with(PROP_IDENTITY_KEY_PASSWORD, "fromproperty");
        assert_eq!(
            profile_config_from_properties(&props, 0)
                .unwrap()
                .identity_key_password
                .as_deref(),
            Some("fromproperty")
        );
    }

    /// An OCSP responder that cannot be reached is refused unless
    /// `org.zerodds.sec.auth.ocsp_unreachable` says accept; a value nobody can
    /// read is an error and does not switch the check off.
    #[test]
    fn the_ocsp_policy_refuses_by_default_and_reads_its_settings() {
        use zerodds_security_pki::OcspUnreachable;
        let none = PropertyQosPolicy::new();
        let policy = ocsp_policy_from_properties(&none).unwrap();
        assert_eq!(policy.unreachable, OcspUnreachable::Fail);
        assert_eq!(policy.timeout, core::time::Duration::from_millis(2000));
        let props = PropertyQosPolicy::new()
            .with(PROP_OCSP_UNREACHABLE, "accept")
            .with(PROP_OCSP_TIMEOUT_MS, "250");
        let policy = ocsp_policy_from_properties(&props).unwrap();
        assert_eq!(policy.unreachable, OcspUnreachable::Accept);
        assert_eq!(policy.timeout, core::time::Duration::from_millis(250));
        for (name, value) in [
            (PROP_OCSP_UNREACHABLE, "pass"),
            (PROP_OCSP_UNREACHABLE, ""),
            (PROP_OCSP_TIMEOUT_MS, "0"),
            (PROP_OCSP_TIMEOUT_MS, "soon"),
        ] {
            let props = PropertyQosPolicy::new().with(name, value);
            assert!(
                matches!(
                    ocsp_policy_from_properties(&props),
                    Err(SecurityPropertyError::InvalidOcspSetting { .. })
                ),
                "{name}={value}"
            );
        }
    }

    /// Table 86: the passphrase property is mandatory, `data:` and `file:`
    /// give `id:passphrase` (the example `data:,5612:Open Sesame`), the
    /// alternatives are a list, the cipher name or AUTO gives the key width.
    /// §10.4.1.2.5.8: the `rtps_psk_protection_kind` of the governance decides
    /// whether and how the RTPS messages are protected with the pre-shared
    /// key; the property of Table 84 only where the governance names NONE.
    #[test]
    fn the_governance_decides_the_pre_shared_key_protection() {
        use zerodds_security_crypto::psk_rtps::PskProtection;
        use zerodds_security_permissions::ProtectionKind as K;
        let props = PropertyQosPolicy::new().with(
            "dds.sec.crypto.rtps_psk_secret_passphrase",
            "data:,5612:Open Sesame",
        );
        let kind = |props: &PropertyQosPolicy, gov| {
            rtps_psk_from_properties(props, 0, gov)
                .unwrap()
                .map(|c| c.protection)
        };
        assert_eq!(kind(&props, Some(K::Sign)), Some(PskProtection::Sign));
        assert_eq!(kind(&props, Some(K::Encrypt)), Some(PskProtection::Encrypt));
        // The governance wins over the property.
        let sign = props
            .clone()
            .with("dds.sec.access.rtps_psk_protection_kind", "SIGN");
        assert_eq!(kind(&sign, Some(K::Encrypt)), Some(PskProtection::Encrypt));
        // NONE in the governance: the property decides.
        assert_eq!(kind(&sign, Some(K::None)), Some(PskProtection::Sign));
        assert_eq!(kind(&props, None), Some(PskProtection::Encrypt));
        // A protection kind of the governance without a passphrase is an error
        // only where the property names it; without either there is no key.
        let none = PropertyQosPolicy::new();
        assert!(kind(&none, Some(K::None)).is_none());
    }

    #[test]
    fn the_psk_crypto_properties_of_table_86_are_read() {
        use zerodds_security_crypto::psk_rtps::{
            PROP_CIPHER, PROP_PASSPHRASE, PROP_PASSPHRASE_ALT, PskProtection,
        };
        let config = |props: &PropertyQosPolicy| {
            psk_crypto_config_from_properties(props, 201, "", PskProtection::Encrypt)
        };
        assert!(matches!(
            config(&PropertyQosPolicy::new()),
            Err(SecurityPropertyError::MissingProperty(_))
        ));
        let props = PropertyQosPolicy::new()
            .with(PROP_PASSPHRASE, "data:,5612:Open Sesame")
            .with(PROP_PASSPHRASE_ALT, "data:,5613:Extra\n5614:Another\r\n");
        let cfg = config(&props).unwrap();
        assert_eq!(
            (cfg.passphrase.id, cfg.passphrase.secret.as_str()),
            (5612, "Open Sesame")
        );
        assert_eq!(
            cfg.alternatives.iter().map(|p| p.id).collect::<Vec<_>>(),
            vec![5613, 5614]
        );
        assert_eq!(cfg.key_bits, 256, "AUTO");
        let props = props.with(PROP_CIPHER, "AES128-GCM");
        assert_eq!(config(&props).unwrap().key_bits, 128);
        for (name, value) in [
            (PROP_PASSPHRASE, "data:,255:reserved"),
            (PROP_PASSPHRASE, "data:,12: spaced"),
            (PROP_PASSPHRASE, "pkcs11:object=x"),
            (PROP_PASSPHRASE_ALT, "data:,1:ok\n255:no"),
            (PROP_CIPHER, "ROT13"),
        ] {
            let props = PropertyQosPolicy::new()
                .with(PROP_PASSPHRASE, "data:,5612:Open Sesame")
                .with(name, value);
            assert!(config(&props).is_err(), "{name}={value}");
        }
        // The spelling of Table 76.
        let t76 = PropertyQosPolicy::new()
            .with("dds.sec.crypto.psk.secret_passphrase", "data:,5632:castle");
        assert_eq!(config(&t76).unwrap().passphrase.id, 5632);
    }

    /// The password is the clause's AES-128 key unless
    /// `org.zerodds.sec.auth.password_reading` names the vendors' reading.
    #[test]
    fn the_password_reading_is_the_clause_unless_named() {
        use zerodds_security_pki::encrypted_key::PasswordReading;
        let reading = |value: Option<&str>| {
            let mut props = complete_property_list();
            if let Some(value) = value {
                props = props.with(PROP_PASSWORD_READING, value);
            }
            profile_config_from_properties(&props, 0).map(|c| c.identity_key_password_reading)
        };
        assert_eq!(reading(None).unwrap(), PasswordReading::Clause);
        assert_eq!(reading(Some("spec")).unwrap(), PasswordReading::Clause);
        assert_eq!(
            reading(Some("openssl_passphrase")).unwrap(),
            PasswordReading::ClauseThenPassphrase
        );
        assert!(matches!(
            reading(Some("both")),
            Err(SecurityPropertyError::InvalidPasswordReading(_))
        ));
    }

    /// §10.3.1 Table 50 `dds.sec.auth.password` reaches the profile
    /// config, which opens the key with it; an empty password is the
    /// absent one.
    #[test]
    fn a_key_password_reaches_the_profile_config() {
        let props = complete_property_list().with(PROP_IDENTITY_KEY_PASSWORD, "hunter2");
        assert_eq!(
            profile_config_from_properties(&props, 0)
                .expect("complete list")
                .identity_key_password
                .as_deref(),
            Some("hunter2")
        );
        let props = complete_property_list().with(PROP_IDENTITY_KEY_PASSWORD, "");
        assert_eq!(
            profile_config_from_properties(&props, 0)
                .expect("complete list")
                .identity_key_password,
            None
        );
    }

    /// DDS-Security 1.2 §10.5.3.3.2 Table 81: `max_blocks_per_session` is
    /// configurable; `org.zerodds.sec.crypto.max_blocks_per_session` reaches the
    /// profile config, and a value that is not a positive whole number is
    /// refused, not taken as absent.
    #[test]
    fn max_blocks_per_session_reaches_the_profile_config() {
        let props = complete_property_list().with(PROP_MAX_BLOCKS_PER_SESSION, "4096");
        assert_eq!(
            profile_config_from_properties(&props, 0)
                .expect("complete list")
                .max_blocks_per_session,
            Some(4096)
        );
        assert_eq!(
            profile_config_from_properties(&complete_property_list(), 0)
                .expect("complete list")
                .max_blocks_per_session,
            None
        );
        for bad in ["0", "-3", "many"] {
            let props = complete_property_list().with(PROP_MAX_BLOCKS_PER_SESSION, bad);
            assert!(
                matches!(
                    profile_config_from_properties(&props, 0),
                    Err(SecurityPropertyError::InvalidMaxBlocksPerSession(_))
                ),
                "{bad}"
            );
        }
    }

    /// DDS-Security 1.2 §10.5.1 Table 71: `symmetric_cipher_algorithm` is
    /// `AUTO` or one of the names of Table 22; absent or `AUTO` is
    /// `AES256-GCM`; any other value is refused, not taken as absent.
    #[test]
    fn symmetric_cipher_algorithm_reaches_the_profile_config() {
        let bits = |value: Option<&str>| {
            let props = match value {
                Some(value) => complete_property_list().with(PROP_SYMMETRIC_CIPHER, value),
                None => complete_property_list(),
            };
            profile_config_from_properties(&props, 0).map(|cfg| cfg.symmetric_cipher_bits)
        };
        assert_eq!(bits(None).unwrap(), 256);
        assert_eq!(bits(Some("AUTO")).unwrap(), 256);
        for name in ["AES256-GCM", "AES256+GCM", "AES256-GMAC"] {
            assert_eq!(bits(Some(name)).unwrap(), 256, "{name}");
        }
        for name in ["AES128+GCM", "AES128-GCM", "AES128-GMAC"] {
            assert_eq!(bits(Some(name)).unwrap(), 128, "{name}");
        }
        for bad in ["aes128-gcm", "AES192-GCM", "ChaCha20", ""] {
            assert!(
                matches!(
                    bits(Some(bad)),
                    Err(SecurityPropertyError::InvalidSymmetricCipher(_))
                ),
                "{bad}"
            );
        }
    }

    /// Participants and endpoints of an incompatible configuration are kept
    /// apart (DDS-Security 1.2 §7.3.10 to §7.3.24) unless the named setting
    /// `org.zerodds.sec.compatibility_check=ignore` says otherwise; another value
    /// is refused.
    #[test]
    fn the_compatibility_check_is_on_unless_ignore_is_named() {
        let on = |value: Option<&str>| {
            let props = match value {
                Some(value) => complete_property_list().with(PROP_COMPATIBILITY_CHECK, value),
                None => complete_property_list(),
            };
            profile_config_from_properties(&props, 0).map(|cfg| cfg.compatibility_checks)
        };
        assert!(on(None).unwrap());
        assert!(on(Some("spec")).unwrap());
        assert!(!on(Some("ignore")).unwrap());
        assert!(matches!(
            on(Some("off")),
            Err(SecurityPropertyError::InvalidCompatibilityCheck(_))
        ));
    }

    /// A key without a receiver-specific key is refused under origin
    /// authentication (DDS-Security 1.2 §10.5.3.3 Table 80) unless
    /// `org.zerodds.sec.origin_authentication_fallback=accept` says otherwise;
    /// the property wins over `ZERODDS_SEC_ORIGIN_AUTHENTICATION_FALLBACK`.
    #[test]
    fn the_origin_authentication_fallback_comes_from_the_property_or_the_environment() {
        let fallback = |value: Option<&str>, env: Option<&'static str>| {
            let props = match value {
                Some(value) => {
                    complete_property_list().with(PROP_ORIGIN_AUTHENTICATION_FALLBACK, value)
                }
                None => complete_property_list(),
            };
            profile_config_from_properties_with_env(&props, 0, &move |name| {
                (name == ENV_ORIGIN_AUTHENTICATION_FALLBACK)
                    .then(|| env.map(ToString::to_string))
                    .flatten()
            })
            .map(|cfg| cfg.origin_authentication_fallback)
        };
        assert!(!fallback(None, None).unwrap());
        assert!(!fallback(Some("spec"), None).unwrap());
        assert!(fallback(Some("accept"), None).unwrap());
        assert!(fallback(None, Some("accept")).unwrap());
        assert!(!fallback(Some("spec"), Some("accept")).unwrap());
        assert!(matches!(
            fallback(Some("yes"), None),
            Err(SecurityPropertyError::InvalidOriginAuthenticationFallback(
                _
            ))
        ));
        let env_only = |value: &'static str| {
            vendor_form_settings_from_env(&move |name| {
                (name == ENV_ORIGIN_AUTHENTICATION_FALLBACK).then(|| value.to_string())
            })
        };
        assert!(env_only("accept").unwrap().origin_authentication_fallback);
        assert!(!env_only("spec").unwrap().origin_authentication_fallback);
        assert!(env_only("yes").is_err());
    }

    /// The Kx key opens only the volatile channel (DDS-Security 1.2
    /// §10.5.2.1.2) unless `org.zerodds.sec.kx_key_fallback=accept` says otherwise.
    /// A profile without a property list (the SROS2 enclave directory) gets
    /// the two vendor-form settings from the environment alone: it ignored
    /// the variables, so `ZERODDS_SEC_KX_KEY_FALLBACK=accept` had no effect
    /// on it.
    #[test]
    fn the_vendor_form_settings_come_from_the_environment_alone() {
        use zerodds_security::AlgoNameNul;
        let with =
            |kx: Option<&'static str>, compat: Option<&'static str>, nul: Option<&'static str>| {
                vendor_form_settings_from_env(&move |name| match name {
                    ENV_KX_KEY_FALLBACK => kx.map(ToString::to_string),
                    ENV_COMPATIBILITY_CHECK => compat.map(ToString::to_string),
                    ENV_ALGO_NAME_NUL => nul.map(ToString::to_string),
                    _ => None,
                })
            };
        let forms = |kx, compat, nul| VendorFormSettings {
            kx_key_fallback: kx,
            compatibility_checks: compat,
            algo_name_nul: nul,
            algorithm_names: zerodds_security_pki::AlgorithmNames::V1_2,
            subject_name_form: zerodds_security::SubjectNameForm::Rfc4514,
            origin_authentication_fallback: false,
        };
        assert_eq!(
            with(None, None, None).unwrap(),
            forms(false, true, AlgoNameNul::Spec)
        );
        assert_eq!(
            with(Some("accept"), None, None).unwrap(),
            forms(true, true, AlgoNameNul::Spec)
        );
        assert_eq!(
            with(None, Some("ignore"), None).unwrap(),
            forms(false, false, AlgoNameNul::Spec)
        );
        assert_eq!(
            with(Some("spec"), Some("spec"), Some("omit")).unwrap(),
            forms(false, true, AlgoNameNul::Omit)
        );
        assert_eq!(
            with(None, None, Some("by_vendor")).unwrap().algo_name_nul,
            AlgoNameNul::ByVendor
        );
        assert!(with(Some("yes"), None, None).is_err());
        assert!(with(None, Some("off"), None).is_err());
        assert!(with(None, None, Some("nul")).is_err());
    }

    /// `ZERODDS_SEC_ALGORITHM_NAMES` is part of the environment settings
    /// (enclave directory, six-path configuration); the property list reads
    /// `org.zerodds.sec.subject_name_form` / `ZERODDS_SEC_SUBJECT_NAME_FORM`:
    /// the property wins, then the variable, RFC 4514 without either; any
    /// other value is refused.
    #[test]
    fn the_subject_name_form_comes_from_the_property_or_the_environment() {
        use zerodds_security::SubjectNameForm as F;
        let none = |_: &str| None;
        let from_env =
            |name: &str| (name == ENV_SUBJECT_NAME_FORM).then(|| "xn_flag_oneline".to_string());
        let empty = PropertyQosPolicy::new();
        assert_eq!(
            subject_name_form_from_properties(&empty, &none).unwrap(),
            F::Rfc4514
        );
        assert_eq!(
            subject_name_form_from_properties(&empty, &from_env).unwrap(),
            F::XnFlagOneline
        );
        let props = PropertyQosPolicy::new().with(PROP_SUBJECT_NAME_FORM, "x509_name_oneline");
        assert_eq!(
            subject_name_form_from_properties(&props, &from_env).unwrap(),
            F::X509NameOneline
        );
        let bad = PropertyQosPolicy::new().with(PROP_SUBJECT_NAME_FORM, "oneline");
        assert!(matches!(
            subject_name_form_from_properties(&bad, &none),
            Err(SecurityPropertyError::InvalidSubjectNameForm(_))
        ));
    }

    /// the property and then the variable; the default is `1.2`.
    #[test]
    fn the_algorithm_names_come_from_the_property_or_the_environment() {
        use zerodds_security_pki::AlgorithmNames as N;
        let env = |value: Option<&'static str>| {
            vendor_form_settings_from_env(&move |name| {
                (name == ENV_ALGORITHM_NAMES)
                    .then(|| value.map(ToString::to_string))
                    .flatten()
            })
        };
        assert_eq!(env(None).unwrap().algorithm_names, N::V1_2);
        assert_eq!(env(Some("1.1")).unwrap().algorithm_names, N::V1_1);
        assert_eq!(env(Some("auto")).unwrap().algorithm_names, N::Auto);
        assert!(env(Some("1.0")).is_err());
        let props = PropertyQosPolicy::new().with(PROP_ALGORITHM_NAMES, "1.1");
        let from_env = |name: &str| (name == ENV_ALGORITHM_NAMES).then(|| "auto".to_string());
        assert_eq!(
            algorithm_names_from_properties(&props, &from_env).unwrap(),
            N::V1_1,
            "the property wins"
        );
        assert_eq!(
            algorithm_names_from_properties(&PropertyQosPolicy::new(), &from_env).unwrap(),
            N::Auto
        );
    }

    /// The form of the algorithm names in a handshake token is the spec form
    /// (with the NUL, §7.3.3.1) unless `org.zerodds.sec.algo_name_nul` names
    /// `omit` or `by_vendor`; another value is refused.
    #[test]
    fn the_algo_name_form_is_the_spec_form_unless_named() {
        use zerodds_security::AlgoNameNul;
        let form = |value: Option<&str>| {
            let props = match value {
                Some(value) => complete_property_list().with(PROP_ALGO_NAME_NUL, value),
                None => complete_property_list(),
            };
            profile_config_from_properties(&props, 0).map(|cfg| cfg.algo_name_nul)
        };
        assert_eq!(form(None).unwrap(), AlgoNameNul::Spec);
        assert_eq!(form(Some("spec")).unwrap(), AlgoNameNul::Spec);
        assert_eq!(form(Some("omit")).unwrap(), AlgoNameNul::Omit);
        assert_eq!(form(Some("mirror")).unwrap(), AlgoNameNul::Mirror);
        assert_eq!(form(Some("by_vendor")).unwrap(), AlgoNameNul::ByVendor);
        assert!(matches!(
            form(Some("nul")),
            Err(SecurityPropertyError::InvalidAlgoNameNul(_))
        ));
    }

    #[test]
    fn the_kx_key_fallback_is_off_unless_accept_is_named() {
        let on = |value: Option<&str>| {
            let props = match value {
                Some(value) => complete_property_list().with(PROP_KX_KEY_FALLBACK, value),
                None => complete_property_list(),
            };
            profile_config_from_properties(&props, 0).map(|cfg| cfg.kx_key_fallback)
        };
        assert!(!on(None).unwrap());
        assert!(!on(Some("spec")).unwrap());
        assert!(on(Some("accept")).unwrap());
        assert!(matches!(
            on(Some("yes")),
            Err(SecurityPropertyError::InvalidKxKeyFallback(_))
        ));
    }

    /// The two vendor-form settings take an environment variable when the list
    /// has no property; the property wins.
    #[test]
    fn the_vendor_form_settings_take_an_environment_variable() {
        let with_env = |props: &PropertyQosPolicy, name: &'static str, value: &'static str| {
            profile_config_from_properties_with_env(props, 0, &move |n| {
                (n == name).then(|| value.to_string())
            })
        };
        let list = complete_property_list();
        assert!(!with_env(&list, "OTHER", "accept").unwrap().kx_key_fallback);
        assert!(
            with_env(&list, ENV_KX_KEY_FALLBACK, "accept")
                .unwrap()
                .kx_key_fallback
        );
        assert!(
            !with_env(&list, ENV_COMPATIBILITY_CHECK, "ignore")
                .unwrap()
                .compatibility_checks
        );
        // The property wins over the variable.
        let spec = list.clone().with(PROP_KX_KEY_FALLBACK, "spec");
        assert!(
            !with_env(&spec, ENV_KX_KEY_FALLBACK, "accept")
                .unwrap()
                .kx_key_fallback
        );
        // A value that is no spelling is refused, from the variable too.
        assert!(matches!(
            with_env(&list, ENV_KX_KEY_FALLBACK, "yes"),
            Err(SecurityPropertyError::InvalidKxKeyFallback(_))
        ));
    }

    /// The config follows the list, not a second source: set a property
    /// and the path follows it, `file:` and all.
    #[test]
    fn the_profile_config_follows_the_property_list() {
        let props = complete_property_list()
            .with(PROP_IDENTITY_CERT, "file:../certs/id_cert.pem")
            .with(PROP_GOVERNANCE, "file:../TESTONLY_governance_signed.p7s");
        let cfg = profile_config_from_properties(&props, 7).expect("complete list");
        assert_eq!(cfg.domain_id, 7);
        assert_eq!(
            cfg.identity_cert_pem,
            Material::File(PathBuf::from("../certs/id_cert.pem"))
        );
        assert_eq!(
            cfg.governance_p7s,
            Material::File(PathBuf::from("../TESTONLY_governance_signed.p7s"))
        );
    }

    /// DDS-Security 1.2 §9.3.1 Table 44: `data:,` carries the content
    /// itself. Taken after the comma unchanged — newlines included — and
    /// with leading blanks skipped, as Cyclone DDS's
    /// `DDS_Security_get_conf_item_type` takes it.
    #[test]
    fn a_data_uri_is_the_content_and_not_a_path() {
        let pem = "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n";
        let props = complete_property_list()
            .with(PROP_IDENTITY_CA, format!("data:,{pem}"))
            .with(PROP_PERMISSIONS_CA, format!("  data:,{pem}"));
        let cfg = profile_config_from_properties(&props, 0).expect("complete list");
        assert_eq!(
            cfg.identity_ca_pem,
            Material::Inline(pem.as_bytes().to_vec())
        );
        assert_eq!(
            cfg.permissions_ca_pem,
            Material::Inline(pem.as_bytes().to_vec())
        );
        assert_eq!(cfg.identity_ca_pem.read().expect("inline"), pem.as_bytes());
    }

    /// The other two RFC 2397 forms Table 50 admits by reference: a
    /// base64 payload and a percent-encoded one.
    #[test]
    fn a_data_uri_with_a_media_type_decodes_by_rfc_2397() {
        assert_eq!(
            resolve_material(
                PROP_IDENTITY_CA,
                "data:application/x-pem-file;base64,TUlJQg=="
            )
            .expect("base64"),
            Material::Inline(b"MIIB".to_vec())
        );
        assert_eq!(
            resolve_material(PROP_IDENTITY_CA, "data:text/plain,a%20b%0A").expect("percent"),
            Material::Inline(b"a b\n".to_vec())
        );
    }

    /// What does not decode is refused naming the property, not opened
    /// as a path called `data:…`.
    #[test]
    fn a_data_uri_that_does_not_decode_is_refused() {
        for value in [
            "data:text/plain;base64,not base64!",
            "data:text/plain,50%",
            "data:no-comma-at-all",
        ] {
            let err = resolve_material(PROP_IDENTITY_CA, value).expect_err(value);
            assert!(
                matches!(err, SecurityPropertyError::MalformedDataUri { property, .. } if property == PROP_IDENTITY_CA),
                "{value}: {err}"
            );
        }
    }

    /// §8.3 Table 26 spelling, our measured wire spelling, the legacy
    /// in-tree one, the absent default — and a refusal for anything else.
    #[test]
    fn the_key_establishment_property_selects_a_suite_and_refuses_the_unknown() {
        let with = |v: &str| PropertyQosPolicy::new().with(PROP_KEY_ESTABLISHMENT, v);
        assert_eq!(
            kx_suite_from_properties(&PropertyQosPolicy::new()).unwrap(),
            KxSuite::EcdhP256,
            "an absent property is Table 26's default"
        );
        assert_eq!(
            kx_suite_from_properties(&with("ECDHE-CEUM+P256")).unwrap(),
            KxSuite::EcdhP256
        );
        assert_eq!(
            kx_suite_from_properties(&with("ECDH+prime256v1-CEUM")).unwrap(),
            KxSuite::EcdhP256
        );
        assert_eq!(
            kx_suite_from_properties(&with("ECDHE-CEUM+P384")).unwrap(),
            KxSuite::EcdhP384
        );
        assert_eq!(
            kx_suite_from_properties(&with("X25519")).unwrap(),
            KxSuite::X25519
        );
        // "AUTO" leaves the choice to the plugin (Table 50): P-256, which every
        // vendor runs.
        assert_eq!(
            kx_suite_from_properties(&with("AUTO")).unwrap(),
            KxSuite::EcdhP256
        );
        // DHE+MODP-2048-256 (Table 26), in its name of 1.2 and of 1.1.
        for name in ["DHE+MODP-2048-256", "DH+MODP-2048-256"] {
            assert_eq!(
                kx_suite_from_properties(&with(name)).unwrap(),
                KxSuite::DhModp2048_256
            );
        }
        assert!(kx_suite_from_properties(&with("ECDHE-CEUM+P521")).is_err());
    }

    /// §10.3.1.3 is the default; the deviation is a property on the
    /// announced QoS, not a value kept beside the list.
    #[test]
    fn the_x509v1_deviation_comes_out_of_the_property_list() {
        let props = complete_property_list();
        assert!(!cert_policy_from_properties(&props).accept_x509v1_identity_certificate);
        let props = props.with(zerodds_security_pki::PROP_ACCEPT_X509V1_IDENTITY, "true");
        assert!(cert_policy_from_properties(&props).accept_x509v1_identity_certificate);
    }

    /// A complete list pointing at files that do not exist fails on the
    /// I/O, naming the path — it does not come back as a profile-less
    /// success.
    #[test]
    fn an_unreadable_path_fails_the_build_rather_than_returning_no_profile() {
        let props = PropertyQosPolicy::new()
            .with(PROP_IDENTITY_CA, "file:/zerodds/_absent_ca.pem")
            .with(PROP_IDENTITY_CERT, "file:/zerodds/_absent_cert.pem")
            .with(PROP_IDENTITY_KEY, "file:/zerodds/_absent_key.pem")
            .with(PROP_PERMISSIONS_CA, "file:/zerodds/_absent_pca.pem")
            .with(PROP_GOVERNANCE, "file:/zerodds/_absent_gov.p7s")
            .with(PROP_PERMISSIONS, "file:/zerodds/_absent_perm.p7s");
        let text = match security_bundle_from_properties(&props, 0, [0u8; 16], None) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("the files do not exist — a bundle must not come back"),
        };
        assert!(text.contains("_absent_"), "{text}");
    }

    /// The six properties with values that parse as paths — the shape a
    /// caller has to supply, without any files behind it.
    fn complete_property_list() -> PropertyQosPolicy {
        PropertyQosPolicy::new()
            .with(PROP_IDENTITY_CA, "ca.pem")
            .with(PROP_IDENTITY_CERT, "cert.pem")
            .with(PROP_IDENTITY_KEY, "key.pem")
            .with(PROP_PERMISSIONS_CA, "pca.pem")
            .with(PROP_GOVERNANCE, "gov.p7s")
            .with(PROP_PERMISSIONS, "perm.p7s")
    }
}
