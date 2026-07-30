// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-Security 1.2 C-FFI.
//!
//! This layer mirrors the vendor practice for security setup:
//! six path setters + one `runtime_create_secure(domain, config)`.
//! Internally all paths are read, the CMS-signed
//! governance/permissions are verified, and a
//! [`zerodds_security_runtime::SharedSecurityGate`] is built and attached to
//! `RuntimeConfig.security`.
//!
//! # Memory model
//!
//! `zerodds_security_config_create()` returns an opaque pointer; the
//! caller must pair `zerodds_security_config_destroy()`. The 6 setters
//! accept null-terminated C strings; on NULL/empty string the
//! respective path is cleared. Setting again overwrites.
//!
//! `zerodds_runtime_create_secure(domain, cfg)` does **not** consume `cfg` —
//! the caller must still free `cfg` itself.
//!
//! # Error diagnosis
//!
//! On a NULL return the FFI prints an error line to stderr —
//! analogous to `zerodds_runtime_create`. The vendor practice in FastDDS/RTI/
//! Cyclone is identical (all three print setup errors to stderr).

extern crate alloc;

use core::ffi::c_char;
use core::ptr;
use std::ffi::CStr;
use std::path::PathBuf;
use std::sync::Arc;

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
use zerodds_security_runtime::{SecurityProfile, SecurityProfileConfig};

use crate::{ZeroDdsRuntime, random_guid_prefix};

/// Opaque builder for DDS-Security setup. All 6 path fields
/// must be set before `zerodds_runtime_create_secure` may be called
/// — otherwise NULL return + stderr diag.
#[derive(Debug, Default)]
pub struct ZeroDdsSecurityConfig {
    /// PEM bundle of the identity CA (trust anchors for remote certs).
    pub identity_ca_path: Option<PathBuf>,
    /// PEM with the identity cert of the local participant.
    pub identity_cert_path: Option<PathBuf>,
    /// PKCS#8 PEM private key of the local participant.
    pub identity_key_path: Option<PathBuf>,
    /// PEM bundle of the permissions CA (signs governance/permissions).
    pub permissions_ca_path: Option<PathBuf>,
    /// CMS-signed governance XML.
    pub governance_path: Option<PathBuf>,
    /// CMS-signed permissions XML.
    pub permissions_path: Option<PathBuf>,
}

impl ZeroDdsSecurityConfig {
    /// Turns the builder into a [`SecurityProfileConfig`]. Returns
    /// `Err(missing-field-name)` if a path is missing.
    fn try_to_profile_cfg(&self, domain_id: u32) -> Result<SecurityProfileConfig, &'static str> {
        Ok(SecurityProfileConfig {
            domain_id,
            identity_ca_pem: self
                .identity_ca_path
                .clone()
                .ok_or("identity_ca_path not set")?,
            identity_cert_pem: self
                .identity_cert_path
                .clone()
                .ok_or("identity_cert_path not set")?,
            identity_key_pem: self
                .identity_key_path
                .clone()
                .ok_or("identity_key_path not set")?,
            permissions_ca_pem: self
                .permissions_ca_path
                .clone()
                .ok_or("permissions_ca_path not set")?,
            governance_p7s: self
                .governance_path
                .clone()
                .ok_or("governance_path not set")?,
            permissions_p7s: self
                .permissions_path
                .clone()
                .ok_or("permissions_path not set")?,
        })
    }
}

// ============================================================================
// Lifecycle
// ============================================================================

/// Creates an empty security config builder.
///
/// # Safety
/// The return value is Box::into_raw — the caller owns it + must
/// pair `zerodds_security_config_destroy`.
#[unsafe(no_mangle)]
pub extern "C" fn zerodds_security_config_create() -> *mut ZeroDdsSecurityConfig {
    Box::into_raw(Box::new(ZeroDdsSecurityConfig::default()))
}

/// Destroys the config builder. NULL-safe.
///
/// # Safety
/// `cfg` must come from `zerodds_security_config_create` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_security_config_destroy(cfg: *mut ZeroDdsSecurityConfig) {
    if cfg.is_null() {
        return;
    }
    // SAFETY: cfg from zerodds_security_config_create (Box::into_raw).
    drop(unsafe { Box::from_raw(cfg) });
}

// ============================================================================
// Setter
// ============================================================================
//
// All setters take an optional C string. NULL/empty → clear the
// field. Strings are decoded as UTF-8; non-UTF-8 is
// rejected (return -1).
//
// Return convention:
//  *  0 = OK
//  * -1 = `cfg` NULL OR non-UTF-8 path
//
// No string-lifetime assumptions — the setter copies immediately.

fn set_path_field(
    cfg: *mut ZeroDdsSecurityConfig,
    path: *const c_char,
    field: fn(&mut ZeroDdsSecurityConfig) -> &mut Option<PathBuf>,
) -> i32 {
    if cfg.is_null() {
        eprintln!("zerodds_security_config_set_*: cfg is NULL");
        return -1;
    }
    // SAFETY: cfg per the call contract from _create (valid + not NULL).
    let c = unsafe { &mut *cfg };
    if path.is_null() {
        *field(c) = None;
        return 0;
    }
    // SAFETY: path is null-terminated per the FFI contract.
    let bytes = unsafe { CStr::from_ptr(path) };
    let s = match bytes.to_str() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("zerodds_security_config_set_*: path non-UTF-8: {e}");
            return -1;
        }
    };
    if s.is_empty() {
        *field(c) = None;
    } else {
        *field(c) = Some(PathBuf::from(s));
    }
    0
}

/// Setter `identity_ca_path` (PEM bundle, trust anchors).
///
/// # Safety
/// `cfg` from `zerodds_security_config_create`, `path` null-terminated or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_security_set_identity_ca_path(
    cfg: *mut ZeroDdsSecurityConfig,
    path: *const c_char,
) -> i32 {
    set_path_field(cfg, path, |c| &mut c.identity_ca_path)
}

/// Setter `identity_cert_path` (PEM, local identity cert).
///
/// # Safety
/// See [`zerodds_security_set_identity_ca_path`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_security_set_identity_cert_path(
    cfg: *mut ZeroDdsSecurityConfig,
    path: *const c_char,
) -> i32 {
    set_path_field(cfg, path, |c| &mut c.identity_cert_path)
}

/// Setter `identity_key_path` (PKCS#8 PEM private key).
///
/// # Safety
/// See [`zerodds_security_set_identity_ca_path`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_security_set_private_key_path(
    cfg: *mut ZeroDdsSecurityConfig,
    path: *const c_char,
) -> i32 {
    set_path_field(cfg, path, |c| &mut c.identity_key_path)
}

/// Setter `permissions_ca_path` (PEM bundle, often = identity_ca).
///
/// # Safety
/// See [`zerodds_security_set_identity_ca_path`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_security_set_permissions_ca_path(
    cfg: *mut ZeroDdsSecurityConfig,
    path: *const c_char,
) -> i32 {
    set_path_field(cfg, path, |c| &mut c.permissions_ca_path)
}

/// Setter `governance_path` (CMS-signed governance XML, `.p7s`).
///
/// # Safety
/// See [`zerodds_security_set_identity_ca_path`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_security_set_governance_path(
    cfg: *mut ZeroDdsSecurityConfig,
    path: *const c_char,
) -> i32 {
    set_path_field(cfg, path, |c| &mut c.governance_path)
}

/// Setter `permissions_path` (CMS-signed permissions XML, `.p7s`).
///
/// # Safety
/// See [`zerodds_security_set_identity_ca_path`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_security_set_permissions_path(
    cfg: *mut ZeroDdsSecurityConfig,
    path: *const c_char,
) -> i32 {
    set_path_field(cfg, path, |c| &mut c.permissions_path)
}

// ============================================================================
// Runtime create with security
// ============================================================================

/// Creates a ZeroDDS runtime with DDS-Security 1.2 active.
///
/// `cfg` must have all 6 paths set; PKI + CMS verify + governance/
/// permissions parsing run synchronously on the call. On error in
/// one of the steps: NULL return + stderr diag, **but** `cfg`
/// stays unchanged (no auto-destroy).
///
/// # Safety
/// `cfg` from `zerodds_security_config_create` (or NULL). `cfg` must
/// not be mutated concurrently from another thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_create_secure(
    domain_id: u32,
    cfg: *const ZeroDdsSecurityConfig,
) -> *mut ZeroDdsRuntime {
    if cfg.is_null() {
        eprintln!("zerodds_runtime_create_secure: cfg is NULL");
        return ptr::null_mut();
    }
    // SAFETY: cfg per the FFI contract from _create + not further mutated.
    let cfg_ref: &ZeroDdsSecurityConfig = unsafe { &*cfg };

    let profile_cfg = match cfg_ref.try_to_profile_cfg(domain_id) {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("zerodds_runtime_create_secure: {msg}");
            return ptr::null_mut();
        }
    };

    let prefix = random_guid_prefix();
    // The PKI plugin needs a 16-byte participant GUID. The RTPS GuidPrefix is
    // 12 bytes; we pad with the builtin EntityId suffix
    // 0x000001C1 (PARTICIPANT) — exactly the format that the
    // builtin PKI plugins of other vendors also use.
    let mut participant_guid = [0u8; 16];
    participant_guid[..12].copy_from_slice(&prefix.0);
    participant_guid[12..].copy_from_slice(&[0x00, 0x00, 0x01, 0xC1]);

    let profile = match SecurityProfile::from_files(&profile_cfg, participant_guid) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("zerodds_runtime_create_secure: {e}");
            return ptr::null_mut();
        }
    };

    // General FFI path: XCDR2-first writer encap (matches the plain path + apps).
    finish_secure_runtime(domain_id, profile, secured_data_representation_offer())
}

/// Creates a ZeroDDS runtime with DDS-Security from an **SROS2 enclave**
/// (C7 "secure by default"). Reads `ZERODDS_SECURITY_DIR` (enclave directory)
/// and `ROS_DOMAIN_ID`, and builds the `SecurityProfile` in one call — no
/// per-path setter ceremony.
///
/// Return: NULL if `ZERODDS_SECURITY_DIR` is not set (the caller then falls
/// back to the plain path) OR on a load/verify error (stderr diag);
/// otherwise the secured runtime. `domain_id` does not override `ROS_DOMAIN_ID` —
/// it is the explicit value the runtime starts with.
///
/// # Safety
/// No pointer arguments; always callable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_runtime_create_secure_from_env(
    domain_id: u32,
) -> *mut ZeroDdsRuntime {
    let prefix = random_guid_prefix();
    let mut participant_guid = [0u8; 16];
    participant_guid[..12].copy_from_slice(&prefix.0);
    participant_guid[12..].copy_from_slice(&[0x00, 0x00, 0x01, 0xC1]);

    match SecurityProfile::from_env(participant_guid) {
        // SROS2/rmw path: keep `ros_defaults()` XCDR1-first writer encap so
        // rmw_zerodds stays wire-compatible with default rmw readers.
        Ok(Some(profile)) => finish_secure_runtime(
            domain_id,
            profile,
            RuntimeConfig::ros_defaults().data_representation_offer,
        ),
        Ok(None) => {
            eprintln!(
                "zerodds_runtime_create_secure_from_env: ZERODDS_SECURITY_DIR not set — \
                 no enclave to load"
            );
            ptr::null_mut()
        }
        Err(e) => {
            eprintln!("zerodds_runtime_create_secure_from_env: {e}");
            ptr::null_mut()
        }
    }
}

/// Data-representation offer for the general [`zerodds_runtime_create_secure`]
/// path (roundtrip bench, codegen backends, direct FFI apps).
///
/// XCDR2 is FIRST — the byte-oriented FFI writer derives its emitted
/// encapsulation header from the offer's first element, and these apps
/// serialize XCDR2 bodies, matching the plain `zerodds_runtime_create` default
/// (offer `[XCDR2]`). XCDR1 stays in the SET so the reader still accepts foreign
/// XCDR1 writers (Cyclone / rmw_*) config-free — the cross-vendor acceptance the
/// secured path needs. Kept separate from the SROS2/rmw `from_env` path, which
/// keeps `ros_defaults()` (XCDR1-first) so rmw_zerodds writers stay
/// wire-compatible with default rmw readers that offer XCDR1 only.
fn secured_data_representation_offer() -> alloc::vec::Vec<i16> {
    use zerodds_rtps::publication_data::data_representation as dr;
    alloc::vec![dr::XCDR2, dr::XCDR]
}

/// Shared tail of [`zerodds_runtime_create_secure`] +
/// [`zerodds_runtime_create_secure_from_env`]: joinability gate, start the runtime with
/// an identity-adjusted GUID prefix, enable the auth builtins.
///
/// `data_rep_offer` is the runtime data-representation offer. It is passed by
/// the caller (rather than hardcoded) because the two entry points need
/// different offer ORDERING: the general FFI path emits XCDR2-first (see
/// [`secured_data_representation_offer`]), the SROS2/rmw `from_env` path keeps
/// `ros_defaults()`' XCDR1-first ordering for rmw wire compatibility.
fn finish_secure_runtime(
    domain_id: u32,
    profile: SecurityProfile,
    data_rep_offer: alloc::vec::Vec<i16>,
) -> *mut ZeroDdsRuntime {
    // DDS-Security §8.4.2.9.3 `check_create_participant`: consult BOTH governance
    // and permissions. With `enable_join_access_control=TRUE` the participant may
    // create iff its permissions grant an `<allow_rule>` whose `<domains>` covers
    // this domain. A fully access-controlled governance is NOT un-joinable per se
    // — it is joinable by a participant whose permissions grant the domain
    // (Cyclone DDS + Fast DDS both join such a governance; SROS2 full-lockdown
    // relies on it). The earlier gate checked governance topology ONLY and denied
    // unconditionally — a spec bug that blocked ZeroDDS from every fully-locked
    // secured domain (verified cross-vendor on codepit, 2026-06-14).
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if !profile
        .governance
        .check_create_participant(&profile.permissions, domain_id, now_secs)
    {
        eprintln!(
            "zerodds_runtime_create_secure(domain_id={domain_id}): check_create_participant \
             denied (DDS-Security §8.4.2.9.3: join access control enabled and no permissions \
             grant covers this domain, or no governance domain rule) — participant create denied"
        );
        return ptr::null_mut();
    }

    // DDS-Security §9.3.3: the runtime MUST run with the identity-adjusted
    // GUID prefix (not the random candidate), so that the SPDP beacon,
    // handshake c.pdata and all entity GUIDs are consistent + cross-vendor
    // accepted.
    let mut adjusted_prefix_bytes = [0u8; 12];
    adjusted_prefix_bytes.copy_from_slice(&profile.adjusted_participant_guid[..12]);
    let prefix = zerodds_rtps::wire_types::GuidPrefix::from_bytes(adjusted_prefix_bytes);

    // A5: this is the secured `rmw_zerodds` entry point (SROS2 enclave), so it
    // gets the same ROS-2 out-of-the-box profile as the plain path — the reader
    // offers XCDR1+XCDR2 to match rmw_cyclonedds/rmw_fastrtps writers config-free.
    // `security_guid_prefix` MUST match the identity-derived `prefix` we pass to
    // `DcpsRuntime::start`, or the §9.3.3 GUID-prefix guard rejects the runtime.
    // `data_rep_offer` param: general FFI path passes XCDR2-first (honest encap
    // for the app's XCDR2 bodies — old ros_defaults() XCDR1-first stamped an
    // XCDR1 header on XCDR2 bodies → typed reader misparse → 0 secured samples);
    // SROS2/rmw `from_env` keeps ros_defaults() XCDR1-first for rmw compatibility.
    let rt_cfg = RuntimeConfig {
        security: Some(Arc::clone(&profile.gate)),
        security_guid_prefix: Some(prefix),
        data_representation_offer: data_rep_offer,
        ..RuntimeConfig::ros_defaults()
    };

    let rt = match DcpsRuntime::start(domain_id as i32, prefix, rt_cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("zerodds_runtime_create_secure(domain_id={domain_id}): {e:?}");
            return ptr::null_mut();
        }
    };

    // FU2: enable the auth handshake driver (gap 4/5/7). The stack
    // announces the stateless/volatile-secure endpoints + the local
    // IdentityToken in the SPDP beacon and drives the PKI handshake with
    // every discovered peer up to the shared secret. `profile.pki` is
    // the same plugin instance that hangs on the crypto gate as the
    // `SharedSecretProvider` (gap 1) — the derived secret
    // is resolvable there.
    //
    // NOTE: this step AUTHENTICATES the peers (identity
    // handshake) and unlocks secured discovery. The
    // secured-DATA key distribution (crypto-token exchange over the
    // Kx-protected VolatileSecure channel, gap 6) is the still-open
    // follow-up step; until then the gate protects user DATA via the
    // existing `data_protection_kind` path.
    rt.enable_security_builtins_with_auth(
        zerodds_rtps::wire_types::VendorId::ZERODDS,
        profile.pki.clone(),
        profile.identity_handle,
    );

    Box::into_raw(Box::new(ZeroDdsRuntime { rt, _shutdown: () }))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn secured_runtime_offers_xcdr2_first() {
        // Regression: `finish_secure_runtime` used `RuntimeConfig::ros_defaults()`,
        // whose offer is `[XCDR1, XCDR2]`. The FFI writer derives its encap header
        // from the offer's FIRST element, so the secured writer stamped an XCDR1
        // (0x0001) encapsulation on XCDR2 bodies → readers reported
        // representation=XCDR1 → typed decode misparsed → secured
        // zerodds<->zerodds delivered 0 samples (plain path, XCDR2-first, worked).
        use zerodds_rtps::publication_data::data_representation as dr;
        let offer = secured_data_representation_offer();
        assert_eq!(
            offer.first().copied(),
            Some(dr::XCDR2),
            "secured writer must emit an XCDR2-first encapsulation header \
             (XCDR1-first stamps the wrong representation on XCDR2 bodies → 0 samples)"
        );
        assert!(
            offer.contains(&dr::XCDR),
            "the offer SET must still advertise XCDR1 for cross-vendor / ROS reader acceptance"
        );
        // The SROS2/rmw `from_env` path deliberately keeps `ros_defaults()`
        // XCDR1-first ordering (rmw wire compatibility) — the two paths differ
        // on purpose. Guard that they really are ordered differently.
        let ros = zerodds_dcps::runtime::RuntimeConfig::ros_defaults().data_representation_offer;
        assert_eq!(
            ros.first().copied(),
            Some(dr::XCDR),
            "ros_defaults() is expected XCDR1-first; the general secured path overrides it"
        );
        assert_ne!(
            offer.first().copied(),
            ros.first().copied(),
            "general vs SROS2 secured paths must keep their distinct writer encap ordering"
        );
    }

    #[test]
    fn config_create_destroy_roundtrip() {
        let cfg = zerodds_security_config_create();
        assert!(!cfg.is_null());
        // SAFETY: cfg from config_create, freed exactly once.
        unsafe { zerodds_security_config_destroy(cfg) };
    }

    #[test]
    fn null_setter_clears_path() {
        let cfg = zerodds_security_config_create();
        let path = std::ffi::CString::new("/tmp/foo.pem").unwrap();
        // SAFETY: cfg valid from config_create; path.as_ptr() lives until the end of the function.
        let rc = unsafe { zerodds_security_set_identity_ca_path(cfg, path.as_ptr()) };
        assert_eq!(rc, 0);
        // SAFETY: cfg valid and non-null — deref reads the set field.
        unsafe {
            assert_eq!((*cfg).identity_ca_path, Some(PathBuf::from("/tmp/foo.pem")));
        }
        // SAFETY: cfg valid; a NULL ptr clears the path (NULL-safe setter).
        let rc = unsafe { zerodds_security_set_identity_ca_path(cfg, ptr::null()) };
        assert_eq!(rc, 0);
        // SAFETY: cfg valid and non-null — deref reads the cleared field.
        unsafe { assert_eq!((*cfg).identity_ca_path, None) };
        // SAFETY: cfg from config_create, freed exactly once.
        unsafe { zerodds_security_config_destroy(cfg) };
    }

    #[test]
    fn missing_path_yields_null_runtime() {
        let cfg = zerodds_security_config_create();
        // Only one of the 6 fields set — `try_to_profile_cfg` must complain.
        let path = std::ffi::CString::new("/tmp/identity_ca.pem").unwrap();
        // SAFETY: cfg valid from config_create; path.as_ptr() lives until the end of the function.
        unsafe {
            zerodds_security_set_identity_ca_path(cfg, path.as_ptr());
        }
        // SAFETY: cfg valid; an incomplete config must return a NULL runtime.
        let rt = unsafe { zerodds_runtime_create_secure(0, cfg) };
        assert!(rt.is_null());
        // SAFETY: cfg from config_create, freed exactly once.
        unsafe { zerodds_security_config_destroy(cfg) };
    }

    #[test]
    fn secure_from_env_null_when_dir_unset() {
        // C7: without ZERODDS_SECURITY_DIR the env variant returns NULL, so that
        // the caller (rmw-shim) falls back to the plain path.
        if std::env::var("ZERODDS_SECURITY_DIR").is_ok() {
            return;
        }
        // SAFETY: no pointer arguments; always callable.
        let rt = unsafe { zerodds_runtime_create_secure_from_env(0) };
        assert!(rt.is_null());
    }

    #[test]
    fn null_cfg_yields_null_runtime() {
        // SAFETY: NULL cfg is explicitly allowed and must return a NULL runtime.
        let rt = unsafe { zerodds_runtime_create_secure(0, ptr::null()) };
        assert!(rt.is_null());
    }

    #[test]
    fn six_setters_all_writeable() {
        let cfg = zerodds_security_config_create();
        let p = std::ffi::CString::new("/x").unwrap();
        for setter in [
            zerodds_security_set_identity_ca_path
                as unsafe extern "C" fn(*mut ZeroDdsSecurityConfig, *const c_char) -> i32,
            zerodds_security_set_identity_cert_path,
            zerodds_security_set_private_key_path,
            zerodds_security_set_permissions_ca_path,
            zerodds_security_set_governance_path,
            zerodds_security_set_permissions_path,
        ] {
            // SAFETY: cfg valid from config_create; p.as_ptr() lives until the end of the function.
            let rc = unsafe { setter(cfg, p.as_ptr()) };
            assert_eq!(rc, 0);
        }
        // SAFETY: cfg valid and non-null — deref reads the 6 set fields.
        unsafe {
            assert!((*cfg).identity_ca_path.is_some());
            assert!((*cfg).identity_cert_path.is_some());
            assert!((*cfg).identity_key_path.is_some());
            assert!((*cfg).permissions_ca_path.is_some());
            assert!((*cfg).governance_path.is_some());
            assert!((*cfg).permissions_path.is_some());
        }
        // SAFETY: cfg from config_create, freed exactly once.
        unsafe { zerodds_security_config_destroy(cfg) };
    }
}
