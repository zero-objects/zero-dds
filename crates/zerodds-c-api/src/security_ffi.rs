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
    /// Plugins of the application (AccessControl, Logging, DataTagging) that
    /// replace the builtin ones, see `plugin_ffi`.
    pub plugins: crate::plugin_ffi::ApplicationPlugins,
}

impl ZeroDdsSecurityConfig {
    /// Turns the builder into a [`SecurityProfileConfig`]. Returns
    /// `Err(missing-field-name)` if a path is missing.
    fn try_to_profile_cfg(&self, domain_id: u32) -> Result<SecurityProfileConfig, &'static str> {
        // The vendor-form settings are `security.*` keys of the configuration
        // (files over the environment, as every runtime setting).
        let source = zerodds_dcps::runtime::ConfigSource::from_standard_paths()
            .map_err(|_| "the ZeroDDS configuration does not load")?;
        self.try_to_profile_cfg_with_env(domain_id, &|name| source.var(name))
    }

    /// [`Self::try_to_profile_cfg`] with the environment given. The three
    /// vendor-form settings (`ZERODDS_SEC_KX_KEY_FALLBACK`,
    /// `ZERODDS_SEC_COMPATIBILITY_CHECK`, `ZERODDS_SEC_ALGO_NAME_NUL`, `ZERODDS_SEC_SUBJECT_NAME_FORM`) come
    /// from it, as they do for the enclave directory
    /// (`SecurityProfile::from_enclave_dir`): this path had the defaults only,
    /// so a variable set for an application that creates its runtime from the
    /// six paths had no effect.
    fn try_to_profile_cfg_with_env(
        &self,
        domain_id: u32,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Result<SecurityProfileConfig, &'static str> {
        let forms = zerodds_security_runtime::vendor_form_settings_from_env(env)
            .map_err(|_| "a ZERODDS_SEC_* vendor-form setting has a value that is not allowed")?;
        Ok(SecurityProfileConfig {
            domain_id,
            identity_ca_pem: self
                .identity_ca_path
                .clone()
                .ok_or("identity_ca_path not set")?
                .into(),
            identity_cert_pem: self
                .identity_cert_path
                .clone()
                .ok_or("identity_cert_path not set")?
                .into(),
            identity_key_pem: self
                .identity_key_path
                .clone()
                .ok_or("identity_key_path not set")?
                .into(),
            permissions_ca_pem: self
                .permissions_ca_path
                .clone()
                .ok_or("permissions_ca_path not set")?
                .into(),
            governance_p7s: self
                .governance_path
                .clone()
                .ok_or("governance_path not set")?
                .into(),
            permissions_p7s: self
                .permissions_path
                .clone()
                .ok_or("permissions_path not set")?
                .into(),
            identity_key_password: None,
            identity_key_password_reading: Default::default(),
            symmetric_cipher_bits: 256,
            max_blocks_per_session: None,
            partition_matching: zerodds_security_runtime::PartitionMatching::Subset,
            plaintext_loopback: false,
            compatibility_checks: forms.compatibility_checks,
            kx_key_fallback: forms.kx_key_fallback,
            algo_name_nul: forms.algo_name_nul,
            algorithm_names: forms.algorithm_names,
            subject_name_form: forms.subject_name_form,
            origin_authentication_fallback: forms.origin_authentication_fallback,
            identity_crl: None,
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

    // An Authentication or a Cryptographic plugin of the application replaces
    // the builtin PKI and AES-GCM plugins the files would build.
    if cfg_ref.plugins.authentication.is_some() || cfg_ref.plugins.cryptographic.is_some() {
        return create_with_application_plugins(domain_id, cfg_ref);
    }

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
    finish_secure_runtime(
        domain_id,
        profile.into(),
        secured_data_representation_offer(),
        &cfg_ref.plugins,
    )
}

/// The SROS2 enclave directory the runtime configuration names:
/// `security.enclave_dir`, env `ZERODDS_SECURITY_DIR`, over the standard
/// layers. `Ok(None)` when no layer sets it.
///
/// # Errors
/// An invalid or unreadable configuration, with its diagnostic.
pub fn configured_enclave_dir() -> Result<Option<std::path::PathBuf>, String> {
    crate::ffi_runtime_config(alloc::vec::Vec::new()).map(|config| config.security_enclave_dir)
}

/// Creates a ZeroDDS runtime with DDS-Security from an **SROS2 enclave**
/// (C7 "secure by default"). The enclave directory is the runtime setting
/// `security.enclave_dir` (env `ZERODDS_SECURITY_DIR`, or a file of the
/// standard layers), and the `SecurityProfile` is built in one call — no
/// per-path setter ceremony. `domain_id` is the domain of both the runtime
/// and the governance/permissions check.
///
/// Return: NULL if no enclave directory is configured (the caller then falls
/// back to the plain path) OR on a configuration, load or verify error
/// (stderr diag); otherwise the secured runtime.
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

    let dir = match configured_enclave_dir() {
        Ok(Some(dir)) => dir,
        Ok(None) => {
            eprintln!(
                "zerodds_runtime_create_secure_from_env: security.enclave_dir \
                 (ZERODDS_SECURITY_DIR) not set — no enclave to load"
            );
            return ptr::null_mut();
        }
        Err(e) => {
            eprintln!("zerodds_runtime_create_secure_from_env: invalid configuration: {e}");
            return ptr::null_mut();
        }
    };
    match SecurityProfile::from_enclave_dir(&dir, domain_id, participant_guid) {
        // SROS2/rmw path: keep `ros_defaults()` XCDR1-first writer encap so
        // rmw_zerodds stays wire-compatible with default rmw readers.
        Ok(profile) => finish_secure_runtime(
            domain_id,
            profile.into(),
            RuntimeConfig::ros_defaults().data_representation_offer,
            &crate::plugin_ffi::ApplicationPlugins::default(),
        ),
        Err(e) => {
            eprintln!(
                "zerodds_runtime_create_secure_from_env: enclave {}: {e}",
                dir.display()
            );
            ptr::null_mut()
        }
    }
}

/// The secured `rmw_zerodds` runtime from layers the caller loaded (the RMW
/// reads `--config`/`--set` from its process's command line): `Ok(None)`
/// when the configuration names no enclave (`security.enclave_dir`), the
/// plain path then applies. A Rust helper, not a C ABI.
///
/// # Errors
/// An invalid configuration, an enclave that does not load, or a refused
/// start (the reason is on stderr, as for the C entry points).
pub fn runtime_create_secure_ros_from_source(
    domain_id: u32,
    source: &zerodds_dcps::runtime::ConfigSource,
    rcl_enclave: Option<&std::path::Path>,
) -> Result<Option<*mut ZeroDdsRuntime>, String> {
    let config = crate::ffi_config_from_source(
        source,
        RuntimeConfig::ros_defaults().data_representation_offer,
    )?;
    // The enclave rcl resolved (`ROS_SECURITY_KEYSTORE`/`enclaves/<name>`,
    // `rmw_security_options_t::security_root_path`) or the configuration's
    // `security.enclave_dir`; two different ones is a contradiction.
    let dir = match (rcl_enclave, config.security_enclave_dir.as_deref()) {
        (Some(rcl), Some(cfg)) if rcl != cfg => {
            return Err(format!(
                "rcl names the enclave {} and security.enclave_dir names {}; name one",
                rcl.display(),
                cfg.display()
            ));
        }
        (Some(rcl), _) => rcl.to_path_buf(),
        (None, Some(cfg)) => cfg.to_path_buf(),
        (None, None) => return Ok(None),
    };
    let prefix = random_guid_prefix();
    let mut participant_guid = [0u8; 16];
    participant_guid[..12].copy_from_slice(&prefix.0);
    participant_guid[12..].copy_from_slice(&[0x00, 0x00, 0x01, 0xC1]);
    let profile = SecurityProfile::from_enclave_dir(&dir, domain_id, participant_guid)
        .map_err(|e| format!("enclave {}: {e}", dir.display()))?;
    let rt = finish_secure_runtime_with(
        domain_id,
        profile.into(),
        config,
        &crate::plugin_ffi::ApplicationPlugins::default(),
    );
    if rt.is_null() {
        return Err(format!(
            "secured runtime for domain {domain_id} not started"
        ));
    }
    Ok(Some(rt))
}

/// Data-representation offer for the general [`zerodds_runtime_create_secure`]
/// path (roundtrip bench, codegen backends, direct FFI apps).
///
/// XCDR2 is FIRST — the byte-oriented FFI writer derives its emitted
/// encapsulation header from the offer's first element, and these apps
/// serialize XCDR2 bodies. XCDR1 stays in the SET so the reader still accepts
/// foreign XCDR1 writers (Cyclone / rmw_*) config-free — the cross-vendor
/// acceptance the secured path needs. Kept separate from the SROS2/rmw
/// `from_env` path, which keeps `ros_defaults()` (XCDR1-first) so rmw_zerodds
/// writers stay wire-compatible with default rmw readers that offer XCDR1 only.
///
/// It is [`crate::ffi_data_representation_offer`] and not a second copy of
/// it. When this function held its own copy, the plain
/// `zerodds_runtime_create` was on `RuntimeConfig::default()` instead —
/// and stayed there when that default became XCDR1-first, which stamped
/// `0x0001` on the FFI's XCDR2 bodies and took plain zerodds↔zerodds to
/// zero samples while the secured path measured 13/13. One function is
/// what keeps the secured and the plain path from parting again.
fn secured_data_representation_offer() -> alloc::vec::Vec<i16> {
    crate::ffi_data_representation_offer()
}

/// `zerodds_runtime_create_secure` with an Authentication plugin of the
/// application, and a Cryptographic plugin of the application or the builtin
/// one. The governance is read and verified from `governance_path` and
/// `permissions_ca_path`; the plugin validates the identity, with the paths
/// that are set as the properties of Table 50 (`file:` URIs). The builtin
/// AccessControl plugin needs the identity certificate of the builtin PKI
/// plugin, so with these plugins the AccessControl plugin is the application's
/// or there is none (the governance decides).
fn create_with_application_plugins(
    domain_id: u32,
    cfg: &ZeroDdsSecurityConfig,
) -> *mut ZeroDdsRuntime {
    use zerodds_security::authentication::{AuthenticationPlugin, ValidationResult};
    use zerodds_security::properties::{Property, PropertyList};
    use zerodds_security_runtime::Material;
    let fail = |message: &str| {
        eprintln!("zerodds_runtime_create_secure: {message}");
        ptr::null_mut()
    };
    let Some(auth_handle) = cfg.plugins.authentication.as_ref() else {
        return fail(
            "a Cryptographic plugin of the application needs an Authentication plugin of \
             the application: it gets the SharedSecret from the plugin it agrees with",
        );
    };
    let (Some(governance), Some(permissions_ca)) = (
        cfg.governance_path.as_ref(),
        cfg.permissions_ca_path.as_ref(),
    ) else {
        return fail("governance_path and permissions_ca_path must be set");
    };
    let forms = match zerodds_security_runtime::vendor_form_settings_from_env(&|name| {
        std::env::var(name).ok()
    }) {
        Ok(forms) => forms,
        Err(_) => {
            return fail("a ZERODDS_SEC_* vendor-form setting has a value that is not allowed");
        }
    };
    let unused = || Material::File(PathBuf::new());
    let profile_cfg = SecurityProfileConfig {
        domain_id,
        identity_ca_pem: unused(),
        identity_cert_pem: unused(),
        identity_key_pem: unused(),
        permissions_ca_pem: permissions_ca.clone().into(),
        governance_p7s: governance.clone().into(),
        permissions_p7s: unused(),
        identity_key_password: None,
        identity_key_password_reading: Default::default(),
        symmetric_cipher_bits: 256,
        max_blocks_per_session: None,
        partition_matching: zerodds_security_runtime::PartitionMatching::Subset,
        plaintext_loopback: false,
        compatibility_checks: forms.compatibility_checks,
        kx_key_fallback: forms.kx_key_fallback,
        algo_name_nul: forms.algo_name_nul,
        algorithm_names: forms.algorithm_names,
        subject_name_form: forms.subject_name_form,
        origin_authentication_fallback: forms.origin_authentication_fallback,
        identity_crl: None,
    };
    let (governance, _governance_signed, _permissions_ca) =
        match zerodds_security_runtime::governance_from_files(&profile_cfg) {
            Ok(g) => g,
            Err(e) => return fail(&format!("{e}")),
        };

    let prefix = random_guid_prefix();
    let mut candidate = [0u8; 16];
    candidate[..12].copy_from_slice(&prefix.0);
    candidate[12..].copy_from_slice(&[0x00, 0x00, 0x01, 0xC1]);

    // `validate_local_identity` with the settings of the configuration as the
    // properties of Table 50.
    let mut properties = PropertyList::new();
    for (name, path) in [
        ("dds.sec.auth.identity_ca", &cfg.identity_ca_path),
        ("dds.sec.auth.identity_certificate", &cfg.identity_cert_path),
        ("dds.sec.auth.private_key", &cfg.identity_key_path),
        ("dds.sec.access.permissions_ca", &cfg.permissions_ca_path),
        ("dds.sec.access.governance", &cfg.governance_path),
        ("dds.sec.access.permissions", &cfg.permissions_path),
    ] {
        if let Some(path) = path {
            properties.set(Property::local(name, format!("file:{}", path.display())));
        }
    }
    let mut auth = match auth_handle.instantiate() {
        Ok(a) => a,
        Err(e) => return fail(&format!("Authentication plugin of the application: {e}")),
    };
    let validated = loop {
        match auth.validate_local_identity(domain_id, &properties, candidate) {
            Ok(v) if v.result == ValidationResult::PendingRetry => {
                std::thread::sleep(zerodds_security_runtime::profile::LOCAL_VALIDATION_RETRY_DELAY);
            }
            Ok(v) => break v,
            Err(e) => return fail(&format!("validate_local_identity of the plugin: {e}")),
        }
    };
    if !validated.result.is_success() {
        return fail("validate_local_identity of the plugin of the application failed");
    }

    let crypto: Box<dyn zerodds_security::cryptographic::CryptographicPluginInterfaces> =
        match cfg.plugins.cryptographic.as_ref() {
            Some(handle) => Box::new(handle.instantiate()),
            None => Box::new(zerodds_security_runtime::builtin_crypto_plugin(
                &profile_cfg,
                &governance,
                auth_handle.secrets(),
            )),
        };
    let gate = match zerodds_security_runtime::gate_with_crypto(&profile_cfg, &governance, crypto) {
        Ok(g) => g,
        Err(e) => return fail(&format!("{e}")),
    };
    let auth: Arc<std::sync::Mutex<dyn AuthenticationPlugin>> =
        Arc::new(std::sync::Mutex::new(auth));
    finish_secure_runtime(
        domain_id,
        Parts {
            gate,
            auth,
            identity: validated.identity,
            guid: validated.adjusted_participant_guid,
            governance,
            builtin: None,
        },
        secured_data_representation_offer(),
        &cfg.plugins,
    )
}

/// What the tail of the runtime creation needs of a security setup: the builtin
/// profile, or the pieces of a setup made with plugins of the application.
struct Parts {
    gate: Arc<zerodds_security_runtime::SharedSecurityGate>,
    auth: Arc<std::sync::Mutex<dyn zerodds_security::authentication::AuthenticationPlugin>>,
    identity: zerodds_security::authentication::IdentityHandle,
    guid: [u8; 16],
    governance: zerodds_security_runtime::Governance,
    /// The builtin profile, where the files made it: it arms the builtin
    /// AccessControl plugin.
    builtin: Option<SecurityProfile>,
}

impl From<SecurityProfile> for Parts {
    fn from(profile: SecurityProfile) -> Self {
        Self {
            gate: Arc::clone(&profile.gate),
            auth: profile.pki.clone(),
            identity: profile.identity_handle,
            guid: profile.adjusted_participant_guid,
            governance: profile.governance.clone(),
            builtin: Some(profile),
        }
    }
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
    profile: Parts,
    data_rep_offer: alloc::vec::Vec<i16>,
    plugins: &crate::plugin_ffi::ApplicationPlugins,
) -> *mut ZeroDdsRuntime {
    // Resolve and validate files before arming any security endpoint state.
    let base_config = match crate::ffi_runtime_config(data_rep_offer) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("zerodds_runtime_create_secure: invalid configuration: {error}");
            return ptr::null_mut();
        }
    };
    finish_secure_runtime_with(domain_id, profile, base_config, plugins)
}

/// [`finish_secure_runtime`] over a configuration the caller already loaded.
fn finish_secure_runtime_with(
    domain_id: u32,
    profile: Parts,
    base_config: RuntimeConfig,
    plugins: &crate::plugin_ffi::ApplicationPlugins,
) -> *mut ZeroDdsRuntime {
    // DDS-Security §9.4.2.9.3 `check_create_participant`: consult BOTH governance
    // and permissions. With `enable_join_access_control=TRUE` the participant may
    // create iff its permissions grant an `<allow_rule>` whose `<domains>` covers
    // this domain. A fully access-controlled governance is NOT un-joinable per se
    // — it is joinable by a participant whose permissions grant the domain
    // (Cyclone DDS + Fast DDS both join such a governance; SROS2 full-lockdown
    // relies on it). The earlier gate checked governance topology ONLY and denied
    // unconditionally — a spec bug that blocked ZeroDDS from every fully-locked
    // secured domain (verified cross-vendor on the Linux test host, 2026-06-14).
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let join_permitted = match &profile.builtin {
        Some(builtin) => {
            profile
                .governance
                .check_create_participant(&builtin.permissions, domain_id, now_secs)
        }
        // Plugins of the application: the domain needs a rule; whether the
        // participant may join is the AccessControl plugin's to say
        // (`check_create_participant` below), as the governance alone cannot.
        None => profile.governance.find_domain_rule(domain_id).is_some(),
    };
    if !join_permitted {
        eprintln!(
            "zerodds_runtime_create_secure(domain_id={domain_id}): check_create_participant \
             denied (DDS-Security §9.4.2.9.3: join access control enabled and no permissions \
             grant covers this domain, or no governance domain rule) — participant create denied"
        );
        return ptr::null_mut();
    }

    // DDS-Security §7.4.3: the runtime MUST run with the identity-adjusted
    // GUID prefix (not the random candidate), so that the SPDP beacon,
    // handshake c.pdata and all entity GUIDs are consistent + cross-vendor
    // accepted.
    let mut adjusted_prefix_bytes = [0u8; 12];
    adjusted_prefix_bytes.copy_from_slice(&profile.guid[..12]);
    let prefix = zerodds_rtps::wire_types::GuidPrefix::from_bytes(adjusted_prefix_bytes);

    // DDS-Security 1.2 §9.4.2.9.4 / §9.4.2.9.5: the gate carries the
    // governance, the profile carries the permissions, and the endpoint-create
    // check needs both. Without this the runtime would hold a gate that knows
    // a topic is access-controlled and has nothing to decide with, and every
    // `create_datawriter`/`create_datareader` on such a topic would be refused
    // as `NoPermissions`.
    if let Some(builtin) = &profile.builtin {
        if let Err(e) = zerodds_security_runtime::arm_endpoint_access_control(builtin) {
            eprintln!("zerodds_runtime_create_secure(domain_id={domain_id}): {e}");
            return core::ptr::null_mut();
        }
    }

    // A5: this is the secured `rmw_zerodds` entry point (SROS2 enclave), so it
    // gets the same ROS-2 out-of-the-box profile as the plain path — the reader
    // offers XCDR1+XCDR2 to match rmw_cyclonedds/rmw_fastrtps writers config-free.
    // `security_guid_prefix` MUST match the identity-derived `prefix` we pass to
    // `DcpsRuntime::start`, or the §7.4.3 GUID-prefix guard rejects the runtime.
    // `data_rep_offer` param: general FFI path passes XCDR2-first (honest encap
    // for the app's XCDR2 bodies — old ros_defaults() XCDR1-first stamped an
    // XCDR1 header on XCDR2 bodies → typed reader misparse → 0 secured samples);
    // SROS2/rmw `from_env` keeps ros_defaults() XCDR1-first for rmw compatibility.
    // The plugins of the application replace the builtin ones: the
    // AccessControl plugin in the gate (DDS-Security 1.2 §9.8.1 step 2:
    // `validate_local_permissions` first, whose handle is the local one),
    // the Logging and the DataTagging plugin in the runtime configuration.
    if let Some(handle) = plugins.access_control.as_ref() {
        if let Err(e) = install_access_control(domain_id, &profile, handle) {
            eprintln!("zerodds_runtime_create_secure(domain_id={domain_id}): {e}");
            return core::ptr::null_mut();
        }
    }
    let security_logger = plugins
        .logging
        .as_ref()
        .map(crate::plugin_ffi::logging_for_runtime)
        .or_else(|| base_config.security_logger.clone());
    let security_data_tagging = plugins
        .data_tagging
        .as_ref()
        .map(crate::plugin_ffi::data_tagging_for_runtime)
        .or_else(|| base_config.security_data_tagging.clone());
    let rt_cfg = RuntimeConfig {
        security: Some(Arc::clone(&profile.gate)),
        security_guid_prefix: Some(prefix),
        security_logger,
        security_data_tagging,
        ..base_config
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
        profile.auth.clone(),
        profile.identity,
    );

    Box::into_raw(Box::new(ZeroDdsRuntime { rt, _shutdown: () }))
}

/// Installs the AccessControl plugin of the application in the gate of
/// `profile`: `validate_local_permissions` (DDS-Security 1.2 §9.8.1 step 2)
/// gives the local handle.
fn install_access_control(
    domain_id: u32,
    profile: &Parts,
    handle: &crate::plugin_ffi::AccessControlHandle,
) -> Result<(), String> {
    use zerodds_security::access_control::{AccessControlPlugin, ParticipantRequest};
    let mut plugin = handle
        .instantiate()
        .map_err(|e| format!("AccessControl plugin of the application: {e}"))?;
    let properties = zerodds_security::properties::PropertyList::new();
    let request = ParticipantRequest::new(domain_id, profile.guid, &properties);
    let local = plugin
        .validate_local_permissions(profile.identity, &request)
        .map_err(|e| format!("validate_local_permissions of the plugin of the application: {e}"))?;
    // §9.8.1 step 3: `check_create_participant`; a participant the plugin
    // refuses is not created.
    let permitted = plugin
        .check_create_participant(local, &request)
        .map_err(|e| format!("check_create_participant of the plugin of the application: {e}"))?;
    if !permitted.is_permitted() {
        return Err(String::from(
            "the AccessControl plugin of the application refuses the participant \
             (check_create_participant, DDS-Security 1.2 §9.8.1 step 3)",
        ));
    }
    profile
        .gate
        .set_access_control(Box::new(plugin), local)
        .map_err(|e| format!("AccessControl plugin of the application: {e:?}"))
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

    /// The six-path configuration reads the vendor-form settings from the
    /// environment, as the enclave directory does.
    #[test]
    fn the_six_path_config_reads_the_vendor_form_settings() {
        let cfg = ZeroDdsSecurityConfig {
            identity_ca_path: Some("/a".into()),
            identity_cert_path: Some("/b".into()),
            identity_key_path: Some("/c".into()),
            permissions_ca_path: Some("/d".into()),
            governance_path: Some("/e".into()),
            permissions_path: Some("/f".into()),
            plugins: crate::plugin_ffi::ApplicationPlugins::default(),
        };
        let none = |_: &str| None;
        let defaults = cfg.try_to_profile_cfg_with_env(0, &none).unwrap();
        assert_eq!(
            defaults.algo_name_nul,
            zerodds_security_runtime::AlgoNameNul::Spec
        );
        assert!(!defaults.kx_key_fallback);
        assert_eq!(
            defaults.algorithm_names,
            zerodds_security_runtime::AlgorithmNames::V1_2
        );
        assert!(defaults.compatibility_checks);
        assert_eq!(
            defaults.subject_name_form,
            zerodds_security_runtime::SubjectNameForm::Rfc4514
        );
        let set = |name: &str| match name {
            "ZERODDS_SEC_SUBJECT_NAME_FORM" => Some("x509_name_oneline".to_string()),
            "ZERODDS_SEC_ALGO_NAME_NUL" => Some("mirror".to_string()),
            "ZERODDS_SEC_KX_KEY_FALLBACK" => Some("accept".to_string()),
            "ZERODDS_SEC_ALGORITHM_NAMES" => Some("1.1".to_string()),
            _ => None,
        };
        let with = cfg.try_to_profile_cfg_with_env(0, &set).unwrap();
        assert_eq!(
            with.algo_name_nul,
            zerodds_security_runtime::AlgoNameNul::Mirror
        );
        assert!(with.kx_key_fallback);
        assert_eq!(
            with.subject_name_form,
            zerodds_security_runtime::SubjectNameForm::X509NameOneline
        );
        assert_eq!(
            with.algorithm_names,
            zerodds_security_runtime::AlgorithmNames::V1_1
        );
        let bad_form =
            |name: &str| (name == "ZERODDS_SEC_SUBJECT_NAME_FORM").then(|| "dn".to_string());
        assert!(cfg.try_to_profile_cfg_with_env(0, &bad_form).is_err());
        let bad = |name: &str| (name == "ZERODDS_SEC_ALGO_NAME_NUL").then(|| "nul".to_string());
        assert!(cfg.try_to_profile_cfg_with_env(0, &bad).is_err());
        let bad_names =
            |name: &str| (name == "ZERODDS_SEC_ALGORITHM_NAMES").then(|| "1.0".to_string());
        assert!(cfg.try_to_profile_cfg_with_env(0, &bad_names).is_err());
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
        // C7: without an enclave directory in any layer the env variant returns
        // NULL, so that the caller (rmw-shim) falls back to the plain path.
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
