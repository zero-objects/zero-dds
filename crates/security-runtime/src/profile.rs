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

use zerodds_security::authentication::{SharedSecretHandle, SharedSecretProvider};
use zerodds_security::error::SecurityError;
use zerodds_security_crypto::{AesGcmCryptoPlugin, Suite};
use zerodds_security_permissions::{
    CmsPkcs7Verifier, Governance, Permissions, PermissionsError, XmlSignatureVerifier,
    open_signed_permissions, parse_governance_xml,
};
use zerodds_security_pki::{IdentityConfig, IdentityHandle, PkiAuthenticationPlugin, PkiError};

use crate::SharedSecurityGate;

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
    pub identity_ca_pem: PathBuf,
    /// PEM with the identity cert of the local participant.
    pub identity_cert_pem: PathBuf,
    /// PKCS#8 PEM private key of the local participant.
    pub identity_key_pem: PathBuf,
    /// PEM bundle of the permissions CA (signs governance/permissions).
    pub permissions_ca_pem: PathBuf,
    /// CMS-signed governance XML.
    pub governance_p7s: PathBuf,
    /// CMS-signed permissions XML.
    pub permissions_p7s: PathBuf,
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
        let identity_cert_pem = read_path(&cfg.identity_cert_pem)?;
        let identity_ca_pem = read_path(&cfg.identity_ca_pem)?;
        let identity_key_pem = read_path(&cfg.identity_key_pem)?;
        let permissions_ca_pem = read_path(&cfg.permissions_ca_pem)?;
        let governance_p7s = read_path(&cfg.governance_p7s)?;
        let permissions_p7s = read_path(&cfg.permissions_p7s)?;

        // DDS-Security §9.3.3: bind the participant GUID prefix
        // cryptographically to the identity. Cyclone/FastDDS check in the handshake
        // (begin_handshake_reply) that the c.pdata GUID is derived from the
        // identity — a random GUID is rejected with "c.pdata contains
        // incorrect participant guid".
        let cert_der = zerodds_security_pki::first_cert_der(&identity_cert_pem)?;
        let adjusted_prefix =
            zerodds_security_pki::adjust_participant_guid_prefix(&participant_guid, &cert_der)?;
        let mut adjusted_participant_guid = participant_guid;
        adjusted_participant_guid[..12].copy_from_slice(&adjusted_prefix);

        // 1. PKI: validate the identity (chain against identity_ca, algo whitelist)
        //    — bound to the ADJUSTED GUID that the participant actually
        //    announces + carries as source_guid/c.pdata in the handshake.
        let mut pki = PkiAuthenticationPlugin::new();
        let identity_handle = pki.validate_with_config(
            IdentityConfig {
                identity_cert_pem,
                identity_ca_pem,
                identity_key_pem: Some(identity_key_pem),
            },
            adjusted_participant_guid,
        )?;

        // 2. CMS verifier on the permissions CA — the same verifier
        //    is used for governance.p7s AND permissions.p7s
        //    (both are typically signed by the same authority).
        let verifier = CmsPkcs7Verifier::new(&permissions_ca_pem)?;

        // 3. Extract + parse the governance XML from the CMS wrapper.
        let governance_xml_bytes = verifier.verify_and_extract(&governance_p7s)?;
        let governance_xml = core::str::from_utf8(&governance_xml_bytes)
            .map_err(SecurityProfileError::GovernanceUtf8)?;
        let governance = parse_governance_xml(governance_xml)?;

        // 4. Permissions analogously via `open_signed_permissions`.
        let permissions = open_signed_permissions(&permissions_p7s, &verifier)?;

        // 4b. Give the CMS-signed permissions document to the PKI plugin —
        //     it is sent along as `c.perm` in the auth handshake. Foreign vendors
        //     with active access control (governance) validate this signature
        //     against the permissions_ca; without `c.perm` they reject the
        //     handshake (cross-vendor NO_MATCH).
        pki.set_local_permissions(permissions_p7s.clone());

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
        // Per-scope suites from the governance: *_protection_kind=SIGN -> AES-256-
        // GMAC (auth-only, cyclone-conform), otherwise AES-256-GCM. participant_suite
        // <- rtps_protection (SRTPS message key); endpoint_suite <- data_protection
        // (payload/submessage key). The Kx key stays independently GCM.
        let sign_suite = |k: zerodds_security_permissions::ProtectionKind| {
            if matches!(k, zerodds_security_permissions::ProtectionKind::Sign) {
                Suite::Aes256Gmac
            } else {
                Suite::Aes256Gcm
            }
        };
        let domain_rule = governance.find_domain_rule(cfg.domain_id);
        let rtps_kind = domain_rule
            .map(|r| r.rtps_protection_kind)
            .unwrap_or_default();
        let data_kind = domain_rule
            .and_then(|r| r.topic_rules.first())
            .map(|t| t.data_protection_kind)
            .unwrap_or_default();
        let metadata_kind = domain_rule
            .and_then(|r| r.topic_rules.first())
            .map(|t| t.metadata_protection_kind)
            .unwrap_or_default();
        let mut crypto = AesGcmCryptoPlugin::with_secret_provider(Suite::Aes256Gcm, provider);
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
        let gate = Arc::new(SharedSecurityGate::new(
            cfg.domain_id,
            governance.clone(),
            Box::new(crypto),
        ));

        Ok(Self {
            gate,
            pki,
            identity_handle,
            adjusted_participant_guid,
            governance,
            permissions,
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
        let dir = enclave_dir.as_ref();
        let cfg = SecurityProfileConfig {
            domain_id,
            identity_ca_pem: dir.join("identity_ca.cert.pem"),
            identity_cert_pem: dir.join("cert.pem"),
            identity_key_pem: dir.join("key.pem"),
            permissions_ca_pem: dir.join("permissions_ca.cert.pem"),
            governance_p7s: dir.join("governance.p7s"),
            permissions_p7s: dir.join("permissions.p7s"),
        };
        Self::from_files(&cfg, participant_guid)
    }

    /// C7 — "secure by default" env entry point. Loads an enclave from
    /// `ZERODDS_SECURITY_DIR`; the domain comes from `ROS_DOMAIN_ID`
    /// (default 0). Returns `Ok(None)` when `ZERODDS_SECURITY_DIR` is unset,
    /// so a launch path can opt into security with a single env var:
    ///
    /// ```text
    /// export ZERODDS_SECURITY_DIR=$ROS_SECURITY_KEYSTORE/enclaves/talker
    /// export ROS_DOMAIN_ID=42
    /// ```
    ///
    /// # Errors
    /// Propagates [`Self::from_enclave_dir`] errors when the dir is set but
    /// the material is missing or invalid.
    pub fn from_env(participant_guid: [u8; 16]) -> Result<Option<Self>, SecurityProfileError> {
        let dir = match std::env::var("ZERODDS_SECURITY_DIR") {
            Ok(d) if !d.is_empty() => d,
            _ => return Ok(None),
        };
        let domain_id = std::env::var("ROS_DOMAIN_ID")
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);
        Self::from_enclave_dir(dir, domain_id, participant_guid).map(Some)
    }
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
            identity_ca_pem: PathBuf::from("/zerodds/_does_not_exist_ca.pem"),
            identity_cert_pem: PathBuf::from("/zerodds/_does_not_exist_cert.pem"),
            identity_key_pem: PathBuf::from("/zerodds/_does_not_exist_key.pem"),
            permissions_ca_pem: PathBuf::from("/zerodds/_does_not_exist_pca.pem"),
            governance_p7s: PathBuf::from("/zerodds/_does_not_exist_gov.p7s"),
            permissions_p7s: PathBuf::from("/zerodds/_does_not_exist_perm.p7s"),
        };
        match SecurityProfile::from_files(&cfg, [0u8; 16]) {
            Err(SecurityProfileError::Io { .. }) => {}
            Err(e) => panic!("expected Io error, got: {e:?}"),
            Ok(_) => panic!("expected Err, got Ok"),
        }
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

    #[test]
    fn from_env_unset_returns_none() {
        // Skip if the var happens to be set in this environment.
        if std::env::var("ZERODDS_SECURITY_DIR").is_ok() {
            return;
        }
        match SecurityProfile::from_env([0u8; 16]) {
            Ok(None) => {}
            other => panic!("expected Ok(None) when ZERODDS_SECURITY_DIR unset, got {other:?}"),
        }
    }
}
