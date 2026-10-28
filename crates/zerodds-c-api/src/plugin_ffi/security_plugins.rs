// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Authentication and the Cryptographic plugin of an application in C
//! (DDS-Security 1.2 §9.3.2.11, §9.5.1): the structs of
//! `include/zerodds_security_plugins.h` and an adapter for each that
//! implements the Rust trait by calling the functions of the struct. The
//! rules of the module above hold: the functions of one plugin are called one
//! at a time, what a plugin hands out is copied at once and given back with
//! the return operation.

#![allow(clippy::type_complexity)]

use super::{
    CDataHolder, CProperties, Destroy, PluginCell, ZERODDS_PLUGIN_ABI_VERSION,
    ZeroDdsPluginBinaryProperty, ZeroDdsPluginDataHolder, ZeroDdsPluginEndpointConfig,
    ZeroDdsPluginError, ZeroDdsPluginParticipantAlgorithmInfo, ZeroDdsPluginParticipantConfig,
    ZeroDdsPluginProperty, bad, blank_error, check, holder_of, missing, slice_of, string_of,
};
use core::ffi::{c_char, c_void};
use std::sync::Arc;

use zerodds_security::access_control::PermissionsHandle;
use zerodds_security::algorithm::CryptoTransformKeyRevisionIntHolder;
use zerodds_security::authentication::{
    AlgorithmMasks, AuthStatusKind, AuthenticationExtensions, AuthenticationListener,
    AuthenticationPlugin, HandshakeHandle, HandshakeStepOutcome, IdentityHandle,
    LocalIdentityValidation, ParticipantSecurityConfig as AuthConfig, RemoteIdentityValidation,
    SharedSecretHandle, SharedSecretProvider, ValidationResult,
};
use zerodds_security::cryptographic::{
    CryptoKeyExchange, CryptoKeyFactory, CryptoTransform, CryptographicPluginInterfaces,
    DatareaderCryptoHandle, DatawriterCryptoHandle, EncodedPayload, EndpointSecurityAlgorithmInfo,
    EndpointSecurityConfig, EndpointSecuritySymmetricCipherAlgorithmInfo, ParticipantCryptoHandle,
    ParticipantSecurityAlgorithmInfo, ParticipantSecurityConfig, PreprocessedSubmessage,
    SecureSubmessageCategory,
};
use zerodds_security::error::SecurityResult;
use zerodds_security::properties::{Property, PropertyList};
use zerodds_security::token::CryptoToken;

type Status = i32;
type Err = *mut ZeroDdsPluginError;
type Handle = u64;
type Guid = *const u8;

/// `ZeroDdsPluginBytes`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginBytes {
    /// The octets.
    pub data: *const u8,
    /// Their number.
    pub len: usize,
}

const NO_BYTES: ZeroDdsPluginBytes = ZeroDdsPluginBytes {
    data: core::ptr::null(),
    len: 0,
};

fn bytes_in(v: &[u8]) -> ZeroDdsPluginBytes {
    ZeroDdsPluginBytes {
        data: v.as_ptr(),
        len: v.len(),
    }
}

fn bytes_out(b: &ZeroDdsPluginBytes) -> Vec<u8> {
    slice_of(b.data, b.len).to_vec()
}

/// `ZeroDdsPluginHandshakeStep`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginHandshakeStep {
    /// 0 send message, 1 send final message, 2 complete, 3 waiting for the
    /// peer, 4 pending retry.
    pub kind: i32,
    /// The token to send (kinds 0 and 1).
    pub token: ZeroDdsPluginBytes,
    /// The SharedSecretHandle (kind 2).
    pub secret: u64,
}

/// `ZeroDdsPluginAuthenticationListener`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginAuthenticationListener {
    /// Handed to the functions.
    pub context: *mut c_void,
    /// `on_revoke_identity`; 0 is success.
    pub on_revoke_identity: Option<unsafe extern "C" fn(*mut c_void, u64) -> i32>,
    /// `on_status_changed`.
    pub on_status_changed: Option<unsafe extern "C" fn(*mut c_void, u64, i32)>,
}

/// `ZeroDdsPluginAuthentication`: the Authentication plugin of an application.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ZeroDdsPluginAuthentication {
    /// Must be [`ZERODDS_PLUGIN_ABI_VERSION`].
    pub abi_version: u32,
    /// Handed to every function.
    pub user: *mut c_void,
    /// `plugin_class_id`.
    pub plugin_class_id: Option<unsafe extern "C" fn(*mut c_void) -> *const c_char>,
    /// `validate_local_identity`.
    pub validate_local_identity: Option<
        unsafe extern "C" fn(
            *mut c_void,
            u32,
            *const ZeroDdsPluginProperty,
            usize,
            *const ZeroDdsPluginBinaryProperty,
            usize,
            Guid,
            *mut i32,
            *mut Handle,
            *mut u8,
            Err,
        ) -> Status,
    >,
    /// `get_identity_token`.
    pub get_identity_token:
        Option<unsafe extern "C" fn(*mut c_void, Handle, *mut ZeroDdsPluginBytes, Err) -> Status>,
    /// `get_identity_status_token`.
    pub get_identity_status_token:
        Option<unsafe extern "C" fn(*mut c_void, Handle, *mut ZeroDdsPluginBytes, Err) -> Status>,
    /// `set_participant_security_config`.
    pub set_participant_security_config: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            *const super::ZeroDdsPluginAlgorithmMasks,
            *mut super::ZeroDdsPluginAlgorithmMasks,
            Err,
        ) -> Status,
    >,
    /// `set_permissions_credential_and_token`.
    pub set_permissions_credential_and_token: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            *const ZeroDdsPluginBytes,
            *const ZeroDdsPluginBytes,
            Err,
        ) -> Status,
    >,
    /// `validate_remote_identity`.
    pub validate_remote_identity: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            Guid,
            *const ZeroDdsPluginBytes,
            *const ZeroDdsPluginBytes,
            *mut i32,
            *mut Handle,
            *mut ZeroDdsPluginBytes,
            Err,
        ) -> Status,
    >,
    /// `begin_handshake_request`.
    pub begin_handshake_request: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            Handle,
            *const ZeroDdsPluginBytes,
            *mut Handle,
            *mut ZeroDdsPluginHandshakeStep,
            Err,
        ) -> Status,
    >,
    /// `begin_handshake_reply`.
    pub begin_handshake_reply: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            Handle,
            *const ZeroDdsPluginBytes,
            *const ZeroDdsPluginBytes,
            *mut Handle,
            *mut ZeroDdsPluginHandshakeStep,
            Err,
        ) -> Status,
    >,
    /// `process_handshake`.
    pub process_handshake: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            *const ZeroDdsPluginBytes,
            *mut ZeroDdsPluginHandshakeStep,
            Err,
        ) -> Status,
    >,
    /// `get_shared_secret`.
    pub get_shared_secret:
        Option<unsafe extern "C" fn(*mut c_void, Handle, *mut Handle, Err) -> Status>,
    /// `get_authenticated_peer_credential_token`.
    pub get_authenticated_peer_credential_token:
        Option<unsafe extern "C" fn(*mut c_void, Handle, *mut ZeroDdsPluginBytes, Err) -> Status>,
    /// `set_listener`; the listener is NULL for none.
    pub set_listener: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const ZeroDdsPluginAuthenticationListener,
            Err,
        ) -> Status,
    >,
    /// `return_identity_token`.
    pub return_identity_token:
        Option<unsafe extern "C" fn(*mut c_void, *const ZeroDdsPluginBytes, Err) -> Status>,
    /// `return_identity_status_token`.
    pub return_identity_status_token:
        Option<unsafe extern "C" fn(*mut c_void, *const ZeroDdsPluginBytes, Err) -> Status>,
    /// `return_authenticated_peer_credential_token`.
    pub return_authenticated_peer_credential_token:
        Option<unsafe extern "C" fn(*mut c_void, *const ZeroDdsPluginBytes, Err) -> Status>,
    /// `return_handshake_handle`.
    pub return_handshake_handle: Option<unsafe extern "C" fn(*mut c_void, Handle, Err) -> Status>,
    /// `return_identity_handle`.
    pub return_identity_handle: Option<unsafe extern "C" fn(*mut c_void, Handle, Err) -> Status>,
    /// `return_sharedsecret_handle`.
    pub return_sharedsecret_handle:
        Option<unsafe extern "C" fn(*mut c_void, Handle, Err) -> Status>,
    /// The PermissionsToken the plugin announces (may be NULL: none).
    pub get_permissions_token: Option<unsafe extern "C" fn(*mut c_void, *mut ZeroDdsPluginBytes)>,
    /// The octets of the SharedSecret behind a handle (may be NULL; needed
    /// where the builtin Cryptographic plugin is used); returns false for an
    /// unknown handle.
    pub get_shared_secret_bytes:
        Option<unsafe extern "C" fn(*mut c_void, Handle, *mut ZeroDdsPluginBytes) -> bool>,
    /// The two challenges of the handshake behind a SharedSecret (may be
    /// NULL); returns false for an unknown handle.
    pub get_shared_secret_challenges:
        Option<unsafe extern "C" fn(*mut c_void, Handle, *mut u8, *mut u8) -> bool>,
    /// Called once when the plugin is released.
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}

impl Destroy for ZeroDdsPluginAuthentication {
    fn destroy(&self) {
        if let Some(destroy) = self.destroy {
            // SAFETY: the plugin's own function, called once with its `user`.
            unsafe { destroy(self.user) };
        }
    }
}

/// `ZeroDdsPluginEndpointAlgorithmInfo`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ZeroDdsPluginEndpointAlgorithmInfo {
    /// `required_mask`.
    pub required_mask: u32,
    /// `supported_mask`.
    pub supported_mask: u32,
}

/// `ZeroDdsPluginCryptoTokens`: a list of CryptoTokens the plugin owns.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginCryptoTokens {
    /// Whether there are tokens at all (false: the answer is "none").
    pub present: bool,
    /// The tokens.
    pub tokens: *const ZeroDdsPluginDataHolder,
    /// Their number.
    pub n: usize,
}

/// `ZeroDdsPluginEncodedPayload`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginEncodedPayload {
    /// Whether there is an answer (false: FALSE without an exception).
    pub present: bool,
    /// The encoded buffer.
    pub encoded: ZeroDdsPluginBytes,
    /// Extra inline QoS the receiver needs.
    pub extra_inline_qos: ZeroDdsPluginBytes,
}

/// `ZeroDdsPluginCryptographic`: the Cryptographic plugin of an application.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ZeroDdsPluginCryptographic {
    /// Must be [`ZERODDS_PLUGIN_ABI_VERSION`].
    pub abi_version: u32,
    /// Handed to every function.
    pub user: *mut c_void,
    /// `register_local_participant`.
    pub register_local_participant: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            Handle,
            *const ZeroDdsPluginProperty,
            usize,
            *const ZeroDdsPluginParticipantConfig,
            *mut Handle,
            *mut ZeroDdsPluginParticipantAlgorithmInfo,
            Err,
        ) -> Status,
    >,
    /// `register_matched_remote_participant`.
    pub register_matched_remote_participant: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            Handle,
            Handle,
            Handle,
            *mut Handle,
            Err,
        ) -> Status,
    >,
    /// `register_local_datawriter`.
    pub register_local_datawriter: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            *const ZeroDdsPluginProperty,
            usize,
            *const ZeroDdsPluginEndpointConfig,
            Guid,
            *mut Handle,
            *mut ZeroDdsPluginEndpointAlgorithmInfo,
            Err,
        ) -> Status,
    >,
    /// `register_matched_remote_datareader`.
    pub register_matched_remote_datareader: Option<
        unsafe extern "C" fn(*mut c_void, Handle, Handle, Handle, bool, *mut Handle, Err) -> Status,
    >,
    /// `register_local_datareader`.
    pub register_local_datareader: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            *const ZeroDdsPluginProperty,
            usize,
            *const ZeroDdsPluginEndpointConfig,
            Guid,
            *mut Handle,
            *mut ZeroDdsPluginEndpointAlgorithmInfo,
            Err,
        ) -> Status,
    >,
    /// `register_matched_remote_datawriter`.
    pub register_matched_remote_datawriter: Option<
        unsafe extern "C" fn(*mut c_void, Handle, Handle, Handle, *mut Handle, Err) -> Status,
    >,
    /// `revise_local_entity_keys`.
    pub revise_local_entity_keys:
        Option<unsafe extern "C" fn(*mut c_void, Handle, *mut i32, Err) -> Status>,
    /// `activate_key_revision`.
    pub activate_key_revision:
        Option<unsafe extern "C" fn(*mut c_void, Handle, i32, *mut bool, Err) -> Status>,
    /// `unregister_participant`.
    pub unregister_participant:
        Option<unsafe extern "C" fn(*mut c_void, Handle, *mut bool, Err) -> Status>,
    /// `unregister_datawriter`.
    pub unregister_datawriter:
        Option<unsafe extern "C" fn(*mut c_void, Handle, *mut bool, Err) -> Status>,
    /// `unregister_datareader`.
    pub unregister_datareader:
        Option<unsafe extern "C" fn(*mut c_void, Handle, *mut bool, Err) -> Status>,
    /// `create_local_participant_crypto_tokens`.
    pub create_local_participant_crypto_tokens: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            Handle,
            i32,
            *mut ZeroDdsPluginCryptoTokens,
            Err,
        ) -> Status,
    >,
    /// `set_remote_participant_crypto_tokens`.
    pub set_remote_participant_crypto_tokens: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            Handle,
            *const ZeroDdsPluginDataHolder,
            usize,
            *mut bool,
            Err,
        ) -> Status,
    >,
    /// `create_local_datawriter_crypto_tokens`.
    pub create_local_datawriter_crypto_tokens: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            Handle,
            i32,
            *mut ZeroDdsPluginCryptoTokens,
            Err,
        ) -> Status,
    >,
    /// `set_remote_datawriter_crypto_tokens`.
    pub set_remote_datawriter_crypto_tokens: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            Handle,
            *const ZeroDdsPluginDataHolder,
            usize,
            *mut bool,
            Err,
        ) -> Status,
    >,
    /// `create_local_datareader_crypto_tokens`.
    pub create_local_datareader_crypto_tokens: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            Handle,
            i32,
            *mut ZeroDdsPluginCryptoTokens,
            Err,
        ) -> Status,
    >,
    /// `set_remote_datareader_crypto_tokens`.
    pub set_remote_datareader_crypto_tokens: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            Handle,
            *const ZeroDdsPluginDataHolder,
            usize,
            *mut bool,
            Err,
        ) -> Status,
    >,
    /// `return_crypto_tokens`.
    pub return_crypto_tokens: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const ZeroDdsPluginDataHolder,
            usize,
            *mut bool,
            Err,
        ) -> Status,
    >,
    /// `encode_serialized_payload`.
    pub encode_serialized_payload: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const ZeroDdsPluginBytes,
            Handle,
            *mut ZeroDdsPluginEncodedPayload,
            Err,
        ) -> Status,
    >,
    /// `encode_datawriter_submessage`.
    pub encode_datawriter_submessage: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const ZeroDdsPluginBytes,
            Handle,
            *const Handle,
            usize,
            *mut usize,
            *mut ZeroDdsPluginBytes,
            *mut bool,
            Err,
        ) -> Status,
    >,
    /// `encode_datareader_submessage`.
    pub encode_datareader_submessage: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const ZeroDdsPluginBytes,
            Handle,
            *const Handle,
            usize,
            *mut bool,
            *mut ZeroDdsPluginBytes,
            Err,
        ) -> Status,
    >,
    /// `encode_rtps_message`.
    pub encode_rtps_message: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const ZeroDdsPluginBytes,
            Handle,
            *const Handle,
            usize,
            *mut usize,
            bool,
            *mut ZeroDdsPluginBytes,
            *mut bool,
            Err,
        ) -> Status,
    >,
    /// `decode_rtps_message`.
    pub decode_rtps_message: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const ZeroDdsPluginBytes,
            Handle,
            Handle,
            *mut bool,
            *mut ZeroDdsPluginBytes,
            Err,
        ) -> Status,
    >,
    /// `preprocess_secure_submsg`.
    pub preprocess_secure_submsg: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const ZeroDdsPluginBytes,
            Handle,
            Handle,
            *mut bool,
            *mut Handle,
            *mut Handle,
            *mut i32,
            Err,
        ) -> Status,
    >,
    /// `decode_datawriter_submessage`.
    pub decode_datawriter_submessage: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const ZeroDdsPluginBytes,
            Handle,
            Handle,
            *mut bool,
            *mut ZeroDdsPluginBytes,
            Err,
        ) -> Status,
    >,
    /// `decode_datareader_submessage`.
    pub decode_datareader_submessage: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const ZeroDdsPluginBytes,
            Handle,
            Handle,
            *mut bool,
            *mut ZeroDdsPluginBytes,
            Err,
        ) -> Status,
    >,
    /// `decode_serialized_payload`.
    pub decode_serialized_payload: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const ZeroDdsPluginBytes,
            *const ZeroDdsPluginBytes,
            Handle,
            Handle,
            *mut bool,
            *mut ZeroDdsPluginBytes,
            Err,
        ) -> Status,
    >,
    /// Called once when the plugin is released.
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}

impl Destroy for ZeroDdsPluginCryptographic {
    fn destroy(&self) {
        if let Some(destroy) = self.destroy {
            // SAFETY: the plugin's own function, called once with its `user`.
            unsafe { destroy(self.user) };
        }
    }
}

macro_rules! op {
    ($table:expr, $name:ident) => {
        $table.$name.ok_or_else(|| missing(stringify!($name)))?
    };
}

// ============================================================================
// Authentication
// ============================================================================

const AUTHENTICATION_OPERATIONS: &[(&str, fn(&ZeroDdsPluginAuthentication) -> bool)] = &[
    ("plugin_class_id", |t| t.plugin_class_id.is_some()),
    ("validate_local_identity", |t| {
        t.validate_local_identity.is_some()
    }),
    ("get_identity_token", |t| t.get_identity_token.is_some()),
    ("get_identity_status_token", |t| {
        t.get_identity_status_token.is_some()
    }),
    ("set_participant_security_config", |t| {
        t.set_participant_security_config.is_some()
    }),
    ("set_permissions_credential_and_token", |t| {
        t.set_permissions_credential_and_token.is_some()
    }),
    ("validate_remote_identity", |t| {
        t.validate_remote_identity.is_some()
    }),
    ("begin_handshake_request", |t| {
        t.begin_handshake_request.is_some()
    }),
    ("begin_handshake_reply", |t| {
        t.begin_handshake_reply.is_some()
    }),
    ("process_handshake", |t| t.process_handshake.is_some()),
    ("get_shared_secret", |t| t.get_shared_secret.is_some()),
    ("get_authenticated_peer_credential_token", |t| {
        t.get_authenticated_peer_credential_token.is_some()
    }),
    ("set_listener", |t| t.set_listener.is_some()),
    ("return_identity_token", |t| {
        t.return_identity_token.is_some()
    }),
    ("return_identity_status_token", |t| {
        t.return_identity_status_token.is_some()
    }),
    ("return_authenticated_peer_credential_token", |t| {
        t.return_authenticated_peer_credential_token.is_some()
    }),
    ("return_handshake_handle", |t| {
        t.return_handshake_handle.is_some()
    }),
    ("return_identity_handle", |t| {
        t.return_identity_handle.is_some()
    }),
    ("return_sharedsecret_handle", |t| {
        t.return_sharedsecret_handle.is_some()
    }),
    ("destroy", |t| t.destroy.is_some()),
];

/// An Authentication plugin table ready to be installed more than once.
#[derive(Clone)]
pub struct AuthenticationHandle(Arc<PluginCell<ZeroDdsPluginAuthentication>>);

impl core::fmt::Debug for AuthenticationHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("AuthenticationHandle")
    }
}

/// The listener the host gave and the table the plugin was given a pointer to.
struct AuthListenerState {
    _listener: Box<Arc<dyn AuthenticationListener>>,
    _table: Box<ZeroDdsPluginAuthenticationListener>,
}

// SAFETY: the table points to the listener next to it, which is `Send + Sync`.
unsafe impl Send for AuthListenerState {}

/// The function the Authentication plugin calls when it revokes an identity.
///
/// # Safety
/// `context` is the pointer the runtime gave in `set_listener`.
unsafe extern "C" fn on_revoke_identity_trampoline(context: *mut c_void, handle: u64) -> i32 {
    if context.is_null() {
        return 1;
    }
    // SAFETY: the pointer is a live `Box<Arc<dyn AuthenticationListener>>`.
    let listener = unsafe { &*context.cast::<Arc<dyn AuthenticationListener>>() };
    match listener.on_revoke_identity(IdentityHandle(handle)) {
        Ok(()) => 0,
        Err(e) => e.code,
    }
}

/// The function the Authentication plugin calls when the identity status
/// changes.
///
/// # Safety
/// `context` is the pointer the runtime gave in `set_listener`.
unsafe extern "C" fn on_status_changed_trampoline(context: *mut c_void, handle: u64, _kind: i32) {
    if context.is_null() {
        return;
    }
    // SAFETY: the pointer is a live `Box<Arc<dyn AuthenticationListener>>`.
    let listener = unsafe { &*context.cast::<Arc<dyn AuthenticationListener>>() };
    listener.on_status_changed(IdentityHandle(handle), AuthStatusKind::IdentityStatus);
}

impl AuthenticationHandle {
    /// Takes a copy of the plugin table.
    ///
    /// # Errors
    /// The `abi_version` is not [`ZERODDS_PLUGIN_ABI_VERSION`] or an operation
    /// is NULL (the name is in the message).
    pub fn new(table: ZeroDdsPluginAuthentication) -> Result<Self, String> {
        if table.abi_version != ZERODDS_PLUGIN_ABI_VERSION {
            return Err(format!(
                "the Authentication plugin has abi_version {}, the runtime {}",
                table.abi_version, ZERODDS_PLUGIN_ABI_VERSION
            ));
        }
        for (name, present) in AUTHENTICATION_OPERATIONS {
            if !present(&table) {
                return Err(format!("the Authentication plugin has no {name}"));
            }
        }
        Ok(Self(Arc::new(PluginCell::new(table))))
    }

    /// A plugin for one runtime.
    ///
    /// # Errors
    /// The plugin gives a class id that is not UTF-8.
    pub fn instantiate(&self) -> SecurityResult<CAuthentication> {
        let class_id = {
            let table = self.0.lock();
            let f = op!(table, plugin_class_id);
            // SAFETY: the plugin's function.
            string_of(unsafe { f(table.user) })?
        };
        Ok(CAuthentication {
            cell: Arc::clone(&self.0),
            class_id,
            identity_tokens: Vec::new(),
            status_tokens: Vec::new(),
            credential_tokens: Vec::new(),
        })
    }

    /// The SharedSecretProvider of the plugin, for the builtin Cryptographic
    /// plugin.
    #[must_use]
    pub fn secrets(&self) -> Arc<dyn SharedSecretProvider> {
        Arc::new(CAuthenticationSecrets(Arc::clone(&self.0)))
    }
}

/// The octets of the SharedSecrets of a C Authentication plugin.
struct CAuthenticationSecrets(Arc<PluginCell<ZeroDdsPluginAuthentication>>);

impl SharedSecretProvider for CAuthenticationSecrets {
    fn get_shared_secret(&self, handle: SharedSecretHandle) -> Option<Vec<u8>> {
        let table = self.0.lock();
        let f = table.get_shared_secret_bytes?;
        let mut out = NO_BYTES;
        // SAFETY: the plugin's function with valid arguments.
        let found = unsafe { f(table.user, handle.0, &mut out) };
        found.then(|| bytes_out(&out))
    }

    fn get_shared_secret_challenges(
        &self,
        handle: SharedSecretHandle,
    ) -> Option<([u8; 32], [u8; 32])> {
        let table = self.0.lock();
        let f = table.get_shared_secret_challenges?;
        let (mut a, mut b) = ([0u8; 32], [0u8; 32]);
        // SAFETY: the plugin's function with 32 octets each.
        let found = unsafe { f(table.user, handle.0, a.as_mut_ptr(), b.as_mut_ptr()) };
        found.then_some((a, b))
    }
}

/// The Authentication plugin of an application in C, as the Rust plugin.
pub struct CAuthentication {
    cell: Arc<PluginCell<ZeroDdsPluginAuthentication>>,
    class_id: String,
    identity_tokens: Vec<(Vec<u8>, ZeroDdsPluginBytes)>,
    status_tokens: Vec<(Vec<u8>, ZeroDdsPluginBytes)>,
    credential_tokens: Vec<(Vec<u8>, ZeroDdsPluginBytes)>,
}

// SAFETY: the issued lists hold structs that point into the plugin's own
// memory; they are touched only through `&mut self`, and every call into the
// plugin goes through the lock of the cell.
unsafe impl Send for CAuthentication {}
// SAFETY: see above; the `&self` operations touch the cell only.
unsafe impl Sync for CAuthentication {}

impl core::fmt::Debug for CAuthentication {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CAuthentication")
            .field("class_id", &self.class_id)
            .finish_non_exhaustive()
    }
}

fn validation_of(code: i32) -> SecurityResult<ValidationResult> {
    Ok(match code {
        0 => ValidationResult::Ok,
        1 => ValidationResult::Failed,
        2 => ValidationResult::PendingRetry,
        3 => ValidationResult::PendingHandshakeRequest,
        4 => ValidationResult::PendingHandshakeMessage,
        5 => ValidationResult::OkFinalMessage,
        _ => {
            return Err(bad(
                "the plugin gave a ValidationResult that is none of the six",
            ));
        }
    })
}

fn step_of(raw: &ZeroDdsPluginHandshakeStep) -> SecurityResult<HandshakeStepOutcome> {
    Ok(match raw.kind {
        0 => HandshakeStepOutcome::SendMessage {
            token: bytes_out(&raw.token),
        },
        1 => HandshakeStepOutcome::SendFinalMessage {
            token: bytes_out(&raw.token),
        },
        2 => HandshakeStepOutcome::Complete {
            secret: SharedSecretHandle(raw.secret),
        },
        3 => HandshakeStepOutcome::WaitingForPeer,
        4 => HandshakeStepOutcome::PendingRetry,
        _ => return Err(bad("the plugin gave a handshake step of an unknown kind")),
    })
}

fn blank_step() -> ZeroDdsPluginHandshakeStep {
    ZeroDdsPluginHandshakeStep {
        kind: 3,
        token: NO_BYTES,
        secret: 0,
    }
}

impl CAuthentication {
    fn token(&mut self, handle: IdentityHandle, which: u8) -> SecurityResult<Vec<u8>> {
        let mut raw = NO_BYTES;
        {
            let table = self.cell.lock();
            let f = if which == 0 {
                op!(table, get_identity_token)
            } else {
                op!(table, get_identity_status_token)
            };
            let mut err = blank_error();
            // SAFETY: the plugin's function with valid arguments.
            let status = unsafe { f(table.user, handle.0, &mut raw, &mut err) };
            check(status, &err)?;
        }
        let value = bytes_out(&raw);
        if which == 0 {
            self.identity_tokens.push((value.clone(), raw));
        } else {
            self.status_tokens.push((value.clone(), raw));
        }
        Ok(value)
    }

    fn give_back(&mut self, token: &[u8], which: u8) -> SecurityResult<()> {
        let list = match which {
            0 => &mut self.identity_tokens,
            1 => &mut self.status_tokens,
            _ => &mut self.credential_tokens,
        };
        let at = list
            .iter()
            .position(|(value, _)| value == token)
            .ok_or_else(|| bad("the token was not issued by this plugin"))?;
        let (_, raw) = list.remove(at);
        let table = self.cell.lock();
        let f = match which {
            0 => op!(table, return_identity_token),
            1 => op!(table, return_identity_status_token),
            _ => op!(table, return_authenticated_peer_credential_token),
        };
        let mut err = blank_error();
        // SAFETY: the plugin's function with the struct it gave.
        let status = unsafe { f(table.user, &raw, &mut err) };
        check(status, &err)
    }

    fn handle_call(&self, which: u8, handle: u64) -> SecurityResult<()> {
        let table = self.cell.lock();
        let f = match which {
            0 => op!(table, return_handshake_handle),
            1 => op!(table, return_identity_handle),
            _ => op!(table, return_sharedsecret_handle),
        };
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe { f(table.user, handle, &mut err) };
        check(status, &err)
    }
}

impl AuthenticationExtensions for CAuthentication {
    fn plugin_class_id(&self) -> &str {
        &self.class_id
    }

    fn get_permissions_token(&self) -> Vec<u8> {
        let table = self.cell.lock();
        let Some(f) = table.get_permissions_token else {
            return Vec::new();
        };
        let mut out = NO_BYTES;
        // SAFETY: the plugin's function with valid arguments.
        unsafe { f(table.user, &mut out) };
        bytes_out(&out)
    }
}

impl AuthenticationPlugin for CAuthentication {
    fn validate_local_identity(
        &mut self,
        domain_id: u32,
        participant_qos: &PropertyList,
        candidate_participant_guid: [u8; 16],
    ) -> SecurityResult<LocalIdentityValidation> {
        let props = CProperties::new(
            participant_qos
                .entries()
                .iter()
                .map(|p| (p.name.as_ref(), p.value.as_ref(), p.propagate)),
        )?;
        let names = participant_qos
            .binary_entries()
            .iter()
            .map(|p| super::cstring(&p.name))
            .collect::<SecurityResult<Vec<_>>>()?;
        let binary: Vec<ZeroDdsPluginBinaryProperty> = participant_qos
            .binary_entries()
            .iter()
            .zip(&names)
            .map(|(p, name)| ZeroDdsPluginBinaryProperty {
                name: name.as_ptr(),
                value: p.value.as_ptr(),
                value_len: p.value.len(),
                propagate: p.propagate,
            })
            .collect();
        let (mut result, mut identity, mut adjusted) = (0i32, 0u64, [0u8; 16]);
        let table = self.cell.lock();
        let f = op!(table, validate_local_identity);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                domain_id,
                props.ptr(),
                props.len(),
                binary.as_ptr(),
                binary.len(),
                candidate_participant_guid.as_ptr(),
                &mut result,
                &mut identity,
                adjusted.as_mut_ptr(),
                &mut err,
            )
        };
        check(status, &err)?;
        Ok(LocalIdentityValidation {
            result: validation_of(result)?,
            identity: IdentityHandle(identity),
            adjusted_participant_guid: adjusted,
        })
    }

    fn get_identity_token(&mut self, handle: IdentityHandle) -> SecurityResult<Vec<u8>> {
        self.token(handle, 0)
    }

    fn get_identity_status_token(&mut self, handle: IdentityHandle) -> SecurityResult<Vec<u8>> {
        self.token(handle, 1)
    }

    fn set_participant_security_config(
        &mut self,
        handle: IdentityHandle,
        participant_security_config: &AuthConfig,
    ) -> SecurityResult<AlgorithmMasks> {
        let a = participant_security_config.algorithm_info;
        let masks = |(s, r): (u32, u32)| super::ZeroDdsPluginMasks {
            supported_mask: s,
            required_mask: r,
        };
        let config = super::ZeroDdsPluginAlgorithmMasks {
            trust_chain: masks(a.trust_chain),
            message_auth: masks(a.message_auth),
            key_establishment: masks(a.key_establishment),
        };
        let mut out = super::ZeroDdsPluginAlgorithmMasks::default();
        {
            let table = self.cell.lock();
            let f = op!(table, set_participant_security_config);
            let mut err = blank_error();
            // SAFETY: the plugin's function with valid arguments.
            let status = unsafe { f(table.user, handle.0, &config, &mut out, &mut err) };
            check(status, &err)?;
        }
        let pair = |m: super::ZeroDdsPluginMasks| (m.supported_mask, m.required_mask);
        Ok(AlgorithmMasks {
            trust_chain: pair(out.trust_chain),
            message_auth: pair(out.message_auth),
            key_establishment: pair(out.key_establishment),
        })
    }

    fn set_permissions_credential_and_token(
        &mut self,
        handle: IdentityHandle,
        permissions_credential_token: &[u8],
        permissions_token: &[u8],
    ) -> SecurityResult<()> {
        let (credential, token) = (
            bytes_in(permissions_credential_token),
            bytes_in(permissions_token),
        );
        let table = self.cell.lock();
        let f = op!(table, set_permissions_credential_and_token);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe { f(table.user, handle.0, &credential, &token, &mut err) };
        check(status, &err)
    }

    fn validate_remote_identity(
        &mut self,
        local: IdentityHandle,
        remote_participant_guid: [u8; 16],
        remote_identity_token: &[u8],
        remote_auth_request_token: Option<&[u8]>,
    ) -> SecurityResult<RemoteIdentityValidation> {
        let identity_token = bytes_in(remote_identity_token);
        let request = remote_auth_request_token.map(bytes_in);
        let (mut result, mut identity, mut local_request) = (0i32, 0u64, NO_BYTES);
        let table = self.cell.lock();
        let f = op!(table, validate_remote_identity);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                local.0,
                remote_participant_guid.as_ptr(),
                &identity_token,
                request.as_ref().map_or(core::ptr::null(), |r| r),
                &mut result,
                &mut identity,
                &mut local_request,
                &mut err,
            )
        };
        check(status, &err)?;
        Ok(RemoteIdentityValidation {
            result: validation_of(result)?,
            identity: IdentityHandle(identity),
            local_auth_request_token: (!local_request.data.is_null())
                .then(|| bytes_out(&local_request)),
        })
    }

    fn begin_handshake_request(
        &mut self,
        initiator: IdentityHandle,
        replier: IdentityHandle,
        serialized_local_participant_data: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        let data = bytes_in(serialized_local_participant_data);
        let (mut handshake, mut step) = (0u64, blank_step());
        let table = self.cell.lock();
        let f = op!(table, begin_handshake_request);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                initiator.0,
                replier.0,
                &data,
                &mut handshake,
                &mut step,
                &mut err,
            )
        };
        check(status, &err)?;
        Ok((HandshakeHandle(handshake), step_of(&step)?))
    }

    fn begin_handshake_reply(
        &mut self,
        replier: IdentityHandle,
        initiator: IdentityHandle,
        request_token: &[u8],
        serialized_local_participant_data: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        let (request, data) = (
            bytes_in(request_token),
            bytes_in(serialized_local_participant_data),
        );
        let (mut handshake, mut step) = (0u64, blank_step());
        let table = self.cell.lock();
        let f = op!(table, begin_handshake_reply);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                replier.0,
                initiator.0,
                &request,
                &data,
                &mut handshake,
                &mut step,
                &mut err,
            )
        };
        check(status, &err)?;
        Ok((HandshakeHandle(handshake), step_of(&step)?))
    }

    fn process_handshake(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome> {
        let token = bytes_in(token);
        let mut step = blank_step();
        let table = self.cell.lock();
        let f = op!(table, process_handshake);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe { f(table.user, handshake.0, &token, &mut step, &mut err) };
        check(status, &err)?;
        step_of(&step)
    }

    fn shared_secret(&self, handshake: HandshakeHandle) -> SecurityResult<SharedSecretHandle> {
        let mut secret = 0u64;
        let table = self.cell.lock();
        let f = op!(table, get_shared_secret);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe { f(table.user, handshake.0, &mut secret, &mut err) };
        check(status, &err)?;
        Ok(SharedSecretHandle(secret))
    }

    fn get_authenticated_peer_credential_token(
        &mut self,
        handshake: HandshakeHandle,
    ) -> SecurityResult<Vec<u8>> {
        let mut raw = NO_BYTES;
        {
            let table = self.cell.lock();
            let f = op!(table, get_authenticated_peer_credential_token);
            let mut err = blank_error();
            // SAFETY: the plugin's function with valid arguments.
            let status = unsafe { f(table.user, handshake.0, &mut raw, &mut err) };
            check(status, &err)?;
        }
        let value = bytes_out(&raw);
        self.credential_tokens.push((value.clone(), raw));
        Ok(value)
    }

    fn set_listener(
        &mut self,
        listener: Option<Arc<dyn AuthenticationListener>>,
    ) -> SecurityResult<()> {
        let state = listener.map(|listener| {
            let mut listener = Box::new(listener);
            let table = Box::new(ZeroDdsPluginAuthenticationListener {
                context: (&raw mut *listener).cast(),
                on_revoke_identity: Some(on_revoke_identity_trampoline),
                on_status_changed: Some(on_status_changed_trampoline),
            });
            (listener, table)
        });
        let pointer = state
            .as_ref()
            .map_or(core::ptr::null(), |(_, t)| &raw const **t);
        {
            let table = self.cell.lock();
            let f = op!(table, set_listener);
            let mut err = blank_error();
            // SAFETY: the plugin's function; the listener table lives in the
            // cell as long as the plugin.
            let status = unsafe { f(table.user, pointer, &mut err) };
            check(status, &err)?;
        }
        *self
            .cell
            .keep
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = state.map(|(l, t)| {
            Box::new(AuthListenerState {
                _listener: l,
                _table: t,
            }) as Box<dyn core::any::Any + Send>
        });
        Ok(())
    }

    fn return_identity_token(&mut self, token: &[u8]) -> SecurityResult<()> {
        self.give_back(token, 0)
    }

    fn return_identity_status_token(&mut self, token: &[u8]) -> SecurityResult<()> {
        self.give_back(token, 1)
    }

    fn return_authenticated_peer_credential_token(&mut self, token: &[u8]) -> SecurityResult<()> {
        self.give_back(token, 2)
    }

    fn return_handshake_handle(&mut self, handshake: HandshakeHandle) -> SecurityResult<()> {
        self.handle_call(0, handshake.0)
    }

    fn return_identity_handle(&mut self, handle: IdentityHandle) -> SecurityResult<()> {
        self.handle_call(1, handle.0)
    }

    fn return_sharedsecret_handle(&mut self, handle: SharedSecretHandle) -> SecurityResult<()> {
        self.handle_call(2, handle.0)
    }
}

// ============================================================================
// Cryptographic
// ============================================================================

const CRYPTOGRAPHIC_OPERATIONS: &[(&str, fn(&ZeroDdsPluginCryptographic) -> bool)] = &[
    ("register_local_participant", |t| {
        t.register_local_participant.is_some()
    }),
    ("register_matched_remote_participant", |t| {
        t.register_matched_remote_participant.is_some()
    }),
    ("register_local_datawriter", |t| {
        t.register_local_datawriter.is_some()
    }),
    ("register_matched_remote_datareader", |t| {
        t.register_matched_remote_datareader.is_some()
    }),
    ("register_local_datareader", |t| {
        t.register_local_datareader.is_some()
    }),
    ("register_matched_remote_datawriter", |t| {
        t.register_matched_remote_datawriter.is_some()
    }),
    ("revise_local_entity_keys", |t| {
        t.revise_local_entity_keys.is_some()
    }),
    ("activate_key_revision", |t| {
        t.activate_key_revision.is_some()
    }),
    ("unregister_participant", |t| {
        t.unregister_participant.is_some()
    }),
    ("unregister_datawriter", |t| {
        t.unregister_datawriter.is_some()
    }),
    ("unregister_datareader", |t| {
        t.unregister_datareader.is_some()
    }),
    ("create_local_participant_crypto_tokens", |t| {
        t.create_local_participant_crypto_tokens.is_some()
    }),
    ("set_remote_participant_crypto_tokens", |t| {
        t.set_remote_participant_crypto_tokens.is_some()
    }),
    ("create_local_datawriter_crypto_tokens", |t| {
        t.create_local_datawriter_crypto_tokens.is_some()
    }),
    ("set_remote_datawriter_crypto_tokens", |t| {
        t.set_remote_datawriter_crypto_tokens.is_some()
    }),
    ("create_local_datareader_crypto_tokens", |t| {
        t.create_local_datareader_crypto_tokens.is_some()
    }),
    ("set_remote_datareader_crypto_tokens", |t| {
        t.set_remote_datareader_crypto_tokens.is_some()
    }),
    ("return_crypto_tokens", |t| t.return_crypto_tokens.is_some()),
    ("encode_serialized_payload", |t| {
        t.encode_serialized_payload.is_some()
    }),
    ("encode_datawriter_submessage", |t| {
        t.encode_datawriter_submessage.is_some()
    }),
    ("encode_datareader_submessage", |t| {
        t.encode_datareader_submessage.is_some()
    }),
    ("encode_rtps_message", |t| t.encode_rtps_message.is_some()),
    ("decode_rtps_message", |t| t.decode_rtps_message.is_some()),
    ("preprocess_secure_submsg", |t| {
        t.preprocess_secure_submsg.is_some()
    }),
    ("decode_datawriter_submessage", |t| {
        t.decode_datawriter_submessage.is_some()
    }),
    ("decode_datareader_submessage", |t| {
        t.decode_datareader_submessage.is_some()
    }),
    ("decode_serialized_payload", |t| {
        t.decode_serialized_payload.is_some()
    }),
    ("destroy", |t| t.destroy.is_some()),
];

/// A Cryptographic plugin table ready to be installed.
#[derive(Clone)]
pub struct CryptographicHandle(Arc<PluginCell<ZeroDdsPluginCryptographic>>);

impl core::fmt::Debug for CryptographicHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("CryptographicHandle")
    }
}

impl CryptographicHandle {
    /// Takes a copy of the plugin table.
    ///
    /// # Errors
    /// The `abi_version` is not [`ZERODDS_PLUGIN_ABI_VERSION`] or an operation
    /// is NULL (the name is in the message).
    pub fn new(table: ZeroDdsPluginCryptographic) -> Result<Self, String> {
        if table.abi_version != ZERODDS_PLUGIN_ABI_VERSION {
            return Err(format!(
                "the Cryptographic plugin has abi_version {}, the runtime {}",
                table.abi_version, ZERODDS_PLUGIN_ABI_VERSION
            ));
        }
        for (name, present) in CRYPTOGRAPHIC_OPERATIONS {
            if !present(&table) {
                return Err(format!("the Cryptographic plugin has no {name}"));
            }
        }
        Ok(Self(Arc::new(PluginCell::new(table))))
    }

    /// A plugin for one runtime.
    #[must_use]
    pub fn instantiate(&self) -> CCryptographic {
        CCryptographic {
            cell: Arc::clone(&self.0),
            issued: Vec::new(),
        }
    }
}

/// The Cryptographic plugin of an application in C, as the Rust plugin.
pub struct CCryptographic {
    cell: Arc<PluginCell<ZeroDdsPluginCryptographic>>,
    /// Token lists handed out and not yet given back.
    issued: Vec<(Vec<CryptoToken>, ZeroDdsPluginCryptoTokens)>,
}

// SAFETY: `issued` holds structs that point into the plugin's own memory; it is
// touched only through `&mut self`, and every call into the plugin goes
// through the lock of the cell.
unsafe impl Send for CCryptographic {}
// SAFETY: see above; the `&self` operations touch the cell only.
unsafe impl Sync for CCryptographic {}

impl core::fmt::Debug for CCryptographic {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("CCryptographic")
    }
}

fn algorithm_info_of(
    raw: &ZeroDdsPluginParticipantAlgorithmInfo,
) -> ParticipantSecurityAlgorithmInfo {
    use zerodds_security::cryptographic as c;
    let req = |m: super::ZeroDdsPluginMasks| c::CryptoAlgorithmRequirements {
        supported_mask: m.supported_mask,
        required_mask: m.required_mask,
    };
    c::ParticipantSecurityAlgorithmInfo {
        digital_signature: c::ParticipantSecurityDigitalSignatureAlgorithmInfo {
            trust_chain: req(raw.trust_chain),
            message_auth: req(raw.message_auth),
        },
        key_establishment: c::ParticipantSecurityKeyEstablishmentAlgorithmInfo {
            shared_secret: req(raw.shared_secret),
        },
        symmetric_cipher: c::ParticipantSecuritySymmetricCipherAlgorithmInfo {
            supported_mask: raw.cipher_supported_mask,
            builtin_endpoints_required_mask: raw.cipher_builtin_endpoints_required_mask,
            builtin_kx_endpoints_required_mask: raw.cipher_builtin_kx_endpoints_required_mask,
            user_endpoints_default_required_mask: raw.cipher_user_endpoints_default_required_mask,
        },
    }
}

fn algorithm_info_to(
    a: &ParticipantSecurityAlgorithmInfo,
) -> ZeroDdsPluginParticipantAlgorithmInfo {
    let masks = |r: zerodds_security::cryptographic::CryptoAlgorithmRequirements| {
        super::ZeroDdsPluginMasks {
            supported_mask: r.supported_mask,
            required_mask: r.required_mask,
        }
    };
    ZeroDdsPluginParticipantAlgorithmInfo {
        trust_chain: masks(a.digital_signature.trust_chain),
        message_auth: masks(a.digital_signature.message_auth),
        shared_secret: masks(a.key_establishment.shared_secret),
        cipher_supported_mask: a.symmetric_cipher.supported_mask,
        cipher_builtin_endpoints_required_mask: a.symmetric_cipher.builtin_endpoints_required_mask,
        cipher_builtin_kx_endpoints_required_mask: a
            .symmetric_cipher
            .builtin_kx_endpoints_required_mask,
        cipher_user_endpoints_default_required_mask: a
            .symmetric_cipher
            .user_endpoints_default_required_mask,
    }
}

/// The C view of the properties of a registration.
fn properties_view(properties: &[Property]) -> SecurityResult<CProperties> {
    CProperties::new(
        properties
            .iter()
            .map(|p| (p.name.as_ref(), p.value.as_ref(), p.propagate)),
    )
}

/// The C view of an EndpointSecurityConfig of the Cryptographic plugin.
fn endpoint_view(
    config: &EndpointSecurityConfig,
) -> SecurityResult<(CProperties, ZeroDdsPluginEndpointConfig)> {
    let props = properties_view(&config.ac_endpoint_properties)?;
    let raw = ZeroDdsPluginEndpointConfig {
        topic: super::ZeroDdsPluginTopicConfig {
            is_read_protected: config.is_read_protected,
            is_write_protected: config.is_write_protected,
            is_discovery_protected: config.is_discovery_protected,
            is_liveliness_protected: config.is_liveliness_protected,
        },
        is_submessage_protected: config.is_submessage_protected,
        is_payload_protected: config.is_payload_protected,
        is_key_protected: config.is_key_protected,
        plugin_endpoint_attributes: config.plugin_endpoint_attributes,
        ac_endpoint_properties: props.ptr(),
        n_ac_endpoint_properties: props.len(),
        cipher_required_mask: config.algorithm_info.symmetric_cipher.required_mask,
        cipher_supported_mask: config.algorithm_info.symmetric_cipher.supported_mask,
    };
    Ok((props, raw))
}

impl CCryptographic {
    fn tokens_out(
        &mut self,
        raw: ZeroDdsPluginCryptoTokens,
    ) -> SecurityResult<Option<Vec<CryptoToken>>> {
        if !raw.present {
            return Ok(None);
        }
        let tokens = slice_of(raw.tokens, raw.n)
            .iter()
            .map(holder_of)
            .collect::<SecurityResult<Vec<_>>>()?;
        self.issued.push((tokens.clone(), raw));
        Ok(Some(tokens))
    }

    fn create_tokens(
        &mut self,
        which: u8,
        first: u64,
        second: u64,
        revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<Option<Vec<CryptoToken>>> {
        let mut raw = ZeroDdsPluginCryptoTokens {
            present: false,
            tokens: core::ptr::null(),
            n: 0,
        };
        {
            let table = self.cell.lock();
            let f = match which {
                0 => op!(table, create_local_participant_crypto_tokens),
                1 => op!(table, create_local_datawriter_crypto_tokens),
                _ => op!(table, create_local_datareader_crypto_tokens),
            };
            let mut err = blank_error();
            // SAFETY: the plugin's function with valid arguments.
            let status = unsafe { f(table.user, first, second, revision, &mut raw, &mut err) };
            check(status, &err)?;
        }
        self.tokens_out(raw)
    }

    fn set_tokens(
        &self,
        which: u8,
        first: u64,
        second: u64,
        tokens: &[CryptoToken],
    ) -> SecurityResult<bool> {
        let views = tokens
            .iter()
            .map(CDataHolder::new)
            .collect::<SecurityResult<Vec<_>>>()?;
        let raw: Vec<ZeroDdsPluginDataHolder> = views.iter().map(|v| v.raw).collect();
        let mut ok = false;
        let table = self.cell.lock();
        let f = match which {
            0 => op!(table, set_remote_participant_crypto_tokens),
            1 => op!(table, set_remote_datawriter_crypto_tokens),
            _ => op!(table, set_remote_datareader_crypto_tokens),
        };
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                first,
                second,
                raw.as_ptr(),
                raw.len(),
                &mut ok,
                &mut err,
            )
        };
        check(status, &err)?;
        drop(views);
        Ok(ok)
    }

    fn flag(
        &self,
        pick: fn(
            &ZeroDdsPluginCryptographic,
        )
            -> Option<unsafe extern "C" fn(*mut c_void, Handle, *mut bool, Err) -> Status>,
        name: &'static str,
        handle: u64,
    ) -> SecurityResult<bool> {
        let mut ok = false;
        let table = self.cell.lock();
        let f = pick(&table).ok_or_else(|| missing(name))?;
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe { f(table.user, handle, &mut ok, &mut err) };
        check(status, &err)?;
        Ok(ok)
    }

    /// A decode operation of the shape (encoded, two handles) -> optional
    /// bytes.
    fn decode(
        &self,
        encoded: &[u8],
        first: u64,
        second: u64,
        pick: fn(
            &ZeroDdsPluginCryptographic,
        ) -> Option<
            unsafe extern "C" fn(
                *mut c_void,
                *const ZeroDdsPluginBytes,
                Handle,
                Handle,
                *mut bool,
                *mut ZeroDdsPluginBytes,
                Err,
            ) -> Status,
        >,
        name: &'static str,
    ) -> SecurityResult<Option<Vec<u8>>> {
        let input = bytes_in(encoded);
        let (mut present, mut out) = (false, NO_BYTES);
        let table = self.cell.lock();
        let f = pick(&table).ok_or_else(|| missing(name))?;
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                &input,
                first,
                second,
                &mut present,
                &mut out,
                &mut err,
            )
        };
        check(status, &err)?;
        Ok(present.then(|| bytes_out(&out)))
    }
}

impl CryptoKeyFactory for CCryptographic {
    fn register_local_participant(
        &mut self,
        participant_identity: IdentityHandle,
        participant_permissions: PermissionsHandle,
        participant_properties: &[Property],
        participant_security_config: &ParticipantSecurityConfig,
    ) -> SecurityResult<(ParticipantCryptoHandle, ParticipantSecurityAlgorithmInfo)> {
        let props = properties_view(participant_properties)?;
        let ac = properties_view(&participant_security_config.ac_endpoint_properties)?;
        let c = participant_security_config;
        let raw = ZeroDdsPluginParticipantConfig {
            allow_unauthenticated_participants: c.allow_unauthenticated_participants,
            is_access_protected: c.is_access_protected,
            is_rtps_axk_protected: c.is_rtps_axk_protected,
            is_rtps_psk_protected: c.is_rtps_psk_protected,
            is_discovery_protected: c.is_discovery_protected,
            is_liveliness_protected: c.is_liveliness_protected,
            is_key_revision_enabled: c.is_key_revision_enabled,
            plugin_participant_attributes: c.plugin_participant_attributes,
            ac_participant_properties: ac.ptr(),
            n_ac_participant_properties: ac.len(),
            algorithm_info: algorithm_info_to(&c.algorithm_info),
        };
        let (mut handle, mut info) = (0u64, ZeroDdsPluginParticipantAlgorithmInfo::default());
        let table = self.cell.lock();
        let f = op!(table, register_local_participant);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                participant_identity.0,
                participant_permissions.0,
                props.ptr(),
                props.len(),
                &raw,
                &mut handle,
                &mut info,
                &mut err,
            )
        };
        check(status, &err)?;
        Ok((ParticipantCryptoHandle(handle), algorithm_info_of(&info)))
    }

    fn register_matched_remote_participant(
        &mut self,
        local_participant_crypto_handle: ParticipantCryptoHandle,
        remote_participant_identity: IdentityHandle,
        remote_participant_permissions: PermissionsHandle,
        shared_secret: SharedSecretHandle,
    ) -> SecurityResult<ParticipantCryptoHandle> {
        let mut handle = 0u64;
        let table = self.cell.lock();
        let f = op!(table, register_matched_remote_participant);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                local_participant_crypto_handle.0,
                remote_participant_identity.0,
                remote_participant_permissions.0,
                shared_secret.0,
                &mut handle,
                &mut err,
            )
        };
        check(status, &err)?;
        Ok(ParticipantCryptoHandle(handle))
    }

    fn register_local_datawriter(
        &mut self,
        participant_crypto: ParticipantCryptoHandle,
        datawriter_properties: &[Property],
        datawriter_security_config: &EndpointSecurityConfig,
        endpoint_guid: [u8; 16],
    ) -> SecurityResult<(DatawriterCryptoHandle, EndpointSecurityAlgorithmInfo)> {
        let props = properties_view(datawriter_properties)?;
        let (_keep, raw) = endpoint_view(datawriter_security_config)?;
        let (mut handle, mut info) = (0u64, ZeroDdsPluginEndpointAlgorithmInfo::default());
        let table = self.cell.lock();
        let f = op!(table, register_local_datawriter);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                participant_crypto.0,
                props.ptr(),
                props.len(),
                &raw,
                endpoint_guid.as_ptr(),
                &mut handle,
                &mut info,
                &mut err,
            )
        };
        check(status, &err)?;
        Ok((DatawriterCryptoHandle(handle), endpoint_info_of(&info)))
    }

    fn register_matched_remote_datareader(
        &mut self,
        local_datawriter_crypto_handle: DatawriterCryptoHandle,
        remote_participant_crypto: ParticipantCryptoHandle,
        shared_secret: SharedSecretHandle,
        relay_only: bool,
    ) -> SecurityResult<DatareaderCryptoHandle> {
        let mut handle = 0u64;
        let table = self.cell.lock();
        let f = op!(table, register_matched_remote_datareader);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                local_datawriter_crypto_handle.0,
                remote_participant_crypto.0,
                shared_secret.0,
                relay_only,
                &mut handle,
                &mut err,
            )
        };
        check(status, &err)?;
        Ok(DatareaderCryptoHandle(handle))
    }

    fn register_local_datareader(
        &mut self,
        participant_crypto: ParticipantCryptoHandle,
        datareader_properties: &[Property],
        datareader_security_config: &EndpointSecurityConfig,
        endpoint_guid: [u8; 16],
    ) -> SecurityResult<(DatareaderCryptoHandle, EndpointSecurityAlgorithmInfo)> {
        let props = properties_view(datareader_properties)?;
        let (_keep, raw) = endpoint_view(datareader_security_config)?;
        let (mut handle, mut info) = (0u64, ZeroDdsPluginEndpointAlgorithmInfo::default());
        let table = self.cell.lock();
        let f = op!(table, register_local_datareader);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                participant_crypto.0,
                props.ptr(),
                props.len(),
                &raw,
                endpoint_guid.as_ptr(),
                &mut handle,
                &mut info,
                &mut err,
            )
        };
        check(status, &err)?;
        Ok((DatareaderCryptoHandle(handle), endpoint_info_of(&info)))
    }

    fn register_matched_remote_datawriter(
        &mut self,
        local_datareader_crypto_handle: DatareaderCryptoHandle,
        remote_participant_crypt: ParticipantCryptoHandle,
        shared_secret: SharedSecretHandle,
    ) -> SecurityResult<DatawriterCryptoHandle> {
        let mut handle = 0u64;
        let table = self.cell.lock();
        let f = op!(table, register_matched_remote_datawriter);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                local_datareader_crypto_handle.0,
                remote_participant_crypt.0,
                shared_secret.0,
                &mut handle,
                &mut err,
            )
        };
        check(status, &err)?;
        Ok(DatawriterCryptoHandle(handle))
    }

    fn revise_local_entity_keys(
        &mut self,
        participant_crypto_handle: ParticipantCryptoHandle,
    ) -> SecurityResult<CryptoTransformKeyRevisionIntHolder> {
        let mut revision = 0i32;
        let table = self.cell.lock();
        let f = op!(table, revise_local_entity_keys);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                participant_crypto_handle.0,
                &mut revision,
                &mut err,
            )
        };
        check(status, &err)?;
        Ok(revision)
    }

    fn activate_key_revision(
        &mut self,
        local_participant_crypto_handle: ParticipantCryptoHandle,
        key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<bool> {
        let mut ok = false;
        let table = self.cell.lock();
        let f = op!(table, activate_key_revision);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                local_participant_crypto_handle.0,
                key_revision,
                &mut ok,
                &mut err,
            )
        };
        check(status, &err)?;
        Ok(ok)
    }

    fn unregister_participant(
        &mut self,
        participant_crypto_handle: ParticipantCryptoHandle,
    ) -> SecurityResult<bool> {
        self.flag(
            |t| t.unregister_participant,
            "unregister_participant",
            participant_crypto_handle.0,
        )
    }

    fn unregister_datawriter(
        &mut self,
        datawriter_crypto_handle: DatawriterCryptoHandle,
    ) -> SecurityResult<bool> {
        self.flag(
            |t| t.unregister_datawriter,
            "unregister_datawriter",
            datawriter_crypto_handle.0,
        )
    }

    fn unregister_datareader(
        &mut self,
        datareader_crypto_handle: DatareaderCryptoHandle,
    ) -> SecurityResult<bool> {
        self.flag(
            |t| t.unregister_datareader,
            "unregister_datareader",
            datareader_crypto_handle.0,
        )
    }
}

fn endpoint_info_of(raw: &ZeroDdsPluginEndpointAlgorithmInfo) -> EndpointSecurityAlgorithmInfo {
    EndpointSecurityAlgorithmInfo {
        symmetric_cipher: EndpointSecuritySymmetricCipherAlgorithmInfo {
            required_mask: raw.required_mask,
            supported_mask: raw.supported_mask,
        },
    }
}

impl CryptoKeyExchange for CCryptographic {
    fn create_local_participant_crypto_tokens(
        &mut self,
        local_participant_crypto: ParticipantCryptoHandle,
        remote_participant_crypto: ParticipantCryptoHandle,
        key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<Option<Vec<CryptoToken>>> {
        self.create_tokens(
            0,
            local_participant_crypto.0,
            remote_participant_crypto.0,
            key_revision,
        )
    }

    fn set_remote_participant_crypto_tokens(
        &mut self,
        local_participant_crypto: ParticipantCryptoHandle,
        remote_participant_crypto: ParticipantCryptoHandle,
        remote_participant_tokens: &[CryptoToken],
    ) -> SecurityResult<bool> {
        self.set_tokens(
            0,
            local_participant_crypto.0,
            remote_participant_crypto.0,
            remote_participant_tokens,
        )
    }

    fn create_local_datawriter_crypto_tokens(
        &mut self,
        local_datawriter_crypto: DatawriterCryptoHandle,
        remote_datareader_crypto: DatareaderCryptoHandle,
        key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<Option<Vec<CryptoToken>>> {
        self.create_tokens(
            1,
            local_datawriter_crypto.0,
            remote_datareader_crypto.0,
            key_revision,
        )
    }

    fn set_remote_datawriter_crypto_tokens(
        &mut self,
        local_datareader_crypto: DatareaderCryptoHandle,
        remote_datawriter_crypto: DatawriterCryptoHandle,
        remote_datawriter_tokens: &[CryptoToken],
    ) -> SecurityResult<bool> {
        self.set_tokens(
            1,
            local_datareader_crypto.0,
            remote_datawriter_crypto.0,
            remote_datawriter_tokens,
        )
    }

    fn create_local_datareader_crypto_tokens(
        &mut self,
        local_datareader_crypto: DatareaderCryptoHandle,
        remote_datawriter_crypto: DatawriterCryptoHandle,
        key_revision: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<Option<Vec<CryptoToken>>> {
        self.create_tokens(
            2,
            local_datareader_crypto.0,
            remote_datawriter_crypto.0,
            key_revision,
        )
    }

    fn set_remote_datareader_crypto_tokens(
        &mut self,
        local_datawriter_crypto: DatawriterCryptoHandle,
        remote_datareader_crypto: DatareaderCryptoHandle,
        remote_datareader_tokens: &[CryptoToken],
    ) -> SecurityResult<bool> {
        self.set_tokens(
            2,
            local_datawriter_crypto.0,
            remote_datareader_crypto.0,
            remote_datareader_tokens,
        )
    }

    fn return_crypto_tokens(&mut self, crypto_tokens: Vec<CryptoToken>) -> SecurityResult<bool> {
        let at = self
            .issued
            .iter()
            .position(|(value, _)| *value == crypto_tokens)
            .ok_or_else(|| bad("the tokens were not issued by this plugin"))?;
        let (_, raw) = self.issued.remove(at);
        let mut ok = false;
        let table = self.cell.lock();
        let f = op!(table, return_crypto_tokens);
        let mut err = blank_error();
        // SAFETY: the plugin's function with the list it gave.
        let status = unsafe { f(table.user, raw.tokens, raw.n, &mut ok, &mut err) };
        check(status, &err)?;
        Ok(ok)
    }
}

impl CryptoTransform for CCryptographic {
    fn encode_serialized_payload(
        &self,
        plain_buffer: &[u8],
        sending_datawriter_crypto: DatawriterCryptoHandle,
    ) -> SecurityResult<Option<EncodedPayload>> {
        let input = bytes_in(plain_buffer);
        let mut out = ZeroDdsPluginEncodedPayload {
            present: false,
            encoded: NO_BYTES,
            extra_inline_qos: NO_BYTES,
        };
        let table = self.cell.lock();
        let f = op!(table, encode_serialized_payload);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                &input,
                sending_datawriter_crypto.0,
                &mut out,
                &mut err,
            )
        };
        check(status, &err)?;
        Ok(out.present.then(|| EncodedPayload {
            encoded_buffer: bytes_out(&out.encoded),
            extra_inline_qos: bytes_out(&out.extra_inline_qos),
        }))
    }

    fn encode_datawriter_submessage(
        &self,
        encoded_rtps_submessage: &mut Vec<u8>,
        plain_rtps_submessage: &[u8],
        sending_datawriter_crypto: DatawriterCryptoHandle,
        receiving_datareader_crypto_list: &[DatareaderCryptoHandle],
        receiving_datareader_crypto_list_index: &mut usize,
    ) -> SecurityResult<bool> {
        let input = bytes_in(plain_rtps_submessage);
        let receivers: Vec<u64> = receiving_datareader_crypto_list
            .iter()
            .map(|h| h.0)
            .collect();
        let (mut ok, mut out) = (false, NO_BYTES);
        let table = self.cell.lock();
        let f = op!(table, encode_datawriter_submessage);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                &input,
                sending_datawriter_crypto.0,
                receivers.as_ptr(),
                receivers.len(),
                receiving_datareader_crypto_list_index,
                &mut out,
                &mut ok,
                &mut err,
            )
        };
        check(status, &err)?;
        if ok {
            *encoded_rtps_submessage = bytes_out(&out);
        }
        Ok(ok)
    }

    fn encode_datareader_submessage(
        &self,
        plain_rtps_submessage: &[u8],
        sending_datareader_crypto: DatareaderCryptoHandle,
        receiving_datawriter_crypto_list: &[DatawriterCryptoHandle],
    ) -> SecurityResult<Option<Vec<u8>>> {
        let input = bytes_in(plain_rtps_submessage);
        let receivers: Vec<u64> = receiving_datawriter_crypto_list
            .iter()
            .map(|h| h.0)
            .collect();
        let (mut present, mut out) = (false, NO_BYTES);
        let table = self.cell.lock();
        let f = op!(table, encode_datareader_submessage);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                &input,
                sending_datareader_crypto.0,
                receivers.as_ptr(),
                receivers.len(),
                &mut present,
                &mut out,
                &mut err,
            )
        };
        check(status, &err)?;
        Ok(present.then(|| bytes_out(&out)))
    }

    fn encode_rtps_message(
        &self,
        encoded_rtps_message: &mut Vec<u8>,
        plain_rtps_message: &[u8],
        sending_participant_crypto: ParticipantCryptoHandle,
        receiving_participant_crypto_list: &[ParticipantCryptoHandle],
        receiving_participant_crypto_list_index: &mut usize,
        transform_with_psk: bool,
    ) -> SecurityResult<bool> {
        let input = bytes_in(plain_rtps_message);
        let receivers: Vec<u64> = receiving_participant_crypto_list
            .iter()
            .map(|h| h.0)
            .collect();
        let (mut ok, mut out) = (false, NO_BYTES);
        let table = self.cell.lock();
        let f = op!(table, encode_rtps_message);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                &input,
                sending_participant_crypto.0,
                receivers.as_ptr(),
                receivers.len(),
                receiving_participant_crypto_list_index,
                transform_with_psk,
                &mut out,
                &mut ok,
                &mut err,
            )
        };
        check(status, &err)?;
        if ok {
            *encoded_rtps_message = bytes_out(&out);
        }
        Ok(ok)
    }

    fn decode_rtps_message(
        &self,
        encoded_buffer: &[u8],
        receiving_participant_crypto: ParticipantCryptoHandle,
        sending_participant_crypto: ParticipantCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        self.decode(
            encoded_buffer,
            receiving_participant_crypto.0,
            sending_participant_crypto.0,
            |t| t.decode_rtps_message,
            "decode_rtps_message",
        )
    }

    fn preprocess_secure_submsg(
        &self,
        encoded_rtps_submessage: &[u8],
        receiving_participant_crypto: ParticipantCryptoHandle,
        sending_participant_crypto: ParticipantCryptoHandle,
    ) -> SecurityResult<Option<PreprocessedSubmessage>> {
        let input = bytes_in(encoded_rtps_submessage);
        let (mut present, mut writer, mut reader, mut category) = (false, 0u64, 0u64, 0i32);
        let table = self.cell.lock();
        let f = op!(table, preprocess_secure_submsg);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                &input,
                receiving_participant_crypto.0,
                sending_participant_crypto.0,
                &mut present,
                &mut writer,
                &mut reader,
                &mut category,
                &mut err,
            )
        };
        check(status, &err)?;
        if !present {
            return Ok(None);
        }
        let secure_submessage_category = match category {
            0 => SecureSubmessageCategory::InfoSubmessage,
            1 => SecureSubmessageCategory::DatawriterSubmessage,
            2 => SecureSubmessageCategory::DatareaderSubmessage,
            _ => {
                return Err(bad(
                    "the plugin gave a SecureSubmessageCategory of an unknown value",
                ));
            }
        };
        Ok(Some(PreprocessedSubmessage {
            datawriter_crypto: DatawriterCryptoHandle(writer),
            datareader_crypto: DatareaderCryptoHandle(reader),
            secure_submessage_category,
        }))
    }

    fn decode_datawriter_submessage(
        &self,
        encoded_rtps_submessage: &[u8],
        receiving_datareader_crypto: DatareaderCryptoHandle,
        sending_datawriter_crypto: DatawriterCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        self.decode(
            encoded_rtps_submessage,
            receiving_datareader_crypto.0,
            sending_datawriter_crypto.0,
            |t| t.decode_datawriter_submessage,
            "decode_datawriter_submessage",
        )
    }

    fn decode_datareader_submessage(
        &self,
        encoded_rtps_submessage: &[u8],
        receiving_datawriter_crypto: DatawriterCryptoHandle,
        sending_datareader_crypto: DatareaderCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        self.decode(
            encoded_rtps_submessage,
            receiving_datawriter_crypto.0,
            sending_datareader_crypto.0,
            |t| t.decode_datareader_submessage,
            "decode_datareader_submessage",
        )
    }

    fn decode_serialized_payload(
        &self,
        encoded_buffer: &[u8],
        inline_qos: &[u8],
        receiving_datareader_crypto: DatareaderCryptoHandle,
        sending_datawriter_crypto: DatawriterCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        let (input, qos) = (bytes_in(encoded_buffer), bytes_in(inline_qos));
        let (mut present, mut out) = (false, NO_BYTES);
        let table = self.cell.lock();
        let f = op!(table, decode_serialized_payload);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe {
            f(
                table.user,
                &input,
                &qos,
                receiving_datareader_crypto.0,
                sending_datawriter_crypto.0,
                &mut present,
                &mut out,
                &mut err,
            )
        };
        check(status, &err)?;
        Ok(present.then(|| bytes_out(&out)))
    }
}

impl CryptographicPluginInterfaces for CCryptographic {}

// ============================================================================
// Placing them into a security configuration
// ============================================================================

/// Hands the Authentication plugin `plugin` (a `ZeroDdsPluginAuthentication`)
/// to the security configuration `cfg`. With it, `zerodds_runtime_create_secure`
/// does not build the PKI plugin from the identity files: the plugin
/// validates the identity (`validate_local_identity`, with the paths that are
/// set as the properties of Table 50), and the identity paths are not needed.
/// Returns 0, or -1 for a NULL argument, -2 for a plugin the runtime refuses
/// (the reason is on stderr).
///
/// # Safety
/// `cfg` is from `zerodds_security_config_create`; `plugin` points to a valid
/// struct.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_security_set_authentication_plugin(
    cfg: *mut crate::security_ffi::ZeroDdsSecurityConfig,
    plugin: *const c_void,
) -> i32 {
    // SAFETY: per the contract of the function.
    let (Some(cfg), Some(table)) = (unsafe { cfg.as_mut() }, unsafe {
        super::table_of::<ZeroDdsPluginAuthentication>(plugin)
    }) else {
        return -1;
    };
    match AuthenticationHandle::new(table) {
        Ok(handle) => {
            cfg.plugins.authentication = Some(handle);
            0
        }
        Err(e) => super::report("zerodds_security_set_authentication_plugin", &e),
    }
}

/// Hands the Cryptographic plugin `plugin` (a `ZeroDdsPluginCryptographic`)
/// to the security configuration `cfg`. With it, `zerodds_runtime_create_secure`
/// does not build the AES-GCM plugin. Needs an Authentication plugin of the
/// application too: the Cryptographic plugin of the application gets the
/// SharedSecret from the Authentication plugin it agrees with, not from the
/// builtin PKI plugin. Returns 0, or -1 for a NULL argument, -2 for a plugin
/// the runtime refuses (the reason is on stderr).
///
/// # Safety
/// `cfg` is from `zerodds_security_config_create`; `plugin` points to a valid
/// struct.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_security_set_cryptographic_plugin(
    cfg: *mut crate::security_ffi::ZeroDdsSecurityConfig,
    plugin: *const c_void,
) -> i32 {
    // SAFETY: per the contract of the function.
    let (Some(cfg), Some(table)) = (unsafe { cfg.as_mut() }, unsafe {
        super::table_of::<ZeroDdsPluginCryptographic>(plugin)
    }) else {
        return -1;
    };
    match CryptographicHandle::new(table) {
        Ok(handle) => {
            cfg.plugins.cryptographic = Some(handle);
            0
        }
        Err(e) => super::report("zerodds_security_set_cryptographic_plugin", &e),
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::undocumented_unsafe_blocks,
    missing_docs
)]
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};

    /// The sizes the C test programs assert for the same structs with
    /// `sizeof` (tests/plugin_c/plugin_test.c): a header that parts from the
    /// Rust layout fails there and here.
    #[test]
    fn the_structs_have_the_layout_of_the_header() {
        assert_eq!(
            (
                size_of::<ZeroDdsPluginBytes>(),
                align_of::<ZeroDdsPluginBytes>()
            ),
            (16, 8)
        );
        assert_eq!(size_of::<ZeroDdsPluginHandshakeStep>(), 32);
        assert_eq!(size_of::<ZeroDdsPluginAuthenticationListener>(), 24);
        assert_eq!(size_of::<ZeroDdsPluginEndpointAlgorithmInfo>(), 8);
        assert_eq!(size_of::<ZeroDdsPluginCryptoTokens>(), 24);
        assert_eq!(size_of::<ZeroDdsPluginEncodedPayload>(), 40);
        // abi_version and user, then one pointer per operation.
        assert_eq!(size_of::<ZeroDdsPluginAuthentication>(), 16 + 8 * 23);
        assert_eq!(size_of::<ZeroDdsPluginCryptographic>(), 16 + 8 * 28);
    }

    // ----- an Authentication plugin in Rust, behind the C functions --------

    // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
    unsafe extern "C" fn class_id(_: *mut c_void) -> *const c_char {
        c"DDS:Auth:RustTable".as_ptr()
    }

    // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
    unsafe extern "C" fn validate_remote(
        _: *mut c_void,
        local: Handle,
        guid: Guid,
        token: *const ZeroDdsPluginBytes,
        request: *const ZeroDdsPluginBytes,
        result: *mut i32,
        identity: *mut Handle,
        local_request: *mut ZeroDdsPluginBytes,
        _: Err,
    ) -> Status {
        static ANSWER: [u8; 3] = *b"REQ";
        // SAFETY: the adapter passes valid pointers.
        unsafe {
            assert_eq!(local, 11);
            assert_eq!(*guid, 0xAB);
            assert_eq!(slice_of((*token).data, (*token).len), b"remote-token");
            assert!(request.is_null());
            *result = 3;
            *identity = 12;
            *local_request = ZeroDdsPluginBytes {
                data: ANSWER.as_ptr(),
                len: 3,
            };
        }
        0
    }

    // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
    unsafe extern "C" fn begin_request(
        _: *mut c_void,
        initiator: Handle,
        replier: Handle,
        data: *const ZeroDdsPluginBytes,
        handshake: *mut Handle,
        step: *mut ZeroDdsPluginHandshakeStep,
        _: Err,
    ) -> Status {
        static TOKEN: [u8; 5] = *b"HELLO";
        // SAFETY: the adapter passes valid pointers.
        unsafe {
            assert_eq!((initiator, replier), (1, 2));
            assert_eq!(slice_of((*data).data, (*data).len), b"pdata");
            *handshake = 77;
            *step = ZeroDdsPluginHandshakeStep {
                kind: 0,
                token: ZeroDdsPluginBytes {
                    data: TOKEN.as_ptr(),
                    len: 5,
                },
                secret: 0,
            };
        }
        0
    }

    // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
    unsafe extern "C" fn process(
        _: *mut c_void,
        handshake: Handle,
        token: *const ZeroDdsPluginBytes,
        step: *mut ZeroDdsPluginHandshakeStep,
        err: Err,
    ) -> Status {
        // SAFETY: the adapter passes valid pointers.
        unsafe {
            assert_eq!(handshake, 77);
            if slice_of((*token).data, (*token).len) == b"BAD" {
                (*err).message = c"bad token".as_ptr();
                (*err).minor_code = 9;
                return 1;
            }
            *step = ZeroDdsPluginHandshakeStep {
                kind: 2,
                token: ZeroDdsPluginBytes {
                    data: core::ptr::null(),
                    len: 0,
                },
                secret: 99,
            };
        }
        0
    }

    // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
    unsafe extern "C" fn shared_secret(
        _: *mut c_void,
        handshake: Handle,
        out: *mut Handle,
        _: Err,
    ) -> Status {
        // SAFETY: the adapter passes a valid pointer.
        unsafe { *out = handshake + 1000 };
        0
    }

    // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
    unsafe extern "C" fn peer_credential(
        _: *mut c_void,
        _: Handle,
        out: *mut ZeroDdsPluginBytes,
        _: Err,
    ) -> Status {
        static TOKEN: [u8; 4] = *b"PEER";
        // SAFETY: the adapter passes a valid pointer.
        unsafe {
            *out = ZeroDdsPluginBytes {
                data: TOKEN.as_ptr(),
                len: 4,
            }
        };
        0
    }

    static PEER_BACK: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

    // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
    unsafe extern "C" fn return_peer(
        _: *mut c_void,
        token: *const ZeroDdsPluginBytes,
        _: Err,
    ) -> Status {
        // SAFETY: the adapter passes the struct the plugin gave.
        let given = unsafe { slice_of((*token).data, (*token).len) };
        assert_eq!(given, b"PEER");
        PEER_BACK.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        0
    }

    // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
    unsafe extern "C" fn nothing(_: *mut c_void) {}

    // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
    unsafe extern "C" fn unused_err(_: *mut c_void, _: Handle, _: Err) -> Status {
        0
    }

    fn authentication_table() -> ZeroDdsPluginAuthentication {
        // Every operation the adapter requires; those the test does not call
        // are the first that fits their shape.
        // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
        unsafe extern "C" fn validate_local(
            _: *mut c_void,
            _: u32,
            _: *const ZeroDdsPluginProperty,
            _: usize,
            _: *const ZeroDdsPluginBinaryProperty,
            _: usize,
            _: Guid,
            _: *mut i32,
            _: *mut Handle,
            _: *mut u8,
            _: Err,
        ) -> Status {
            6
        }
        // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
        unsafe extern "C" fn token(
            _: *mut c_void,
            _: Handle,
            _: *mut ZeroDdsPluginBytes,
            _: Err,
        ) -> Status {
            6
        }
        // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
        unsafe extern "C" fn config(
            _: *mut c_void,
            _: Handle,
            _: *const super::super::ZeroDdsPluginAlgorithmMasks,
            _: *mut super::super::ZeroDdsPluginAlgorithmMasks,
            _: Err,
        ) -> Status {
            6
        }
        // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
        unsafe extern "C" fn credential(
            _: *mut c_void,
            _: Handle,
            _: *const ZeroDdsPluginBytes,
            _: *const ZeroDdsPluginBytes,
            _: Err,
        ) -> Status {
            6
        }
        // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
        unsafe extern "C" fn begin_reply(
            _: *mut c_void,
            _: Handle,
            _: Handle,
            _: *const ZeroDdsPluginBytes,
            _: *const ZeroDdsPluginBytes,
            _: *mut Handle,
            _: *mut ZeroDdsPluginHandshakeStep,
            _: Err,
        ) -> Status {
            6
        }
        // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
        unsafe extern "C" fn listener(
            _: *mut c_void,
            _: *const ZeroDdsPluginAuthenticationListener,
            _: Err,
        ) -> Status {
            0
        }
        // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
        unsafe extern "C" fn return_token(
            _: *mut c_void,
            _: *const ZeroDdsPluginBytes,
            _: Err,
        ) -> Status {
            0
        }
        ZeroDdsPluginAuthentication {
            abi_version: ZERODDS_PLUGIN_ABI_VERSION,
            user: core::ptr::null_mut(),
            plugin_class_id: Some(class_id),
            validate_local_identity: Some(validate_local),
            get_identity_token: Some(token),
            get_identity_status_token: Some(token),
            set_participant_security_config: Some(config),
            set_permissions_credential_and_token: Some(credential),
            validate_remote_identity: Some(validate_remote),
            begin_handshake_request: Some(begin_request),
            begin_handshake_reply: Some(begin_reply),
            process_handshake: Some(process),
            get_shared_secret: Some(shared_secret),
            get_authenticated_peer_credential_token: Some(peer_credential),
            set_listener: Some(listener),
            return_identity_token: Some(return_token),
            return_identity_status_token: Some(return_token),
            return_authenticated_peer_credential_token: Some(return_peer),
            return_handshake_handle: Some(unused_err),
            return_identity_handle: Some(unused_err),
            return_sharedsecret_handle: Some(unused_err),
            get_permissions_token: None,
            get_shared_secret_bytes: None,
            get_shared_secret_challenges: None,
            destroy: Some(nothing),
        }
    }

    /// The handshake operations reach the plugin with their arguments and
    /// give back what the plugin answers: the steps, the handles, the tokens.
    #[test]
    fn the_handshake_goes_through_the_c_functions() {
        let handle = AuthenticationHandle::new(authentication_table()).unwrap();
        let mut plugin = handle.instantiate().unwrap();
        assert_eq!(plugin.plugin_class_id(), "DDS:Auth:RustTable");

        let remote = plugin
            .validate_remote_identity(IdentityHandle(11), [0xAB; 16], b"remote-token", None)
            .unwrap();
        assert_eq!(remote.result, ValidationResult::PendingHandshakeRequest);
        assert_eq!(remote.identity, IdentityHandle(12));
        assert_eq!(
            remote.local_auth_request_token.as_deref(),
            Some(&b"REQ"[..])
        );

        let (shake, step) = plugin
            .begin_handshake_request(IdentityHandle(1), IdentityHandle(2), b"pdata")
            .unwrap();
        assert_eq!(shake, HandshakeHandle(77));
        assert!(
            matches!(step, HandshakeStepOutcome::SendMessage { ref token } if token == b"HELLO")
        );

        let done = plugin.process_handshake(shake, b"ANY").unwrap();
        assert!(
            matches!(done, HandshakeStepOutcome::Complete { secret } if secret == SharedSecretHandle(99))
        );
        // A failing step is the error of the plugin, with its code, message and minor code.
        let error = plugin.process_handshake(shake, b"BAD").unwrap_err();
        assert_eq!((error.code, error.minor_code), (1, 9));
        assert_eq!(error.detail, "bad token");

        assert_eq!(
            plugin.shared_secret(shake).unwrap(),
            SharedSecretHandle(1077)
        );
        let credential = plugin
            .get_authenticated_peer_credential_token(shake)
            .unwrap();
        assert_eq!(credential, b"PEER");
        plugin
            .return_authenticated_peer_credential_token(&credential)
            .unwrap();
        assert_eq!(PEER_BACK.load(core::sync::atomic::Ordering::SeqCst), 1);
        // A token the plugin did not issue is refused before it reaches the plugin.
        assert!(
            plugin
                .return_authenticated_peer_credential_token(b"OTHER")
                .is_err()
        );
    }

    // ----- a Cryptographic plugin in Rust, behind the C functions ---------

    // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
    unsafe extern "C" fn tokens(
        _: *mut c_void,
        a: Handle,
        b: Handle,
        revision: i32,
        out: *mut ZeroDdsPluginCryptoTokens,
        _: Err,
    ) -> Status {
        static CLASS: &core::ffi::CStr = c"DDS:Crypto:RustTable";
        static HOLDER: ZeroDdsHolderSync = ZeroDdsHolderSync(ZeroDdsPluginDataHolder {
            class_id: CLASS.as_ptr(),
            properties: core::ptr::null(),
            n_properties: 0,
            binary_properties: core::ptr::null(),
            n_binary_properties: 0,
        });
        assert_eq!((a, b, revision), (5, 6, 3));
        // SAFETY: the adapter passes a valid pointer.
        unsafe {
            *out = ZeroDdsPluginCryptoTokens {
                present: true,
                tokens: &raw const HOLDER.0,
                n: 1,
            }
        };
        0
    }

    struct ZeroDdsHolderSync(ZeroDdsPluginDataHolder);
    // SAFETY: a static with a pointer to a static string, only read.
    unsafe impl Sync for ZeroDdsHolderSync {}

    static TOKENS_BACK: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

    // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
    unsafe extern "C" fn return_tokens(
        _: *mut c_void,
        tokens: *const ZeroDdsPluginDataHolder,
        n: usize,
        ok: *mut bool,
        _: Err,
    ) -> Status {
        // SAFETY: the adapter passes the list the plugin gave.
        unsafe {
            assert_eq!(n, 1);
            assert_eq!(
                string_of((*tokens).class_id).unwrap(),
                "DDS:Crypto:RustTable"
            );
            *ok = true;
        }
        TOKENS_BACK.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        0
    }

    // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
    unsafe extern "C" fn encode_submessage(
        _: *mut c_void,
        plain: *const ZeroDdsPluginBytes,
        writer: Handle,
        readers: *const Handle,
        n: usize,
        index: *mut usize,
        out: *mut ZeroDdsPluginBytes,
        ok: *mut bool,
        _: Err,
    ) -> Status {
        static OUT: [u8; 3] = *b"ENC";
        // SAFETY: the adapter passes valid pointers.
        unsafe {
            assert_eq!(slice_of((*plain).data, (*plain).len), b"plain");
            assert_eq!(writer, 8);
            assert_eq!(slice_of(readers, n), &[21, 22]);
            *index += 2;
            *out = ZeroDdsPluginBytes {
                data: OUT.as_ptr(),
                len: 3,
            };
            *ok = true;
        }
        0
    }

    // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
    unsafe extern "C" fn preprocess(
        _: *mut c_void,
        _: *const ZeroDdsPluginBytes,
        receiving: Handle,
        sending: Handle,
        present: *mut bool,
        writer: *mut Handle,
        reader: *mut Handle,
        category: *mut i32,
        _: Err,
    ) -> Status {
        // SAFETY: the adapter passes valid pointers.
        unsafe {
            assert_eq!((receiving, sending), (3, 4));
            *present = true;
            *writer = 41;
            *reader = 42;
            *category = 2;
        }
        0
    }

    fn cryptographic_table() -> ZeroDdsPluginCryptographic {
        // The shapes the test does not call fail with "not implemented".
        // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
        unsafe extern "C" fn flag(_: *mut c_void, _: Handle, _: *mut bool, _: Err) -> Status {
            6
        }
        // SAFETY: the adapter calls this with the pointers of its own call, valid for it.
        unsafe extern "C" fn set_tokens(
            _: *mut c_void,
            _: Handle,
            _: Handle,
            _: *const ZeroDdsPluginDataHolder,
            _: usize,
            _: *mut bool,
            _: Err,
        ) -> Status {
            6
        }
        macro_rules! missing_table {
            () => {
                ZeroDdsPluginCryptographic {
                    abi_version: ZERODDS_PLUGIN_ABI_VERSION,
                    user: core::ptr::null_mut(),
                    register_local_participant: None,
                    register_matched_remote_participant: None,
                    register_local_datawriter: None,
                    register_matched_remote_datareader: None,
                    register_local_datareader: None,
                    register_matched_remote_datawriter: None,
                    revise_local_entity_keys: None,
                    activate_key_revision: None,
                    unregister_participant: Some(flag),
                    unregister_datawriter: Some(flag),
                    unregister_datareader: Some(flag),
                    create_local_participant_crypto_tokens: Some(tokens),
                    set_remote_participant_crypto_tokens: Some(set_tokens),
                    create_local_datawriter_crypto_tokens: Some(tokens),
                    set_remote_datawriter_crypto_tokens: Some(set_tokens),
                    create_local_datareader_crypto_tokens: Some(tokens),
                    set_remote_datareader_crypto_tokens: Some(set_tokens),
                    return_crypto_tokens: Some(return_tokens),
                    encode_serialized_payload: None,
                    encode_datawriter_submessage: Some(encode_submessage),
                    encode_datareader_submessage: None,
                    encode_rtps_message: None,
                    decode_rtps_message: None,
                    preprocess_secure_submsg: Some(preprocess),
                    decode_datawriter_submessage: None,
                    decode_datareader_submessage: None,
                    decode_serialized_payload: None,
                    destroy: Some(nothing),
                }
            };
        }
        missing_table!()
    }

    /// Tokens, a submessage encoded for a list of readers and the preprocessing
    /// of a secure submessage reach the plugin with their arguments.
    #[test]
    fn tokens_and_transforms_go_through_the_c_functions() {
        // The adapter needs every operation: take the table with the missing
        // ones filled by one that fails, so that `new` accepts it.
        let mut table = cryptographic_table();
        let table_copy = table;
        assert!(
            CryptographicHandle::new(table_copy).is_err(),
            "functions missing"
        );
        // A plugin built for the call: only the shapes below are present, so
        // the adapter is made from the cell directly.
        table.register_local_participant = None;
        let cell = Arc::new(PluginCell::new(table));
        let mut plugin = CCryptographic {
            cell,
            issued: Vec::new(),
        };

        let created = plugin
            .create_local_datawriter_crypto_tokens(
                DatawriterCryptoHandle(5),
                DatareaderCryptoHandle(6),
                3,
            )
            .unwrap()
            .expect("tokens");
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].class_id, "DDS:Crypto:RustTable");
        assert!(plugin.return_crypto_tokens(created).unwrap());
        assert_eq!(TOKENS_BACK.load(core::sync::atomic::Ordering::SeqCst), 1);
        assert!(
            plugin.return_crypto_tokens(Vec::new()).is_err(),
            "not issued"
        );

        let mut out = Vec::new();
        let mut index = 0usize;
        let ok = plugin
            .encode_datawriter_submessage(
                &mut out,
                b"plain",
                DatawriterCryptoHandle(8),
                &[DatareaderCryptoHandle(21), DatareaderCryptoHandle(22)],
                &mut index,
            )
            .unwrap();
        assert!(ok);
        assert_eq!((out.as_slice(), index), (&b"ENC"[..], 2));

        let pre = plugin
            .preprocess_secure_submsg(b"x", ParticipantCryptoHandle(3), ParticipantCryptoHandle(4))
            .unwrap()
            .expect("answer");
        assert_eq!(pre.datawriter_crypto, DatawriterCryptoHandle(41));
        assert_eq!(pre.datareader_crypto, DatareaderCryptoHandle(42));
        assert_eq!(
            pre.secure_submessage_category,
            SecureSubmessageCategory::DatareaderSubmessage
        );
        // An operation the plugin answers with "not implemented" is that error.
        let error = plugin
            .unregister_datawriter(DatawriterCryptoHandle(1))
            .unwrap_err();
        assert_eq!(error.code, 6);
    }
}
