// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds-lint: allow no_dyn_in_safe
// Rationale: the profile builder emits the config-driven chosen
// DDS-Security plugins as `dyn ...Plugin` — inherent runtime
// polymorphism of the plugin pattern, not replaceable by concrete generics.
//! Vendor-style-conform "from-paths" builder for DDS-Security 1.2 setups.
//!
//! Bundles the building blocks:
//! - [`zerodds_security_pki::PkiAuthenticationPlugin`] with identity cert+CA+key
//! - [`zerodds_security_permissions::CmsPkcs7Verifier`] on the permissions CA
//! - `governance.p7s` + `permissions.p7s` verified + parsed via CMS
//! - [`zerodds_security_crypto::AesGcmCryptoPlugin`] + [`crate::SharedSecurityGate`]
//!
//! This gives FFI callers (zerodds-c-api) and bench apps a
//! vendor-equivalent API: give me 6 PEM/PKCS#7 paths, I deliver
//! a ready-to-consume [`SecurityProfile`] whose `gate` can be attached
//! to `RuntimeConfig.security` in [`crate::SharedSecurityGate`] form.

// `cfg(feature = "std")` already lives on the `mod profile` statement in lib.rs;
// do not attribute it additionally here (clippy::duplicated_attributes).

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use zerodds_security::authentication::{
    AuthenticationPlugin, SharedSecretHandle, SharedSecretProvider, ValidationResult,
};
use zerodds_security::error::SecurityError;
use zerodds_security::properties::{Property, PropertyList};
use zerodds_security_crypto::{AesGcmCryptoPlugin, Suite};
use zerodds_security_permissions::{
    CmsPkcs7Verifier, Governance, Permissions, PermissionsError, XmlSignatureVerifier,
    open_signed_permissions, parse_governance_xml,
};
use zerodds_security_pki::{
    CertPolicy, IdentityConfig, IdentityHandle, PkiAuthenticationPlugin, PkiError,
};

use crate::SharedSecurityGate;
use crate::shared::PARTICIPANT_WIDE_TOPIC;

/// Shares the `Arc<Mutex<PkiAuthenticationPlugin>>` as a
/// [`SharedSecretProvider`] with the crypto plugin. The crypto plugin
/// calls `get_shared_secret` only on `register_matched_remote_participant`
/// — the handshake driver MUST release the PKI lock beforehand (otherwise
/// a deadlock, since `get_shared_secret` takes the same mutex).
struct SharedPkiSecretProvider(Arc<Mutex<PkiAuthenticationPlugin>>);

impl SharedSecretProvider for SharedPkiSecretProvider {
    fn get_shared_secret(&self, handle: SharedSecretHandle) -> Option<Vec<u8>> {
        self.0.lock().ok()?.get_shared_secret(handle)
    }

    fn get_shared_secret_challenges(
        &self,
        handle: SharedSecretHandle,
    ) -> Option<([u8; 32], [u8; 32])> {
        // MUST be forwarded — otherwise the default `None` applies and the
        // crypto plugin falls back to the proprietary from_shared_secret_kx
        // instead of the cyclone-exact VolatileSecure key derivation (§9.5.3.5).
        self.0.lock().ok()?.get_shared_secret_challenges(handle)
    }
}

/// Error paths when building a [`SecurityProfile`] from files.
#[derive(Debug)]
pub enum SecurityProfileError {
    /// `std::fs::read` failed on `path`.
    Io {
        /// Path at which the I/O attempt failed.
        path: PathBuf,
        /// Original error.
        source: std::io::Error,
    },
    /// The PKI layer rejected the cert/key bundle (chain, algo, format).
    Pki(PkiError),
    /// The PKI plugin (or a sub-plugin underneath) rejected the
    /// handshake/identity setup path with a [`SecurityError`]
    /// — typically a wrapper around [`PkiError`].
    PkiSecurity(SecurityError),
    /// The permissions/governance layer rejected the XML/CMS.
    Permissions(PermissionsError),
    /// The verified governance XML was not valid UTF-8.
    GovernanceUtf8(core::str::Utf8Error),
    /// A `pkcs11:` token could not be read or signed with
    /// (DDS-Security 1.2 §9.3.1 Table 44, RFC 7512).
    Pkcs11(crate::pkcs11::Pkcs11Error),
    /// No grant of the permissions document names the SubjectName of the
    /// identity certificate (DDS-Security 1.2 §10.4.1.5.3.1, Table 70
    /// `validate_local_permissions`).
    NoGrantForSubject,
    /// A setting read from the environment has a value it does not know.
    Setting(String),
}

impl core::fmt::Display for SecurityProfileError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "security-profile io {}: {source}", path.display())
            }
            Self::Pki(e) => write!(f, "security-profile pki: {e}"),
            Self::PkiSecurity(e) => write!(f, "security-profile pki: {e}"),
            Self::Permissions(e) => write!(f, "security-profile permissions: {e}"),
            Self::GovernanceUtf8(e) => write!(f, "security-profile governance utf-8: {e}"),
            Self::Pkcs11(e) => write!(f, "security-profile pkcs11: {e}"),
            Self::Setting(message) => write!(f, "security-profile setting: {message}"),
            Self::NoGrantForSubject => write!(
                f,
                "security-profile permissions: no grant names the identity certificate's subject"
            ),
        }
    }
}

impl std::error::Error for SecurityProfileError {}

impl From<PkiError> for SecurityProfileError {
    fn from(e: PkiError) -> Self {
        Self::Pki(e)
    }
}
impl From<SecurityError> for SecurityProfileError {
    fn from(e: SecurityError) -> Self {
        Self::PkiSecurity(e)
    }
}
impl From<PermissionsError> for SecurityProfileError {
    fn from(e: PermissionsError) -> Self {
        Self::Permissions(e)
    }
}
impl From<crate::pkcs11::Pkcs11Error> for SecurityProfileError {
    fn from(e: crate::pkcs11::Pkcs11Error) -> Self {
        Self::Pkcs11(e)
    }
}

/// File-path-based configuration for a [`SecurityProfile`].
///
/// All paths must be readable and contain the usual
/// DDS-Security 1.2 file formats:
/// - `identity_ca_pem` — PEM bundle of the identity CA
/// - `identity_cert_pem` — PEM with the participant's identity certificate
/// - `identity_key_pem` — PKCS#8 PEM with the private key
/// - `permissions_ca_pem` — PEM bundle of the permissions CA (often = identity_ca)
/// - `governance_p7s` — CMS-signed governance XML (`.p7s`)
/// - `permissions_p7s` — CMS-signed permissions XML (`.p7s`)
#[derive(Debug, Clone)]
pub struct SecurityProfileConfig {
    /// DDS domain id (decides the match in the governance).
    pub domain_id: u32,
    /// PEM bundle of the identity CA (trust anchors for remote certs).
    pub identity_ca_pem: Material,
    /// PEM with the identity cert of the local participant.
    pub identity_cert_pem: Material,
    /// PKCS#8 PEM private key of the local participant.
    pub identity_key_pem: Material,
    /// PEM bundle of the permissions CA (signs governance/permissions).
    pub permissions_ca_pem: Material,
    /// CMS-signed governance XML.
    pub governance_p7s: Material,
    /// CMS-signed permissions XML.
    pub permissions_p7s: Material,
    /// `dds.sec.auth.password` (DDS-Security 1.2 §10.3.1 Table 50): what
    /// opens an encrypted private key. `None` for an unencrypted one.
    /// `zerodds_security_pki::encrypted_key` says how it is read.
    pub identity_key_password: Option<String>,
    /// How [`Self::identity_key_password`] is read: as the clause's AES-128
    /// key (default) or also as an OpenSSL passphrase
    /// (`org.zerodds.sec.auth.password_reading`).
    pub identity_key_password_reading: zerodds_security_pki::encrypted_key::PasswordReading,
    /// The key width of the crypto plugin, 128 or 256 bits: what
    /// `dds.sec.crypto.symmetric_cipher_algorithm` names (DDS-Security 1.2
    /// §10.5.1 Table 71); `AUTO` and a missing property are 256 (AES256-GCM).
    pub symmetric_cipher_bits: u16,
    /// `max_blocks_per_session` of the crypto plugin (DDS-Security 1.2
    /// §10.5.3.3.2 Table 81), `None` for its default.
    pub max_blocks_per_session: Option<u64>,
    /// The certificate revocation list(s) of the identity CA (DDS-Security
    /// 1.2 §10.3.3 Table 59), PEM or DER; `None` for none.
    pub identity_crl: Option<Material>,
    /// How the partitions of a permissions allow rule match (§10.4.1.5.3.4.1.4):
    /// subset, or the "legacy matching" the spec requires a way to select.
    pub partition_matching: zerodds_security_permissions::PartitionMatching,
    /// Plaintext RTPS messages over loopback are taken on an RTPS-protected
    /// domain (`org.zerodds.sec.plaintext_loopback`); `false` by default.
    pub plaintext_loopback: bool,
    /// Participants and endpoints whose algorithm infos or security masks are
    /// not compatible are not authenticated or matched, as DDS-Security 1.2
    /// §7.3.10 to §7.3.24 require (`true`, the default). `false` is the named
    /// setting `org.zerodds.sec.compatibility_check=ignore`.
    pub compatibility_checks: bool,
    /// The Kx key is also tried for submessages and payloads no key of their
    /// sender opens (`org.zerodds.sec.kx_key_fallback=accept`); `false`, the
    /// specification, by default.
    pub kx_key_fallback: bool,
    /// How the algorithm names of a handshake token are written
    /// (`org.zerodds.sec.algo_name_nul`, DDS-Security 1.2 §7.3.3.1): with the
    /// NUL (`spec`, the default), without it (`omit`) or by the peer's vendor
    /// (`by_vendor`).
    pub algo_name_nul: zerodds_security::AlgoNameNul,
    /// The spelling of the algorithm names the initiator sends
    /// (`org.zerodds.sec.algorithm_names`, DDS-Security 1.2 §8.2 Table 25,
    /// §8.3 Table 26): `1.2`, the names of the tables, by default.
    pub algorithm_names: zerodds_security_pki::AlgorithmNames,
    /// The form of the subject names in the IdentityToken and the
    /// PermissionsToken (`org.zerodds.sec.subject_name_form`, DDS-Security 1.2
    /// §9.4.1.2.3): RFC 4514 by default.
    pub subject_name_form: zerodds_security::SubjectNameForm,
    /// Under origin authentication, an RTPS message or submessage under a key
    /// without a receiver-specific key is verified with the common MAC alone
    /// (`org.zerodds.sec.origin_authentication_fallback=accept`); `false`, the
    /// refusal of DDS-Security 1.2 §10.5.3.3 Table 80, by default.
    pub origin_authentication_fallback: bool,
}

/// Where one piece of security material comes from.
///
/// DDS-Security 1.2 §9.3.1 Table 44 and §10.3.1 Table 50 write each
/// value as a URI: `file:` names a file, `data:,` carries the content
/// itself after the comma, `pkcs11:` names an object on a PKCS#11 token
/// (RFC 7512). Cyclone DDS reads the two the same way
/// (`DDS_Security_get_conf_item_type`, `dds_security_utils.c`): the text
/// after `data:,` is the content, unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Material {
    /// A file, read when the profile is built.
    File(PathBuf),
    /// The content itself.
    Inline(Vec<u8>),
    /// An object on a PKCS#11 token (RFC 7512). The certificate material
    /// is read from `CKA_VALUE`; a private key stays on the token and is
    /// signed with through `C_Sign`.
    Pkcs11(crate::pkcs11::Pkcs11Uri),
}

impl Material {
    /// The file, for material that is one.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::File(path) => Some(path),
            Self::Inline(_) | Self::Pkcs11(_) => None,
        }
    }

    /// The content: the file's, read now, the inline bytes, or the
    /// certificate object's `CKA_VALUE` read from the token.
    ///
    /// # Errors
    /// [`SecurityProfileError::Io`] when the file cannot be read, or
    /// [`SecurityProfileError::Pkcs11`] when the token read fails.
    pub fn read(&self) -> Result<Vec<u8>, SecurityProfileError> {
        match self {
            Self::File(path) => read_path(path),
            Self::Inline(bytes) => Ok(bytes.clone()),
            Self::Pkcs11(uri) => {
                crate::pkcs11::read_certificate_der(uri).map_err(SecurityProfileError::Pkcs11)
            }
        }
    }
}

impl From<PathBuf> for Material {
    fn from(path: PathBuf) -> Self {
        Self::File(path)
    }
}

impl From<&Path> for Material {
    fn from(path: &Path) -> Self {
        Self::File(path.to_path_buf())
    }
}

impl From<&str> for Material {
    /// A path, as a caller writes one in a test or a tool.
    fn from(path: &str) -> Self {
        Self::File(PathBuf::from(path))
    }
}

/// Fully built security profile. The caller typically only needs
/// `gate` (attach to `RuntimeConfig.security`) — `pki`/`identity_handle`
/// are needed for later programmatic handshake driving
/// (e.g. for tests that work without SEDP).
pub struct SecurityProfile {
    /// Ready-to-consume [`SharedSecurityGate`] in `Arc` form, as
    /// `RuntimeConfig.security` expects it.
    pub gate: Arc<SharedSecurityGate>,
    /// PKI plugin with a registered local identity. `Arc<Mutex>`,
    /// because it is shared by both the handshake driver (`&mut` for
    /// begin/process_handshake) and the crypto plugin (as a
    /// [`SharedSecretProvider`], `&self`).
    pub pki: Arc<Mutex<PkiAuthenticationPlugin>>,
    /// Handle of the local participant in the PKI plugin.
    pub identity_handle: IdentityHandle,
    /// The DDS-Security §9.3.3-adjusted 16-byte participant GUID (prefix
    /// cryptographically bound to the identity). The caller MUST use this GUID
    /// (or its prefix) for the runtime/SPDP participant, so that
    /// the SPDP beacon, handshake `c.pdata` and all entity GUIDs are consistent.
    pub adjusted_participant_guid: [u8; 16],
    /// Parsed governance.
    pub governance: Governance,
    /// Parsed permissions.
    pub permissions: Permissions,
    /// The signed permissions document as it was read: the
    /// PermissionsCredentialToken (Table 64) carries it and the handshake
    /// sends it as `c.perm`.
    pub permissions_signed: Vec<u8>,
    /// The signed governance document as it was read.
    pub governance_signed: Vec<u8>,
    /// The identity certificate of the participant, PEM, as it was read.
    pub identity_certificate_pem: Vec<u8>,
    /// PEM bundle of the permissions CA, retained so the gate can verify a
    /// remote peer's signed `c.perm` for the remote endpoint checks.
    pub permissions_ca_pem: alloc::vec::Vec<u8>,
}

impl core::fmt::Debug for SecurityProfile {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // PkiAuthenticationPlugin holds cert/key/secrets — never
        // debug-print. Gate debug is OK (shows only metadata).
        f.debug_struct("SecurityProfile")
            .field("gate", &self.gate)
            .field("identity_handle", &self.identity_handle)
            .field("governance", &self.governance)
            .field("permissions", &"<redacted>")
            .field("pki", &"<redacted PkiAuthenticationPlugin>")
            .finish()
    }
}

impl SecurityProfile {
    /// Reads all files, verifies CMS signatures, builds PKI + gate.
    ///
    /// `participant_guid` is the 16-byte DDS GUID of the local
    /// participant — embedded by the PKI plugin into the handshake
    /// token.
    ///
    /// # Errors
    /// [`SecurityProfileError`] in the variants Io / Pki / Permissions
    /// / GovernanceUtf8.
    pub fn from_files(
        cfg: &SecurityProfileConfig,
        participant_guid: [u8; 16],
    ) -> Result<Self, SecurityProfileError> {
        Self::from_files_with_cert_policy(cfg, participant_guid, CertPolicy::spec())
    }

    /// [`SecurityProfile::from_files`], with the identity-certificate
    /// validation policy named explicitly.
    ///
    /// [`CertPolicy::spec`] — what `from_files` passes — is DDS-Security
    /// 1.2 §10.3.1.3: X.509 v3 identity certificates only. The one other
    /// policy this stack has is the X.509 v1 deviation, which the caller
    /// has to switch on deliberately and which relaxes the version check
    /// and nothing else; see `zerodds_security_pki::x509v1_compat`.
    ///
    /// # Errors
    /// [`SecurityProfileError`] in the variants Io / Pki / Permissions
    /// / GovernanceUtf8.
    pub fn from_files_with_cert_policy(
        cfg: &SecurityProfileConfig,
        participant_guid: [u8; 16],
        cert_policy: CertPolicy,
    ) -> Result<Self, SecurityProfileError> {
        let identity_cert_pem = cfg.identity_cert_pem.read()?;
        let identity_cert_pem_bytes = identity_cert_pem.clone();
        let identity_ca_pem = cfg.identity_ca_pem.read()?;
        let permissions_ca_pem = cfg.permissions_ca_pem.read()?;
        let governance_p7s = cfg.governance_p7s.read()?;
        let permissions_p7s = cfg.permissions_p7s.read()?;

        // DDS-Security §9.3.3: bind the participant GUID prefix
        // cryptographically to the identity. Cyclone/FastDDS check in the handshake
        // (begin_handshake_reply) that the c.pdata GUID is derived from the
        // identity — a random GUID is rejected with "c.pdata contains
        // incorrect participant guid".
        let cert_der = zerodds_security_pki::first_cert_der(&identity_cert_pem)?;

        // The private key: either PKCS#8 bytes read from a file or inline,
        // or a token-backed signer whose key never leaves the token
        // (DDS-Security 1.2 §9.3.1 Table 44, RFC 7512). The two never
        // coexist. §10.3.1 Table 50: the password "does not apply to private
        // keys supplied with the 'pkcs11:' URI"; the token PIN comes from
        // that URI, and a password next to it has no effect.
        let token_uri = match &cfg.identity_key_pem {
            Material::Pkcs11(uri) => Some(uri.clone()),
            _ => None,
        };
        #[cfg(feature = "pkcs11")]
        let token_signer: Option<
            alloc::sync::Arc<dyn zerodds_security_pki::IdentitySigner>,
        > = match &token_uri {
            Some(uri) => {
                let algo = zerodds_security_pki::cert_key_algo(&cert_der);
                Some(Arc::new(crate::pkcs11::Pkcs11Signer::open(uri, algo)?))
            }
            None => None,
        };
        #[cfg(not(feature = "pkcs11"))]
        let token_signer: Option<
            alloc::sync::Arc<dyn zerodds_security_pki::IdentitySigner>,
        > = match &token_uri {
            Some(_) => {
                return Err(SecurityProfileError::Pkcs11(
                    crate::pkcs11::Pkcs11Error::FeatureDisabled,
                ));
            }
            None => None,
        };
        let identity_key_pem = private_key_pem(
            &cfg.identity_key_pem,
            cfg.identity_key_password.as_deref(),
            cfg.identity_key_password_reading,
        )?;

        // 1. PKI: validate the identity (chain against identity_ca, algo whitelist)
        //    — bound to the ADJUSTED GUID that the participant actually
        //    announces + carries as source_guid/c.pdata in the handshake.
        let mut pki = PkiAuthenticationPlugin::new();
        pki.set_algorithm_names(cfg.algorithm_names);
        pki.set_subject_name_form(cfg.subject_name_form);
        // The certificate policy has to be in place before the identity is
        // validated — it is what decides whether the identity is accepted
        // at all.
        pki.set_accept_x509v1_identity_certificates(cert_policy.accept_x509v1_identity_certificate);
        pki.set_expected_certificate_subject(cert_policy.expected_subject.clone());
        pki.set_ocsp_policy(cert_policy.ocsp);
        let mut crls = cert_policy.crls_der.clone();
        if let Some(material) = &cfg.identity_crl {
            crls.extend(zerodds_security_pki::crls_der(&material.read()?)?);
        }
        pki.set_crls(crls);
        let identity_cfg = IdentityConfig {
            identity_cert_pem,
            identity_ca_pem,
            identity_key_pem,
        };
        // `validate_local_identity` (§9.3.2.11.2, Table 59) returns the
        // `adjusted_participant_guid` for the candidate GUID, and
        // VALIDATION_PENDING_RETRY while the verification has not finished
        // (an OCSP responder is asked): "the operation shall be called again
        // after a configurable delay [...] until the operation returns either
        // VALIDATION_OK [...] or VALIDATION_FAILED" (Figure 9, RetryWait1).
        // A private key on a token signs through its signer, which only the
        // blocking `validate_identity` takes.
        let validated = match token_signer {
            Some(signer) => {
                pki.validate_identity(identity_cfg, cfg.domain_id, participant_guid, Some(signer))?
            }
            None => {
                let props = auth_properties(&identity_cfg, &cert_policy)?;
                loop {
                    let validated = AuthenticationPlugin::validate_local_identity(
                        &mut pki,
                        cfg.domain_id,
                        &props,
                        participant_guid,
                    )?;
                    if validated.result != ValidationResult::PendingRetry {
                        break validated;
                    }
                    std::thread::sleep(LOCAL_VALIDATION_RETRY_DELAY);
                }
            }
        };
        let identity_handle = validated.identity;
        let adjusted_participant_guid = validated.adjusted_participant_guid;

        // 2. CMS verifier on the permissions CA — the same verifier
        //    is used for governance.p7s AND permissions.p7s
        //    (both are typically signed by the same authority).
        let verifier = CmsPkcs7Verifier::new(&permissions_ca_pem)?;

        // 3. Extract + parse the governance XML from the CMS wrapper.
        let governance_xml_bytes = verifier.verify_and_extract(&governance_p7s)?;
        let governance_xml = core::str::from_utf8(&governance_xml_bytes)
            .map_err(SecurityProfileError::GovernanceUtf8)?;
        let governance = parse_governance_xml(governance_xml)?;

        // 4. Permissions analogously via `open_signed_permissions`, reduced
        //    to the grant of this identity: "A permissions section with a
        //    subject name that does not match the subject name given in the
        //    corresponding Identity Certificate will be ignored"
        //    (§10.4.1.5.3.1); without one `validate_local_permissions` fails
        //    (Table 70).
        let permissions = open_signed_permissions(&permissions_p7s, &verifier)?
            .for_subject(&zerodds_security_pki::subject_attributes(&cert_der)?)
            .ok_or(SecurityProfileError::NoGrantForSubject)?;

        // 4b. Give the CMS-signed permissions document to the PKI plugin —
        //     it is sent along as `c.perm` in the auth handshake. Foreign vendors
        //     with active access control (governance) validate this signature
        //     against the permissions_ca; without `c.perm` they reject the
        //     handshake (cross-vendor NO_MATCH).
        //     That is `set_permissions_credential_and_token` (§9.3.2.11.12):
        //     the credential token of DDS:Access:Permissions holds the
        //     document as `dds.perm.cert` (§10.4.2.1 Table 64), the
        //     PermissionsToken the class the SPDP announces.
        {
            use zerodds_security::token::{DataHolder, class_id};
            let credential = match core::str::from_utf8(&permissions_p7s) {
                Ok(text) => DataHolder::new(class_id::ACCESS_PERMISSIONS_CREDENTIAL)
                    .with_property(zerodds_security_pki::PERMISSIONS_CREDENTIAL_CERT, text),
                Err(_) => DataHolder::new(class_id::ACCESS_PERMISSIONS_CREDENTIAL)
                    .with_binary_property(
                        zerodds_security_pki::PERMISSIONS_CREDENTIAL_CERT,
                        permissions_p7s.clone(),
                    ),
            };
            let token = DataHolder::new("DDS:Access:Permissions:1.0");
            pki.set_permissions_credential_and_token(
                identity_handle,
                &credential.to_cdr_le(),
                &token.to_cdr_le(),
            )
            .map_err(SecurityProfileError::PkiSecurity)?;
        }
        // §7.3.11: the signature algorithm of that document is one the trust
        // chain requires of a peer.
        if let Ok(oid) = verifier.signature_algorithm_oid(&permissions_p7s) {
            pki.set_local_permissions_signature_algorithm(&oid);
        }

        // 5. Build the gate with AES-GCM crypto + parsed governance.
        //    The crypto plugin gets the PKI plugin as a
        //    SharedSecretProvider: after the auth handshake completes,
        //    `register_matched_remote_participant` derives the per-peer
        //    master key deterministically from the DH shared secret (instead of
        //    random + token exchange). Without a completed handshake
        //    (e.g. the ZeroDDS-self path without enable_security_builtins)
        //    the provider returns None and the crypto plugin falls back to
        //    the v1.4 random-key path — backward compat preserved.
        let pki = Arc::new(Mutex::new(pki));
        let provider: Arc<dyn SharedSecretProvider> =
            Arc::new(SharedPkiSecretProvider(Arc::clone(&pki)));
        let crypto = builtin_crypto_plugin(cfg, &governance, provider);
        let gate = gate_with_crypto(cfg, &governance, Box::new(crypto))?;

        Ok(Self {
            gate,
            pki,
            identity_handle,
            adjusted_participant_guid,
            governance,
            permissions,
            permissions_signed: permissions_p7s,
            governance_signed: governance_p7s,
            identity_certificate_pem: identity_cert_pem_bytes,
            permissions_ca_pem,
        })
    }

    /// C7 — loads a profile from an **SROS2 enclave directory** in one call.
    ///
    /// An SROS2 keystore lays each participant's material out under
    /// `enclaves/<name>/` and symlinks the standard file names into it:
    ///
    /// | enclave file              | role                          |
    /// |---------------------------|-------------------------------|
    /// | `cert.pem`                | identity certificate          |
    /// | `key.pem`                 | identity private key (PKCS#8) |
    /// | `identity_ca.cert.pem`    | identity CA bundle            |
    /// | `permissions_ca.cert.pem` | permissions CA bundle         |
    /// | `governance.p7s`          | CMS-signed governance XML     |
    /// | `permissions.p7s`         | CMS-signed permissions XML    |
    ///
    /// This replaces the six-path [`SecurityProfileConfig`] ceremony with a
    /// single directory, mirroring how `ros2 security` enclaves are consumed.
    ///
    /// # Errors
    /// [`SecurityProfileError::Io`] when a standard file is absent, plus the
    /// Pki/Permissions/Governance variants from [`Self::from_files`].
    pub fn from_enclave_dir(
        enclave_dir: impl AsRef<Path>,
        domain_id: u32,
        participant_guid: [u8; 16],
    ) -> Result<Self, SecurityProfileError> {
        Self::from_enclave_dir_with_env(enclave_dir, domain_id, participant_guid, &|name| {
            std::env::var(name).ok()
        })
    }

    /// [`Self::from_enclave_dir`] with the vendor-form variables (`ZERODDS_SEC_*`)
    /// given as a lookup.
    ///
    /// # Errors
    /// As [`Self::from_enclave_dir`]; a vendor-form value that is not allowed
    /// is [`SecurityProfileError::Setting`].
    pub fn from_enclave_dir_with_env(
        enclave_dir: impl AsRef<Path>,
        domain_id: u32,
        participant_guid: [u8; 16],
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Self, SecurityProfileError> {
        let dir = enclave_dir.as_ref();
        // No property list here: the two vendor-form settings come from the
        // environment (`ZERODDS_SEC_KX_KEY_FALLBACK`,
        // `ZERODDS_SEC_COMPATIBILITY_CHECK`).
        let forms = crate::qos_properties::vendor_form_settings_from_env(env)
            .map_err(|error| SecurityProfileError::Setting(error.to_string()))?;
        let cfg = SecurityProfileConfig {
            domain_id,
            identity_ca_pem: dir.join("identity_ca.cert.pem").into(),
            identity_cert_pem: dir.join("cert.pem").into(),
            identity_key_pem: dir.join("key.pem").into(),
            permissions_ca_pem: dir.join("permissions_ca.cert.pem").into(),
            governance_p7s: dir.join("governance.p7s").into(),
            permissions_p7s: dir.join("permissions.p7s").into(),
            identity_key_password: None,
            identity_key_password_reading: Default::default(),
            symmetric_cipher_bits: 256,
            max_blocks_per_session: None,
            partition_matching: zerodds_security_permissions::PartitionMatching::Subset,
            plaintext_loopback: false,
            compatibility_checks: forms.compatibility_checks,
            kx_key_fallback: forms.kx_key_fallback,
            algo_name_nul: forms.algo_name_nul,
            algorithm_names: forms.algorithm_names,
            subject_name_form: forms.subject_name_form,
            origin_authentication_fallback: forms.origin_authentication_fallback,
            identity_crl: None,
        };
        Self::from_files(&cfg, participant_guid)
    }
}

/// The governance of `cfg`, read from `cfg.governance_p7s` and verified
/// against `cfg.permissions_ca_pem`; with the signed document as it was read.
///
/// # Errors
/// [`SecurityProfileError`] for a file that cannot be read, a signature that
/// does not verify or a document that does not parse.
pub fn governance_from_files(
    cfg: &SecurityProfileConfig,
) -> Result<(Governance, Vec<u8>, Vec<u8>), SecurityProfileError> {
    let permissions_ca_pem = cfg.permissions_ca_pem.read()?;
    let governance_p7s = cfg.governance_p7s.read()?;
    let verifier = CmsPkcs7Verifier::new(&permissions_ca_pem)?;
    let governance_xml_bytes = verifier.verify_and_extract(&governance_p7s)?;
    let governance_xml = core::str::from_utf8(&governance_xml_bytes)
        .map_err(SecurityProfileError::GovernanceUtf8)?;
    Ok((
        parse_governance_xml(governance_xml)?,
        governance_p7s,
        permissions_ca_pem,
    ))
}

/// The builtin AES-GCM-GMAC plugin as the profile builds it from `governance`:
/// the suites follow the protection kinds of the domain rule, and the secrets
/// of the handshake come from `provider`.
#[must_use]
pub fn builtin_crypto_plugin(
    cfg: &SecurityProfileConfig,
    governance: &Governance,
    provider: Arc<dyn SharedSecretProvider>,
) -> AesGcmCryptoPlugin {
    // Per-scope suites from the governance: *_protection_kind=SIGN -> AES-256-
    // GMAC (auth-only, cyclone-conform), otherwise AES-256-GCM. participant_suite
    // <- rtps_protection (SRTPS message key); endpoint_suite <- data_protection
    // (payload/submessage key). The Kx key stays independently GCM.
    let bits = cfg.symmetric_cipher_bits;
    let sign_suite = |k: zerodds_security_permissions::ProtectionKind| {
        use zerodds_security_permissions::ProtectionKind;
        if matches!(
            k,
            ProtectionKind::Sign | ProtectionKind::SignWithOriginAuthentication
        ) {
            Suite::Aes256Gmac.with_key_bits(bits)
        } else {
            Suite::Aes256Gcm.with_key_bits(bits)
        }
    };
    let domain_rule = governance.find_domain_rule(cfg.domain_id);
    let rtps_kind = domain_rule
        .map(|r| r.rtps_protection_kind)
        .unwrap_or_default();
    // The two endpoint suites here are the PARTICIPANT-WIDE fallback of
    // the plugin, for an endpoint registered without its kinds (see
    // `SharedSecurityGate::register_local_endpoint_with_kinds`, Table 78).
    let participant_wide = governance.find_topic_rule(cfg.domain_id, PARTICIPANT_WIDE_TOPIC);
    let data_kind = participant_wide
        .map(|t| t.data_protection_kind)
        .unwrap_or_default();
    let metadata_kind = participant_wide
        .map(|t| t.metadata_protection_kind)
        .unwrap_or_default();
    let mut crypto =
        AesGcmCryptoPlugin::with_secret_provider(Suite::Aes256Gcm.with_key_bits(bits), provider);
    if let Some(blocks) = cfg.max_blocks_per_session {
        crypto.set_max_blocks_per_session(blocks);
    }
    // Set metadata_suite only when metadata protection is active; then
    // the submessage key gets the metadata kind and (if != data kind)
    // register_local_endpoint switches to the dual-key model (meta-sign-data).
    let metadata_suite = if matches!(
        metadata_kind,
        zerodds_security_permissions::ProtectionKind::None
    ) {
        None
    } else {
        Some(sign_suite(metadata_kind))
    };
    crypto.set_local_protection_suites(
        Some(sign_suite(rtps_kind)),
        Some(sign_suite(data_kind)),
        metadata_suite,
    );
    crypto
}

/// The gate over `governance` and the Cryptographic plugin `crypto`, with the
/// settings of `cfg`.
///
/// # Errors
/// [`SecurityProfileError`] for a setting the gate refuses.
pub fn gate_with_crypto(
    cfg: &SecurityProfileConfig,
    governance: &Governance,
    crypto: Box<dyn zerodds_security::cryptographic::CryptographicPluginInterfaces>,
) -> Result<Arc<SharedSecurityGate>, SecurityProfileError> {
    let mut gate_governance = governance.clone();
    gate_governance.set_partition_matching(cfg.partition_matching);
    let gate = Arc::new(SharedSecurityGate::new(
        cfg.domain_id,
        gate_governance,
        crypto,
    ));
    let refused = |e| {
        SecurityProfileError::Permissions(PermissionsError::Malformed(alloc::format!(
            "security gate: {e:?}"
        )))
    };
    gate.set_symmetric_cipher_bits(cfg.symmetric_cipher_bits)
        .and_then(|()| gate.set_compatibility_checks(cfg.compatibility_checks))
        .and_then(|()| gate.set_kx_key_fallback(cfg.kx_key_fallback))
        .and_then(|()| gate.set_algo_name_nul(cfg.algo_name_nul))
        .and_then(|()| gate.set_subject_name_form(cfg.subject_name_form))
        .and_then(|()| gate.set_algorithm_names(cfg.algorithm_names))
        .and_then(|()| gate.set_origin_authentication_fallback(cfg.origin_authentication_fallback))
        .map_err(refused)?;
    gate.set_plaintext_loopback(cfg.plaintext_loopback)
        .map_err(refused)?;
    Ok(gate)
}

/// PKCS#8 DER as the `PRIVATE KEY` PEM block the PKI plugin reads.
/// The delay between two calls of `validate_local_identity` that returned
/// VALIDATION_PENDING_RETRY (§9.3.2.11.2: "after a configurable delay").
pub const LOCAL_VALIDATION_RETRY_DELAY: core::time::Duration =
    core::time::Duration::from_millis(100);

/// The properties of Table 50 for `validate_local_identity`: the identity
/// certificate, the CA and the private key, and the settings of the policy the
/// profile was asked to build under.
fn auth_properties(
    identity: &IdentityConfig,
    policy: &CertPolicy,
) -> Result<PropertyList, PkiError> {
    let text = |what: &'static str, bytes: &[u8]| {
        core::str::from_utf8(bytes)
            .map(str::to_string)
            .map_err(|_| PkiError::InvalidPem(alloc::format!("{what} is not PEM text")))
    };
    let mut props = PropertyList::new()
        .with(Property::local(
            "dds.sec.auth.identity_certificate",
            text("the identity certificate", &identity.identity_cert_pem)?,
        ))
        .with(Property::local(
            "dds.sec.auth.identity_ca",
            text("the identity CA", &identity.identity_ca_pem)?,
        ))
        .with(Property::local(
            zerodds_security_pki::PROP_ACCEPT_X509V1_IDENTITY,
            if policy.accept_x509v1_identity_certificate {
                "true"
            } else {
                "false"
            },
        ));
    if let Some(key) = &identity.identity_key_pem {
        props.set(Property::local(
            "dds.sec.auth.private_key",
            text("the private key", key)?,
        ));
    }
    Ok(props)
}

/// The PEM of the private key to read, decrypted with `password` where there
/// is one; `None` for a key on a token. DDS-Security 1.2 §10.3.1 Table 50:
/// "The password property is only used if the private_key is provided with a
/// 'file:' or a 'data:' URI. It does not apply to private keys supplied with
/// the 'pkcs11:' URI": the password next to a `pkcs11:` key is not applied,
/// and not an error (the PIN of the token is `pin-value` of the URI).
///
/// # Errors
/// The key cannot be read, or not be decrypted with the password.
fn private_key_pem(
    key: &Material,
    password: Option<&str>,
    reading: zerodds_security_pki::encrypted_key::PasswordReading,
) -> Result<Option<Vec<u8>>, SecurityProfileError> {
    if matches!(key, Material::Pkcs11(_)) {
        return Ok(None);
    }
    Ok(Some(match password {
        Some(password) => pkcs8_pem(
            &zerodds_security_pki::encrypted_key::decrypt_private_key_pem_as(
                &key.read()?,
                password,
                reading,
            )?,
        ),
        None => key.read()?,
    }))
}

fn pkcs8_pem(der: &[u8]) -> Vec<u8> {
    use base64::Engine as _;
    let body = base64::engine::general_purpose::STANDARD.encode(der);
    let mut pem = String::from("-----BEGIN PRIVATE KEY-----\n");
    for line in body.as_bytes().chunks(64) {
        pem.push_str(core::str::from_utf8(line).unwrap_or_default());
        pem.push('\n');
    }
    pem.push_str("-----END PRIVATE KEY-----\n");
    pem.into_bytes()
}

fn read_path(p: &Path) -> Result<Vec<u8>, SecurityProfileError> {
    std::fs::read(p).map_err(|source| SecurityProfileError::Io {
        path: p.to_path_buf(),
        source,
    })
}

/// Small helper renderer that extracts a path property from a
/// `file:///abs/path` or plain-path string. The vendor
/// property strings are typically `file:///etc/dds/certs/...`,
/// partly also bare — both forms are swallowed.
#[must_use]
pub fn strip_file_url(s: &str) -> String {
    s.strip_prefix("file://")
        .map(|rest| rest.trim_start_matches('/').to_string())
        .map(|rest| {
            // `file:///etc/...` → `/etc/...`; `file://etc/...` (rare) → `etc/...`
            if s.starts_with("file:///") {
                format!("/{rest}")
            } else {
                rest
            }
        })
        .unwrap_or_else(|| s.to_string())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn strip_file_url_handles_triple_slash() {
        assert_eq!(
            strip_file_url("file:///etc/dds/certs/ca.pem"),
            "/etc/dds/certs/ca.pem"
        );
    }

    #[test]
    fn strip_file_url_handles_double_slash() {
        assert_eq!(
            strip_file_url("file://relative/path.pem"),
            "relative/path.pem"
        );
    }

    #[test]
    fn strip_file_url_passes_plain_path() {
        assert_eq!(strip_file_url("/tmp/whatever.pem"), "/tmp/whatever.pem");
    }

    #[test]
    fn missing_file_returns_io_error() {
        let cfg = SecurityProfileConfig {
            domain_id: 0,
            identity_ca_pem: PathBuf::from("/zerodds/_does_not_exist_ca.pem").into(),
            identity_cert_pem: PathBuf::from("/zerodds/_does_not_exist_cert.pem").into(),
            identity_key_pem: PathBuf::from("/zerodds/_does_not_exist_key.pem").into(),
            permissions_ca_pem: PathBuf::from("/zerodds/_does_not_exist_pca.pem").into(),
            governance_p7s: PathBuf::from("/zerodds/_does_not_exist_gov.p7s").into(),
            permissions_p7s: PathBuf::from("/zerodds/_does_not_exist_perm.p7s").into(),
            identity_key_password: None,
            identity_key_password_reading: Default::default(),
            symmetric_cipher_bits: 256,
            max_blocks_per_session: None,
            partition_matching: zerodds_security_permissions::PartitionMatching::Subset,
            plaintext_loopback: false,
            compatibility_checks: true,
            kx_key_fallback: false,
            algo_name_nul: Default::default(),
            algorithm_names: Default::default(),
            subject_name_form: Default::default(),
            origin_authentication_fallback: false,
            identity_crl: None,
        };
        match SecurityProfile::from_files(&cfg, [0u8; 16]) {
            Err(SecurityProfileError::Io { .. }) => {}
            Err(e) => panic!("expected Io error, got: {e:?}"),
            Ok(_) => panic!("expected Err, got Ok"),
        }
    }

    /// DDS-Security 1.2 §10.3.1 Table 50: "The password property is only used
    /// if the private_key is provided with a 'file:' or a 'data:' URI. It does
    /// not apply to private keys supplied with the 'pkcs11:' URI." A password
    /// next to a `pkcs11:` key is not applied and not an error; next to a
    /// `data:` key it opens the key.
    #[test]
    fn a_password_does_not_apply_to_a_pkcs11_key() {
        use zerodds_security_pki::encrypted_key::PasswordReading;
        let token_key = Material::Pkcs11(
            crate::pkcs11::parse(
                "pkcs11:object=MyParticipantPrivateKey;type=private?pin-value=OpenSesame",
            )
            .expect("uri"),
        );
        // The password is not read: the key is on the token.
        assert_eq!(
            private_key_pem(&token_key, Some("OpenSesame"), PasswordReading::default()).unwrap(),
            None
        );
        assert_eq!(
            private_key_pem(&token_key, None, PasswordReading::default()).unwrap(),
            None
        );
        // For a key in the PEM form the password opens it: the key of the
        // clause's own example, encrypted with AES-128 under the base64 key.
        let encrypted = Material::Inline(
            include_bytes!("../../security-pki/tests/fixtures/encrypted_keys/ec_trad_spec_key.pem")
                .to_vec(),
        );
        let plain = private_key_pem(
            &encrypted,
            Some("AAECAwQFBgcICQoLDA0ODw=="),
            PasswordReading::default(),
        )
        .unwrap()
        .expect("a PEM key");
        assert!(plain.starts_with(b"-----BEGIN PRIVATE KEY-----"));
        // A wrong password for a PEM key is an error, not a silent pass.
        assert!(private_key_pem(&encrypted, Some("wrong"), PasswordReading::default()).is_err());
    }

    /// Unique scratch directory under the system temp dir (no external
    /// `tempfile` dep; cleaned by the caller).
    fn scratch_dir(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "zerodds_enclave_{tag}_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).expect("mkdir scratch");
        dir
    }

    #[test]
    fn enclave_dir_resolves_all_sros2_filenames() {
        // C7: from_enclave_dir must map the six SROS2 enclave file names. With
        // all six present (dummy content) it gets PAST file resolution and
        // fails later at PKI parsing — i.e. NO Io error → the mapping is
        // correct end-to-end.
        let dir = scratch_dir("all");
        for f in [
            "cert.pem",
            "key.pem",
            "identity_ca.cert.pem",
            "permissions_ca.cert.pem",
            "governance.p7s",
            "permissions.p7s",
        ] {
            std::fs::write(dir.join(f), b"dummy").expect("write");
        }
        let res = SecurityProfile::from_enclave_dir(&dir, 0, [0u8; 16]);
        std::fs::remove_dir_all(&dir).ok();
        match res {
            Err(SecurityProfileError::Io { path, .. }) => {
                panic!(
                    "filename mapping wrong — unexpected Io on {}",
                    path.display()
                )
            }
            Err(_) => {} // expected: PKI/parse error on the dummy cert
            Ok(_) => panic!("dummy content must not build a valid profile"),
        }
    }

    #[test]
    fn enclave_dir_missing_cert_is_io_naming_cert() {
        // An empty enclave dir → the first read (cert.pem) fails with an Io
        // error whose path is exactly the SROS2 cert name.
        let dir = scratch_dir("nocert");
        let res = SecurityProfile::from_enclave_dir(&dir, 0, [0u8; 16]);
        std::fs::remove_dir_all(&dir).ok();
        match res {
            Err(SecurityProfileError::Io { path, .. }) => {
                assert!(
                    path.ends_with("cert.pem"),
                    "Io path should be the enclave cert.pem, got {}",
                    path.display()
                );
            }
            other => panic!("expected Io on cert.pem, got {other:?}"),
        }
    }
}
