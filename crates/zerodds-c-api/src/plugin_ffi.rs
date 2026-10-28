// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Plugins of an application in C, C++ and Java.
//!
//! DDS-Security 1.2 clause 8 and clause 9 define the plugin interfaces and
//! clause 11 their language representations. This module is the C side of
//! placing a plugin of an application into a participant: the structs of
//! `include/zerodds_security_plugins.h` (AccessControl, Logging,
//! DataTagging), mirrored here with the same layout, and an adapter for each
//! that implements the Rust trait of the plugin by calling the functions of
//! the struct. C++ and Java reach the same structs: C++ through
//! `include/zerodds_security_plugins.hpp`, Java through the foreign function
//! interface of the JDK (`java/org/zerodds/security`).
//!
//! # Threads
//!
//! The functions of one plugin are called one at a time: the adapter holds a
//! lock around every call, so a plugin does not need its own.
//!
//! # Memory
//!
//! What the runtime passes in lives for the call. What a plugin hands out (a
//! token, a config) is copied at once and given back with the matching
//! `return_*` operation, as the specification says (9.4.2.9.18 and the
//! following); the adapter keeps the plugin's own struct for that call.

extern crate alloc;

use core::ffi::{c_char, c_void};
use std::borrow::Cow;
use std::ffi::{CStr, CString};
use std::sync::{Arc, Mutex, MutexGuard};

use zerodds_security::access_control::{
    AccessControlListener, AccessControlPlugin, AccessDecision, CryptoAlgorithmRequirements,
    DigitalSignatureAlgorithmInfo, EndpointRequest, EndpointSecurityAlgorithmInfo,
    EndpointSecurityConfig, EndpointSymmetricCipherAlgorithmInfo, KeyEstablishmentAlgorithmInfo,
    ParticipantRequest, ParticipantSecurityAlgorithmInfo, ParticipantSecurityConfig,
    PermissionsHandle, RelayRole, RemoteReaderDecision, SymmetricCipherAlgorithmInfo, TopicRequest,
    TopicSecurityConfig,
};
use zerodds_security::authentication::IdentityHandle;
use zerodds_security::data_tagging::{DataTag, DataTaggingPlugin};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::logging::{LogLevel, LogOptions, LoggerListener, LoggingPlugin};
use zerodds_security::properties::Property;
use zerodds_security::token::{BinaryProperty, DataHolder, WireProperty};

/// The revision of the structs below, `ZERODDS_PLUGIN_ABI_VERSION` of the
/// header.
pub const ZERODDS_PLUGIN_ABI_VERSION: u32 = 1;

// ============================================================================
// The structs of the header
// ============================================================================

/// `ZeroDdsPluginError`.
#[repr(C)]
#[derive(Debug)]
pub struct ZeroDdsPluginError {
    /// NUL-terminated, valid until the next call; may be NULL.
    pub message: *const c_char,
    /// The plugin's own number.
    pub minor_code: i32,
}

/// `ZeroDdsPluginProperty`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginProperty {
    /// Name.
    pub name: *const c_char,
    /// Value.
    pub value: *const c_char,
    /// Whether it is propagated.
    pub propagate: bool,
}

/// `ZeroDdsPluginBinaryProperty`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginBinaryProperty {
    /// Name.
    pub name: *const c_char,
    /// Value.
    pub value: *const u8,
    /// Length of the value.
    pub value_len: usize,
    /// Whether it is propagated.
    pub propagate: bool,
}

/// `ZeroDdsPluginDataHolder`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginDataHolder {
    /// Class id.
    pub class_id: *const c_char,
    /// String properties.
    pub properties: *const ZeroDdsPluginProperty,
    /// Their number.
    pub n_properties: usize,
    /// Binary properties.
    pub binary_properties: *const ZeroDdsPluginBinaryProperty,
    /// Their number.
    pub n_binary_properties: usize,
}

/// `ZeroDdsPluginPair`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginPair {
    /// Name.
    pub name: *const c_char,
    /// Value.
    pub value: *const c_char,
}

/// `ZeroDdsPluginMasks`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ZeroDdsPluginMasks {
    /// Supported algorithms.
    pub supported_mask: u32,
    /// Required algorithms.
    pub required_mask: u32,
}

/// `ZeroDdsPluginAlgorithmMasks`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ZeroDdsPluginAlgorithmMasks {
    /// `trust_chain`.
    pub trust_chain: ZeroDdsPluginMasks,
    /// `message_auth`.
    pub message_auth: ZeroDdsPluginMasks,
    /// `key_establishment`.
    pub key_establishment: ZeroDdsPluginMasks,
}

/// `ZeroDdsPluginParticipantAlgorithmInfo`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ZeroDdsPluginParticipantAlgorithmInfo {
    /// `digital_signature.trust_chain`.
    pub trust_chain: ZeroDdsPluginMasks,
    /// `digital_signature.message_auth`.
    pub message_auth: ZeroDdsPluginMasks,
    /// `key_establishment.shared_secret`.
    pub shared_secret: ZeroDdsPluginMasks,
    /// `symmetric_cipher.supported_mask`.
    pub cipher_supported_mask: u32,
    /// `symmetric_cipher.builtin_endpoints_required_mask`.
    pub cipher_builtin_endpoints_required_mask: u32,
    /// `symmetric_cipher.builtin_kx_endpoints_required_mask`.
    pub cipher_builtin_kx_endpoints_required_mask: u32,
    /// `symmetric_cipher.user_endpoints_default_required_mask`.
    pub cipher_user_endpoints_default_required_mask: u32,
}

/// `ZeroDdsPluginParticipantRequest`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginParticipantRequest {
    /// Domain id.
    pub domain_id: u32,
    /// Domain tag.
    pub domain_tag: *const c_char,
    /// Participant GUID.
    pub participant_guid: [u8; 16],
    /// Properties of the participant QoS.
    pub properties: *const ZeroDdsPluginProperty,
    /// Their number.
    pub n_properties: usize,
    /// Whether `identity_algorithms` is set.
    pub has_identity_algorithms: bool,
    /// The algorithms of the identity.
    pub identity_algorithms: ZeroDdsPluginAlgorithmMasks,
}

/// `ZeroDdsPluginTopicRequest`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginTopicRequest {
    /// Domain id.
    pub domain_id: u32,
    /// Topic name.
    pub topic_name: *const c_char,
    /// QoS entries.
    pub qos: *const ZeroDdsPluginPair,
    /// Their number.
    pub n_qos: usize,
}

/// `ZeroDdsPluginEndpointRequest`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginEndpointRequest {
    /// Domain id.
    pub domain_id: u32,
    /// Topic name.
    pub topic_name: *const c_char,
    /// Partitions.
    pub partitions: *const *const c_char,
    /// Their number.
    pub n_partitions: usize,
    /// Data tags.
    pub data_tags: *const ZeroDdsPluginPair,
    /// Their number.
    pub n_data_tags: usize,
    /// QoS entries.
    pub qos: *const ZeroDdsPluginPair,
    /// Their number.
    pub n_qos: usize,
}

/// `ZeroDdsPluginParticipantConfig`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginParticipantConfig {
    /// `allow_unauthenticated_participants`.
    pub allow_unauthenticated_participants: bool,
    /// `is_access_protected`.
    pub is_access_protected: bool,
    /// `is_rtps_axk_protected`.
    pub is_rtps_axk_protected: bool,
    /// `is_rtps_psk_protected`.
    pub is_rtps_psk_protected: bool,
    /// `is_discovery_protected`.
    pub is_discovery_protected: bool,
    /// `is_liveliness_protected`.
    pub is_liveliness_protected: bool,
    /// `is_key_revision_enabled`.
    pub is_key_revision_enabled: bool,
    /// `plugin_participant_attributes`.
    pub plugin_participant_attributes: u32,
    /// `ac_participant_properties`.
    pub ac_participant_properties: *const ZeroDdsPluginProperty,
    /// Their number.
    pub n_ac_participant_properties: usize,
    /// `algorithm_info`.
    pub algorithm_info: ZeroDdsPluginParticipantAlgorithmInfo,
}

/// `ZeroDdsPluginTopicConfig`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ZeroDdsPluginTopicConfig {
    /// `is_read_protected`.
    pub is_read_protected: bool,
    /// `is_write_protected`.
    pub is_write_protected: bool,
    /// `is_discovery_protected`.
    pub is_discovery_protected: bool,
    /// `is_liveliness_protected`.
    pub is_liveliness_protected: bool,
}

/// `ZeroDdsPluginEndpointConfig`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginEndpointConfig {
    /// The config of the topic.
    pub topic: ZeroDdsPluginTopicConfig,
    /// `is_submessage_protected`.
    pub is_submessage_protected: bool,
    /// `is_payload_protected`.
    pub is_payload_protected: bool,
    /// `is_key_protected`.
    pub is_key_protected: bool,
    /// `plugin_endpoint_attributes`.
    pub plugin_endpoint_attributes: u32,
    /// `ac_endpoint_properties`.
    pub ac_endpoint_properties: *const ZeroDdsPluginProperty,
    /// Their number.
    pub n_ac_endpoint_properties: usize,
    /// `algorithm_info.symmetric_cipher.required_mask`.
    pub cipher_required_mask: u32,
    /// `algorithm_info.symmetric_cipher.supported_mask`.
    pub cipher_supported_mask: u32,
}

/// `ZeroDdsPluginAccessControlListener`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginAccessControlListener {
    /// Handed to the function.
    pub context: *mut c_void,
    /// `on_revoke_permissions`.
    pub on_revoke_permissions:
        Option<unsafe extern "C" fn(*mut c_void, *const c_char, u64) -> bool>,
}

type Status = i32;
type Err = *mut ZeroDdsPluginError;
type Bool = *mut bool;
type Handle = u64;
type ParticipantReq = *const ZeroDdsPluginParticipantRequest;
type EndpointReq = *const ZeroDdsPluginEndpointRequest;
type TopicReq = *const ZeroDdsPluginTopicRequest;
type EndpointCheckFn = unsafe extern "C" fn(*mut c_void, Handle, EndpointReq, Bool, Err) -> Status;
type ParticipantCheckFn =
    unsafe extern "C" fn(*mut c_void, Handle, ParticipantReq, Bool, Err) -> Status;
type TopicCheckFn = unsafe extern "C" fn(*mut c_void, Handle, TopicReq, Bool, Err) -> Status;
type InstanceCheckFn =
    unsafe extern "C" fn(*mut c_void, Handle, Guid, *const u8, usize, Bool, Err) -> Status;
type RemoteInstanceCheckFn =
    unsafe extern "C" fn(*mut c_void, Handle, Guid, Guid, *const u8, usize, Bool, Err) -> Status;
type MatchCheckFn = unsafe extern "C" fn(
    *mut c_void,
    Handle,
    Handle,
    EndpointReq,
    EndpointReq,
    Bool,
    Err,
) -> Status;
type Presence = fn(&ZeroDdsPluginAccessControl) -> bool;
type Guid = *const u8;

/// `ZeroDdsPluginAccessControl`: the AccessControl plugin of an application.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ZeroDdsPluginAccessControl {
    /// Must be [`ZERODDS_PLUGIN_ABI_VERSION`].
    pub abi_version: u32,
    /// Handed to every function.
    pub user: *mut c_void,
    /// `plugin_class_id`.
    pub plugin_class_id: Option<unsafe extern "C" fn(*mut c_void) -> *const c_char>,
    /// `validate_local_permissions`.
    pub validate_local_permissions: Option<
        unsafe extern "C" fn(*mut c_void, Handle, ParticipantReq, *mut Handle, Err) -> Status,
    >,
    /// `validate_remote_permissions`.
    pub validate_remote_permissions: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            Handle,
            *const ZeroDdsPluginDataHolder,
            *const ZeroDdsPluginDataHolder,
            *mut Handle,
            Err,
        ) -> Status,
    >,
    /// `check_create_participant`.
    pub check_create_participant:
        Option<unsafe extern "C" fn(*mut c_void, Handle, ParticipantReq, Bool, Err) -> Status>,
    /// `check_create_datawriter`.
    pub check_create_datawriter:
        Option<unsafe extern "C" fn(*mut c_void, Handle, EndpointReq, Bool, Err) -> Status>,
    /// `check_create_datareader`.
    pub check_create_datareader:
        Option<unsafe extern "C" fn(*mut c_void, Handle, EndpointReq, Bool, Err) -> Status>,
    /// `check_create_topic`.
    pub check_create_topic:
        Option<unsafe extern "C" fn(*mut c_void, Handle, TopicReq, Bool, Err) -> Status>,
    /// `check_local_datawriter_register_instance`.
    pub check_local_datawriter_register_instance: Option<
        unsafe extern "C" fn(*mut c_void, Handle, Guid, *const u8, usize, Bool, Err) -> Status,
    >,
    /// `check_local_datawriter_dispose_instance`.
    pub check_local_datawriter_dispose_instance: Option<
        unsafe extern "C" fn(*mut c_void, Handle, Guid, *const u8, usize, Bool, Err) -> Status,
    >,
    /// `check_remote_participant`.
    pub check_remote_participant:
        Option<unsafe extern "C" fn(*mut c_void, Handle, ParticipantReq, Bool, Err) -> Status>,
    /// `check_remote_datawriter`.
    pub check_remote_datawriter:
        Option<unsafe extern "C" fn(*mut c_void, Handle, EndpointReq, Bool, Err) -> Status>,
    /// `check_remote_datareader`.
    pub check_remote_datareader:
        Option<unsafe extern "C" fn(*mut c_void, Handle, EndpointReq, *mut i32, Err) -> Status>,
    /// `check_remote_topic`.
    pub check_remote_topic:
        Option<unsafe extern "C" fn(*mut c_void, Handle, TopicReq, Bool, Err) -> Status>,
    /// `check_local_datawriter_match`.
    pub check_local_datawriter_match: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            Handle,
            EndpointReq,
            EndpointReq,
            Bool,
            Err,
        ) -> Status,
    >,
    /// `check_local_datareader_match`.
    pub check_local_datareader_match: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            Handle,
            EndpointReq,
            EndpointReq,
            Bool,
            Err,
        ) -> Status,
    >,
    /// `check_remote_datawriter_register_instance`.
    pub check_remote_datawriter_register_instance: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            Guid,
            Guid,
            *const u8,
            usize,
            Bool,
            Err,
        ) -> Status,
    >,
    /// `check_remote_datawriter_dispose_instance`.
    pub check_remote_datawriter_dispose_instance: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            Guid,
            Guid,
            *const u8,
            usize,
            Bool,
            Err,
        ) -> Status,
    >,
    /// `check_relay`.
    pub check_relay:
        Option<unsafe extern "C" fn(*mut c_void, Handle, i32, EndpointReq, Bool, Err) -> Status>,
    /// `get_permissions_token`.
    pub get_permissions_token: Option<
        unsafe extern "C" fn(*mut c_void, Handle, *mut ZeroDdsPluginDataHolder, Err) -> Status,
    >,
    /// `get_permissions_credential_token`.
    pub get_permissions_credential_token: Option<
        unsafe extern "C" fn(*mut c_void, Handle, *mut ZeroDdsPluginDataHolder, Err) -> Status,
    >,
    /// `set_listener`.
    pub set_listener: Option<
        unsafe extern "C" fn(*mut c_void, *const ZeroDdsPluginAccessControlListener, Err) -> Status,
    >,
    /// `return_permissions_token`.
    pub return_permissions_token:
        Option<unsafe extern "C" fn(*mut c_void, *const ZeroDdsPluginDataHolder, Err) -> Status>,
    /// `return_permissions_credential_token`.
    pub return_permissions_credential_token:
        Option<unsafe extern "C" fn(*mut c_void, *const ZeroDdsPluginDataHolder, Err) -> Status>,
    /// `get_participant_security_config`.
    pub get_participant_security_config: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            *mut ZeroDdsPluginParticipantConfig,
            Err,
        ) -> Status,
    >,
    /// `get_topic_security_config`.
    pub get_topic_security_config: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            *const c_char,
            *mut ZeroDdsPluginTopicConfig,
            Err,
        ) -> Status,
    >,
    /// `get_datawriter_security_config`.
    pub get_datawriter_security_config: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            EndpointReq,
            *mut ZeroDdsPluginEndpointConfig,
            Err,
        ) -> Status,
    >,
    /// `get_datareader_security_config`.
    pub get_datareader_security_config: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Handle,
            EndpointReq,
            *mut ZeroDdsPluginEndpointConfig,
            Err,
        ) -> Status,
    >,
    /// `return_participant_security_config`.
    pub return_participant_security_config: Option<
        unsafe extern "C" fn(*mut c_void, *const ZeroDdsPluginParticipantConfig, Err) -> Status,
    >,
    /// `return_topic_security_config`.
    pub return_topic_security_config:
        Option<unsafe extern "C" fn(*mut c_void, *const ZeroDdsPluginTopicConfig, Err) -> Status>,
    /// `return_datawriter_security_config`.
    pub return_datawriter_security_config: Option<
        unsafe extern "C" fn(*mut c_void, *const ZeroDdsPluginEndpointConfig, Err) -> Status,
    >,
    /// `return_datareader_security_config`.
    pub return_datareader_security_config: Option<
        unsafe extern "C" fn(*mut c_void, *const ZeroDdsPluginEndpointConfig, Err) -> Status,
    >,
    /// Called once when the plugin is released.
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}

/// `ZeroDdsPluginLogOptions`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginLogOptions {
    /// `log_level` (`ZERODDS_LOG_*`).
    pub log_level: i32,
    /// `log_file`, NULL for none.
    pub log_file: *const c_char,
    /// `distribute`.
    pub distribute: bool,
}

/// `ZeroDdsPluginLoggerListener`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginLoggerListener {
    /// Handed to the function.
    pub context: *mut c_void,
    /// `log_message`.
    pub log_message: Option<
        unsafe extern "C" fn(*mut c_void, i32, *const u8, *const c_char, *const c_char) -> i32,
    >,
}

/// `ZeroDdsPluginLogging`: the Logging plugin of an application.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ZeroDdsPluginLogging {
    /// Must be [`ZERODDS_PLUGIN_ABI_VERSION`].
    pub abi_version: u32,
    /// Handed to every function.
    pub user: *mut c_void,
    /// `plugin_class_id`.
    pub plugin_class_id: Option<unsafe extern "C" fn(*mut c_void) -> *const c_char>,
    /// `set_log_options`.
    pub set_log_options:
        Option<unsafe extern "C" fn(*mut c_void, *const ZeroDdsPluginLogOptions, Err) -> Status>,
    /// `enable_logging`.
    pub enable_logging: Option<unsafe extern "C" fn(*mut c_void, Err) -> Status>,
    /// `set_listener`.
    pub set_listener: Option<
        unsafe extern "C" fn(*mut c_void, *const ZeroDdsPluginLoggerListener, Err) -> Status,
    >,
    /// `log`.
    pub log:
        Option<unsafe extern "C" fn(*mut c_void, i32, *const u8, *const c_char, *const c_char)>,
    /// Called once when the plugin is released.
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}

/// `ZeroDdsPluginDataTag`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPluginDataTag {
    /// Name.
    pub name: *const c_char,
    /// Value.
    pub value: *const c_char,
}

/// `ZeroDdsPluginDataTagging`: the DataTagging plugin of an application.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ZeroDdsPluginDataTagging {
    /// Must be [`ZERODDS_PLUGIN_ABI_VERSION`].
    pub abi_version: u32,
    /// Handed to every function.
    pub user: *mut c_void,
    /// `plugin_class_id`.
    pub plugin_class_id: Option<unsafe extern "C" fn(*mut c_void) -> *const c_char>,
    /// `set_tags`.
    pub set_tags:
        Option<unsafe extern "C" fn(*mut c_void, *const u8, *const ZeroDdsPluginDataTag, usize)>,
    /// `get_tags`.
    pub get_tags: Option<
        unsafe extern "C" fn(*mut c_void, *const u8, *mut *const ZeroDdsPluginDataTag, *mut usize),
    >,
    /// Called once when the plugin is released.
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}

// ============================================================================
// Conversions
// ============================================================================

fn bad(detail: &'static str) -> SecurityError {
    SecurityError::new(SecurityErrorKind::BadArgument, detail)
}

fn kind_of_code(code: i32) -> SecurityErrorKind {
    match code {
        1 => SecurityErrorKind::AuthenticationFailed,
        2 => SecurityErrorKind::AccessDenied,
        3 => SecurityErrorKind::CryptoFailed,
        4 => SecurityErrorKind::InvalidConfiguration,
        5 => SecurityErrorKind::BadArgument,
        6 => SecurityErrorKind::NotImplemented,
        _ => SecurityErrorKind::Internal,
    }
}

/// An error struct to hand a plugin.
fn blank_error() -> ZeroDdsPluginError {
    ZeroDdsPluginError {
        message: core::ptr::null(),
        minor_code: 0,
    }
}

/// The result of a plugin operation: `status` 0 is success; any other is the
/// `code` of the SecurityException with the message and minor code of `err`.
fn check(status: Status, err: &ZeroDdsPluginError) -> SecurityResult<()> {
    if status == 0 {
        return Ok(());
    }
    let message = if err.message.is_null() {
        Cow::Borrowed("the plugin of the application failed")
    } else {
        // SAFETY: the plugin gives a NUL-terminated string that is valid
        // until the next call (header, "Status").
        let text = unsafe { CStr::from_ptr(err.message) };
        Cow::Owned(text.to_string_lossy().into_owned())
    };
    let kind = kind_of_code(status);
    let mut error = SecurityError::new(kind, message).with_minor_code(err.minor_code);
    error.code = status;
    Err(error)
}

fn missing(operation: &'static str) -> SecurityError {
    SecurityError::new(
        SecurityErrorKind::NotImplemented,
        Cow::Owned(format!("the plugin of the application has no {operation}")),
    )
}

/// A NUL-terminated copy for the C side.
fn cstring(text: &str) -> SecurityResult<CString> {
    CString::new(text)
        .map_err(|_| bad("a string given to a plugin of the application contains a NUL"))
}

/// A string of the plugin.
fn string_of(text: *const c_char) -> SecurityResult<String> {
    if text.is_null() {
        return Err(bad("a plugin of the application gave a NULL string"));
    }
    // SAFETY: NUL-terminated, valid for the call (header, "Memory").
    unsafe { CStr::from_ptr(text) }
        .to_str()
        .map(str::to_owned)
        .map_err(|_| bad("a plugin of the application gave a string that is not UTF-8"))
}

/// A slice of the plugin.
fn slice_of<'a, T>(data: *const T, len: usize) -> &'a [T] {
    if data.is_null() || len == 0 {
        return &[];
    }
    // SAFETY: the plugin gives `len` elements at `data`, valid until the
    // return_* call that gives them back.
    unsafe { core::slice::from_raw_parts(data, len) }
}

/// The C view of a list of properties.
struct CProperties {
    _strings: Vec<CString>,
    items: Vec<ZeroDdsPluginProperty>,
}

impl CProperties {
    fn new<'a>(items: impl IntoIterator<Item = (&'a str, &'a str, bool)>) -> SecurityResult<Self> {
        let mut strings = Vec::new();
        let mut raw = Vec::new();
        for (name, value, propagate) in items {
            strings.push(cstring(name)?);
            strings.push(cstring(value)?);
            raw.push(propagate);
        }
        let items = raw
            .iter()
            .enumerate()
            .map(|(i, propagate)| ZeroDdsPluginProperty {
                name: strings[2 * i].as_ptr(),
                value: strings[2 * i + 1].as_ptr(),
                propagate: *propagate,
            })
            .collect();
        Ok(Self {
            _strings: strings,
            items,
        })
    }

    fn ptr(&self) -> *const ZeroDdsPluginProperty {
        self.items.as_ptr()
    }

    fn len(&self) -> usize {
        self.items.len()
    }
}

/// The C view of a list of name/value pairs.
struct CPairs {
    _strings: Vec<CString>,
    items: Vec<ZeroDdsPluginPair>,
}

impl CPairs {
    fn new(pairs: &[(&str, &str)]) -> SecurityResult<Self> {
        let mut strings = Vec::new();
        for (name, value) in pairs {
            strings.push(cstring(name)?);
            strings.push(cstring(value)?);
        }
        let items = (0..pairs.len())
            .map(|i| ZeroDdsPluginPair {
                name: strings[2 * i].as_ptr(),
                value: strings[2 * i + 1].as_ptr(),
            })
            .collect();
        Ok(Self {
            _strings: strings,
            items,
        })
    }
}

fn properties_of(data: *const ZeroDdsPluginProperty, len: usize) -> SecurityResult<Vec<Property>> {
    slice_of(data, len)
        .iter()
        .map(|p| {
            Ok(Property {
                name: Cow::Owned(string_of(p.name)?),
                value: Cow::Owned(string_of(p.value)?),
                propagate: p.propagate,
            })
        })
        .collect()
}

fn masks_of(m: ZeroDdsPluginMasks) -> CryptoAlgorithmRequirements {
    CryptoAlgorithmRequirements {
        supported_mask: m.supported_mask,
        required_mask: m.required_mask,
    }
}

/// The C view of a participant request.
struct CParticipantRequest {
    _domain_tag: CString,
    _properties: CProperties,
    raw: ZeroDdsPluginParticipantRequest,
}

impl CParticipantRequest {
    fn new(request: &ParticipantRequest<'_>) -> SecurityResult<Self> {
        let domain_tag = cstring(request.domain_tag)?;
        let properties = CProperties::new(
            request
                .properties
                .entries()
                .iter()
                .map(|p| (p.name.as_ref(), p.value.as_ref(), p.propagate)),
        )?;
        let masks = request
            .identity_algorithms
            .map(|a| ZeroDdsPluginAlgorithmMasks {
                trust_chain: pair_masks(a.trust_chain),
                message_auth: pair_masks(a.message_auth),
                key_establishment: pair_masks(a.key_establishment),
            });
        let raw = ZeroDdsPluginParticipantRequest {
            domain_id: request.domain_id,
            domain_tag: domain_tag.as_ptr(),
            participant_guid: request.participant_guid,
            properties: properties.ptr(),
            n_properties: properties.len(),
            has_identity_algorithms: masks.is_some(),
            identity_algorithms: masks.unwrap_or_default(),
        };
        Ok(Self {
            _domain_tag: domain_tag,
            _properties: properties,
            raw,
        })
    }
}

fn pair_masks((supported, required): (u32, u32)) -> ZeroDdsPluginMasks {
    ZeroDdsPluginMasks {
        supported_mask: supported,
        required_mask: required,
    }
}

/// The C view of a topic request.
struct CTopicRequest {
    _topic: CString,
    _qos: CPairs,
    raw: ZeroDdsPluginTopicRequest,
}

impl CTopicRequest {
    fn new(request: &TopicRequest<'_>) -> SecurityResult<Self> {
        let topic = cstring(request.topic_name)?;
        let qos = CPairs::new(request.qos)?;
        let raw = ZeroDdsPluginTopicRequest {
            domain_id: request.domain_id,
            topic_name: topic.as_ptr(),
            qos: qos.items.as_ptr(),
            n_qos: qos.items.len(),
        };
        Ok(Self {
            _topic: topic,
            _qos: qos,
            raw,
        })
    }
}

/// The C view of an endpoint request.
struct CEndpointRequest {
    _topic: CString,
    _partitions: Vec<CString>,
    _partition_ptrs: Vec<*const c_char>,
    _tags: CPairs,
    _qos: CPairs,
    raw: ZeroDdsPluginEndpointRequest,
}

impl CEndpointRequest {
    fn new(request: &EndpointRequest<'_>) -> SecurityResult<Self> {
        let topic = cstring(request.topic_name)?;
        let partitions = request
            .partitions
            .iter()
            .map(|p| cstring(p))
            .collect::<SecurityResult<Vec<_>>>()?;
        let partition_ptrs: Vec<*const c_char> = partitions.iter().map(|p| p.as_ptr()).collect();
        let tags = CPairs::new(request.data_tags)?;
        let qos = CPairs::new(request.qos)?;
        let raw = ZeroDdsPluginEndpointRequest {
            domain_id: request.domain_id,
            topic_name: topic.as_ptr(),
            partitions: partition_ptrs.as_ptr(),
            n_partitions: partition_ptrs.len(),
            data_tags: tags.items.as_ptr(),
            n_data_tags: tags.items.len(),
            qos: qos.items.as_ptr(),
            n_qos: qos.items.len(),
        };
        Ok(Self {
            _topic: topic,
            _partitions: partitions,
            _partition_ptrs: partition_ptrs,
            _tags: tags,
            _qos: qos,
            raw,
        })
    }
}

/// The C view of a DataHolder.
struct CDataHolder {
    _class_id: CString,
    _properties: CProperties,
    _binary_names: Vec<CString>,
    _binary: Vec<ZeroDdsPluginBinaryProperty>,
    raw: ZeroDdsPluginDataHolder,
}

impl CDataHolder {
    fn new(holder: &DataHolder) -> SecurityResult<Self> {
        let class_id = cstring(&holder.class_id)?;
        let properties = CProperties::new(
            holder
                .properties
                .iter()
                .map(|p| (p.name.as_str(), p.value.as_str(), false)),
        )?;
        let names = holder
            .binary_properties
            .iter()
            .map(|p| cstring(&p.name))
            .collect::<SecurityResult<Vec<_>>>()?;
        let binary: Vec<ZeroDdsPluginBinaryProperty> = holder
            .binary_properties
            .iter()
            .zip(&names)
            .map(|(p, name)| ZeroDdsPluginBinaryProperty {
                name: name.as_ptr(),
                value: p.value.as_ptr(),
                value_len: p.value.len(),
                propagate: p.propagate,
            })
            .collect();
        let raw = ZeroDdsPluginDataHolder {
            class_id: class_id.as_ptr(),
            properties: properties.ptr(),
            n_properties: properties.len(),
            binary_properties: binary.as_ptr(),
            n_binary_properties: binary.len(),
        };
        Ok(Self {
            _class_id: class_id,
            _properties: properties,
            _binary_names: names,
            _binary: binary,
            raw,
        })
    }
}

fn holder_of(raw: &ZeroDdsPluginDataHolder) -> SecurityResult<DataHolder> {
    Ok(DataHolder {
        class_id: string_of(raw.class_id)?,
        properties: slice_of(raw.properties, raw.n_properties)
            .iter()
            .map(|p| {
                Ok(WireProperty {
                    name: string_of(p.name)?,
                    value: string_of(p.value)?,
                })
            })
            .collect::<SecurityResult<Vec<_>>>()?,
        binary_properties: slice_of(raw.binary_properties, raw.n_binary_properties)
            .iter()
            .map(|p| {
                Ok(BinaryProperty {
                    name: string_of(p.name)?,
                    value: slice_of(p.value, p.value_len).to_vec(),
                    propagate: p.propagate,
                })
            })
            .collect::<SecurityResult<Vec<_>>>()?,
    })
}

fn participant_config_of(
    raw: &ZeroDdsPluginParticipantConfig,
) -> SecurityResult<ParticipantSecurityConfig> {
    let a = &raw.algorithm_info;
    Ok(ParticipantSecurityConfig {
        allow_unauthenticated_participants: raw.allow_unauthenticated_participants,
        is_access_protected: raw.is_access_protected,
        is_rtps_axk_protected: raw.is_rtps_axk_protected,
        is_rtps_psk_protected: raw.is_rtps_psk_protected,
        is_discovery_protected: raw.is_discovery_protected,
        is_liveliness_protected: raw.is_liveliness_protected,
        is_key_revision_enabled: raw.is_key_revision_enabled,
        plugin_participant_attributes: raw.plugin_participant_attributes,
        ac_participant_properties: properties_of(
            raw.ac_participant_properties,
            raw.n_ac_participant_properties,
        )?,
        algorithm_info: ParticipantSecurityAlgorithmInfo {
            digital_signature: DigitalSignatureAlgorithmInfo {
                trust_chain: masks_of(a.trust_chain),
                message_auth: masks_of(a.message_auth),
            },
            key_establishment: KeyEstablishmentAlgorithmInfo {
                shared_secret: masks_of(a.shared_secret),
            },
            symmetric_cipher: SymmetricCipherAlgorithmInfo {
                supported_mask: a.cipher_supported_mask,
                builtin_endpoints_required_mask: a.cipher_builtin_endpoints_required_mask,
                builtin_kx_endpoints_required_mask: a.cipher_builtin_kx_endpoints_required_mask,
                user_endpoints_default_required_mask: a.cipher_user_endpoints_default_required_mask,
            },
        },
    })
}

fn topic_config_of(raw: ZeroDdsPluginTopicConfig) -> TopicSecurityConfig {
    TopicSecurityConfig {
        is_read_protected: raw.is_read_protected,
        is_write_protected: raw.is_write_protected,
        is_discovery_protected: raw.is_discovery_protected,
        is_liveliness_protected: raw.is_liveliness_protected,
    }
}

fn topic_config_to(config: TopicSecurityConfig) -> ZeroDdsPluginTopicConfig {
    ZeroDdsPluginTopicConfig {
        is_read_protected: config.is_read_protected,
        is_write_protected: config.is_write_protected,
        is_discovery_protected: config.is_discovery_protected,
        is_liveliness_protected: config.is_liveliness_protected,
    }
}

fn endpoint_config_of(raw: &ZeroDdsPluginEndpointConfig) -> SecurityResult<EndpointSecurityConfig> {
    Ok(EndpointSecurityConfig {
        topic: topic_config_of(raw.topic),
        is_submessage_protected: raw.is_submessage_protected,
        is_payload_protected: raw.is_payload_protected,
        is_key_protected: raw.is_key_protected,
        plugin_endpoint_attributes: raw.plugin_endpoint_attributes,
        ac_endpoint_properties: properties_of(
            raw.ac_endpoint_properties,
            raw.n_ac_endpoint_properties,
        )?,
        algorithm_info: EndpointSecurityAlgorithmInfo {
            symmetric_cipher: EndpointSymmetricCipherAlgorithmInfo {
                required_mask: raw.cipher_required_mask,
                supported_mask: raw.cipher_supported_mask,
            },
        },
    })
}

// ============================================================================
// A plugin of the application behind a lock
// ============================================================================

/// A vtable and the lock that serializes its calls; `destroy` runs when the
/// last owner lets go. `keep` holds what the plugin was given a pointer to
/// (a listener) and frees it after `destroy`.
struct PluginCell<T: Destroy> {
    table: Mutex<T>,
    keep: Mutex<Option<Box<dyn core::any::Any + Send>>>,
}

/// What `Cell` does when it goes away.
trait Destroy: Copy {
    /// Calls the `destroy` of the plugin.
    fn destroy(&self);
}

// SAFETY: the table holds raw pointers to the plugin's own state; every call
// goes through the mutex, so the plugin is never entered twice at once, and
// the header requires a plugin to be callable from the thread that holds the
// lock, whichever it is.
unsafe impl<T: Destroy> Send for PluginCell<T> {}
// SAFETY: see above.
unsafe impl<T: Destroy> Sync for PluginCell<T> {}

impl<T: Destroy> PluginCell<T> {
    fn new(table: T) -> Self {
        Self {
            table: Mutex::new(table),
            keep: Mutex::new(None),
        }
    }

    fn lock(&self) -> MutexGuard<'_, T> {
        self.table
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl<T: Destroy> Drop for PluginCell<T> {
    fn drop(&mut self) {
        self.lock().destroy();
    }
}

macro_rules! destroy_with {
    ($t:ty) => {
        impl Destroy for $t {
            fn destroy(&self) {
                if let Some(destroy) = self.destroy {
                    // SAFETY: the plugin's own function, called once with its
                    // own `user`.
                    unsafe { destroy(self.user) };
                }
            }
        }
    };
}
destroy_with!(ZeroDdsPluginAccessControl);
destroy_with!(ZeroDdsPluginLogging);
destroy_with!(ZeroDdsPluginDataTagging);

/// The function `f` of a table, or the error that names it.
macro_rules! op {
    ($table:expr, $name:ident) => {
        $table.$name.ok_or_else(|| missing(stringify!($name)))?
    };
}

// ============================================================================
// AccessControl
// ============================================================================

/// What the plugin handed out and the host has not given back.
#[derive(Default)]
struct Issued {
    permissions_tokens: Vec<(DataHolder, ZeroDdsPluginDataHolder)>,
    credential_tokens: Vec<(DataHolder, ZeroDdsPluginDataHolder)>,
    participant_configs: Vec<(ParticipantSecurityConfig, ZeroDdsPluginParticipantConfig)>,
    writer_configs: Vec<(EndpointSecurityConfig, ZeroDdsPluginEndpointConfig)>,
    reader_configs: Vec<(EndpointSecurityConfig, ZeroDdsPluginEndpointConfig)>,
}

/// The listener the host gave and the table the plugin was given a pointer
/// to; both live as long as the plugin.
struct ListenerState {
    _listener: Box<Arc<dyn AccessControlListener>>,
    table: Box<ZeroDdsPluginAccessControlListener>,
}

// SAFETY: the table holds a pointer to the listener next to it, which is
// `Send + Sync`; nothing else is shared.
unsafe impl Send for ListenerState {}

/// The function the AccessControl plugin calls to tell the runtime that it
/// revoked permissions.
///
/// # Safety
/// `context` is the pointer the runtime gave in `set_listener`; `plugin` is
/// NULL or NUL-terminated.
unsafe extern "C" fn on_revoke_trampoline(
    context: *mut c_void,
    plugin: *const c_char,
    handle: u64,
) -> bool {
    if context.is_null() {
        return false;
    }
    // SAFETY: `context` is the pointer `set_listener` made from a live
    // `Box<Arc<dyn AccessControlListener>>`; it stays valid until the plugin
    // is destroyed.
    let listener = unsafe { &*context.cast::<Arc<dyn AccessControlListener>>() };
    let name = if plugin.is_null() {
        ""
    } else {
        // SAFETY: NUL-terminated by the plugin's contract.
        unsafe { CStr::from_ptr(plugin) }.to_str().unwrap_or("")
    };
    listener.on_revoke_permissions(name, PermissionsHandle(handle))
}

/// The AccessControl plugin of an application in C, as the Rust plugin.
pub struct CAccessControl {
    cell: Arc<PluginCell<ZeroDdsPluginAccessControl>>,
    class_id: String,
    issued: Issued,
}

// SAFETY: `issued` holds structs that point into the plugin's own memory;
// it is read and written only through `&mut self`, and every call into the
// plugin goes through the lock of the cell.
unsafe impl Send for CAccessControl {}
// SAFETY: see above; the `&self` operations touch the cell only.
unsafe impl Sync for CAccessControl {}

impl core::fmt::Debug for CAccessControl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CAccessControl")
            .field("class_id", &self.class_id)
            .finish_non_exhaustive()
    }
}

/// A plugin table ready to be installed more than once.
#[derive(Clone)]
pub struct AccessControlHandle(Arc<PluginCell<ZeroDdsPluginAccessControl>>);

impl core::fmt::Debug for AccessControlHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("AccessControlHandle")
    }
}

const ACCESS_CONTROL_OPERATIONS: &[(&str, Presence)] = &[
    ("plugin_class_id", |t| t.plugin_class_id.is_some()),
    ("validate_local_permissions", |t| {
        t.validate_local_permissions.is_some()
    }),
    ("validate_remote_permissions", |t| {
        t.validate_remote_permissions.is_some()
    }),
    ("check_create_participant", |t| {
        t.check_create_participant.is_some()
    }),
    ("check_create_datawriter", |t| {
        t.check_create_datawriter.is_some()
    }),
    ("check_create_datareader", |t| {
        t.check_create_datareader.is_some()
    }),
    ("check_create_topic", |t| t.check_create_topic.is_some()),
    ("check_local_datawriter_register_instance", |t| {
        t.check_local_datawriter_register_instance.is_some()
    }),
    ("check_local_datawriter_dispose_instance", |t| {
        t.check_local_datawriter_dispose_instance.is_some()
    }),
    ("check_remote_participant", |t| {
        t.check_remote_participant.is_some()
    }),
    ("check_remote_datawriter", |t| {
        t.check_remote_datawriter.is_some()
    }),
    ("check_remote_datareader", |t| {
        t.check_remote_datareader.is_some()
    }),
    ("check_remote_topic", |t| t.check_remote_topic.is_some()),
    ("check_local_datawriter_match", |t| {
        t.check_local_datawriter_match.is_some()
    }),
    ("check_local_datareader_match", |t| {
        t.check_local_datareader_match.is_some()
    }),
    ("check_remote_datawriter_register_instance", |t| {
        t.check_remote_datawriter_register_instance.is_some()
    }),
    ("check_remote_datawriter_dispose_instance", |t| {
        t.check_remote_datawriter_dispose_instance.is_some()
    }),
    ("check_relay", |t| t.check_relay.is_some()),
    ("get_permissions_token", |t| {
        t.get_permissions_token.is_some()
    }),
    ("get_permissions_credential_token", |t| {
        t.get_permissions_credential_token.is_some()
    }),
    ("set_listener", |t| t.set_listener.is_some()),
    ("return_permissions_token", |t| {
        t.return_permissions_token.is_some()
    }),
    ("return_permissions_credential_token", |t| {
        t.return_permissions_credential_token.is_some()
    }),
    ("get_participant_security_config", |t| {
        t.get_participant_security_config.is_some()
    }),
    ("get_topic_security_config", |t| {
        t.get_topic_security_config.is_some()
    }),
    ("get_datawriter_security_config", |t| {
        t.get_datawriter_security_config.is_some()
    }),
    ("get_datareader_security_config", |t| {
        t.get_datareader_security_config.is_some()
    }),
    ("return_participant_security_config", |t| {
        t.return_participant_security_config.is_some()
    }),
    ("return_topic_security_config", |t| {
        t.return_topic_security_config.is_some()
    }),
    ("return_datawriter_security_config", |t| {
        t.return_datawriter_security_config.is_some()
    }),
    ("return_datareader_security_config", |t| {
        t.return_datareader_security_config.is_some()
    }),
    ("destroy", |t| t.destroy.is_some()),
];

impl AccessControlHandle {
    /// Takes a copy of the plugin table; the runtime refuses a table with
    /// another `abi_version` or a function missing.
    ///
    /// # Errors
    /// The `abi_version` is not [`ZERODDS_PLUGIN_ABI_VERSION`] or an operation
    /// is NULL (the name is in the message).
    pub fn new(table: ZeroDdsPluginAccessControl) -> Result<Self, String> {
        if table.abi_version != ZERODDS_PLUGIN_ABI_VERSION {
            return Err(format!(
                "the AccessControl plugin has abi_version {}, the runtime {}",
                table.abi_version, ZERODDS_PLUGIN_ABI_VERSION
            ));
        }
        for (name, present) in ACCESS_CONTROL_OPERATIONS {
            if !present(&table) {
                return Err(format!("the AccessControl plugin has no {name}"));
            }
        }
        Ok(Self(Arc::new(PluginCell::new(table))))
    }

    /// A plugin for one runtime.
    ///
    /// # Errors
    /// The plugin gives a class id that is not UTF-8.
    pub fn instantiate(&self) -> SecurityResult<CAccessControl> {
        let class_id = {
            let table = self.0.lock();
            let class_id = op!(table, plugin_class_id);
            // SAFETY: the plugin's own function.
            let text = unsafe { class_id(table.user) };
            string_of(text)?
        };
        Ok(CAccessControl {
            cell: Arc::clone(&self.0),
            class_id,
            issued: Issued::default(),
        })
    }
}

impl CAccessControl {
    /// Calls an operation of the plugin with the lock held; a status other
    /// than 0 is the error of the plugin.
    fn call(
        &self,
        f: impl FnOnce(&ZeroDdsPluginAccessControl, &mut ZeroDdsPluginError) -> SecurityResult<Status>,
    ) -> SecurityResult<()> {
        let table = self.cell.lock();
        let mut err = blank_error();
        let status = f(&table, &mut err)?;
        check(status, &err)
    }

    fn decide_endpoint(
        &self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
        pick: fn(&ZeroDdsPluginAccessControl) -> Option<EndpointCheckFn>,
        name: &'static str,
    ) -> SecurityResult<AccessDecision> {
        let c = CEndpointRequest::new(request)?;
        let mut permit = false;
        self.call(|t, err| {
            let f = pick(t).ok_or_else(|| missing(name))?;
            // SAFETY: the plugin's function with valid arguments.
            Ok(unsafe { f(t.user, permissions.0, &c.raw, &mut permit, err) })
        })?;
        Ok(AccessDecision::from(permit))
    }

    fn decide_participant(
        &self,
        permissions: PermissionsHandle,
        participant: &ParticipantRequest<'_>,
        pick: fn(&ZeroDdsPluginAccessControl) -> Option<ParticipantCheckFn>,
        name: &'static str,
    ) -> SecurityResult<AccessDecision> {
        let c = CParticipantRequest::new(participant)?;
        let mut permit = false;
        self.call(|t, err| {
            let f = pick(t).ok_or_else(|| missing(name))?;
            // SAFETY: the plugin's function with valid arguments.
            Ok(unsafe { f(t.user, permissions.0, &c.raw, &mut permit, err) })
        })?;
        Ok(AccessDecision::from(permit))
    }

    fn decide_topic(
        &self,
        permissions: PermissionsHandle,
        request: &TopicRequest<'_>,
        pick: fn(&ZeroDdsPluginAccessControl) -> Option<TopicCheckFn>,
        name: &'static str,
    ) -> SecurityResult<AccessDecision> {
        let c = CTopicRequest::new(request)?;
        let mut permit = false;
        self.call(|t, err| {
            let f = pick(t).ok_or_else(|| missing(name))?;
            // SAFETY: the plugin's function with valid arguments.
            Ok(unsafe { f(t.user, permissions.0, &c.raw, &mut permit, err) })
        })?;
        Ok(AccessDecision::from(permit))
    }

    fn decide_instance(
        &self,
        permissions: PermissionsHandle,
        writer: [u8; 16],
        key: &[u8],
        pick: fn(&ZeroDdsPluginAccessControl) -> Option<InstanceCheckFn>,
        name: &'static str,
    ) -> SecurityResult<AccessDecision> {
        let mut permit = false;
        self.call(|t, err| {
            let f = pick(t).ok_or_else(|| missing(name))?;
            // SAFETY: the plugin's function with valid arguments.
            Ok(unsafe {
                f(
                    t.user,
                    permissions.0,
                    writer.as_ptr(),
                    key.as_ptr(),
                    key.len(),
                    &mut permit,
                    err,
                )
            })
        })?;
        Ok(AccessDecision::from(permit))
    }

    fn decide_remote_instance(
        &self,
        permissions: PermissionsHandle,
        reader: [u8; 16],
        publication: [u8; 16],
        key: &[u8],
        pick: fn(&ZeroDdsPluginAccessControl) -> Option<RemoteInstanceCheckFn>,
        name: &'static str,
    ) -> SecurityResult<AccessDecision> {
        let mut permit = false;
        self.call(|t, err| {
            let f = pick(t).ok_or_else(|| missing(name))?;
            // SAFETY: the plugin's function with valid arguments.
            Ok(unsafe {
                f(
                    t.user,
                    permissions.0,
                    reader.as_ptr(),
                    publication.as_ptr(),
                    key.as_ptr(),
                    key.len(),
                    &mut permit,
                    err,
                )
            })
        })?;
        Ok(AccessDecision::from(permit))
    }

    fn decide_match(
        &self,
        writer_permissions: PermissionsHandle,
        reader_permissions: PermissionsHandle,
        first: &EndpointRequest<'_>,
        second: &EndpointRequest<'_>,
        pick: fn(&ZeroDdsPluginAccessControl) -> Option<MatchCheckFn>,
        name: &'static str,
    ) -> SecurityResult<AccessDecision> {
        let a = CEndpointRequest::new(first)?;
        let b = CEndpointRequest::new(second)?;
        let mut permit = false;
        self.call(|t, err| {
            let f = pick(t).ok_or_else(|| missing(name))?;
            // SAFETY: the plugin's function with valid arguments.
            Ok(unsafe {
                f(
                    t.user,
                    writer_permissions.0,
                    reader_permissions.0,
                    &a.raw,
                    &b.raw,
                    &mut permit,
                    err,
                )
            })
        })?;
        Ok(AccessDecision::from(permit))
    }

    fn token(&mut self, handle: PermissionsHandle, credential: bool) -> SecurityResult<DataHolder> {
        let mut raw = ZeroDdsPluginDataHolder {
            class_id: core::ptr::null(),
            properties: core::ptr::null(),
            n_properties: 0,
            binary_properties: core::ptr::null(),
            n_binary_properties: 0,
        };
        {
            let table = self.cell.lock();
            let f = if credential {
                op!(table, get_permissions_credential_token)
            } else {
                op!(table, get_permissions_token)
            };
            let mut err = blank_error();
            // SAFETY: the plugin's function with valid arguments.
            let status = unsafe { f(table.user, handle.0, &mut raw, &mut err) };
            check(status, &err)?;
        }
        let holder = holder_of(&raw)?;
        let issued = if credential {
            &mut self.issued.credential_tokens
        } else {
            &mut self.issued.permissions_tokens
        };
        issued.push((holder.clone(), raw));
        Ok(holder)
    }

    fn give_back_token(&mut self, token: &DataHolder, credential: bool) -> SecurityResult<()> {
        let list = if credential {
            &mut self.issued.credential_tokens
        } else {
            &mut self.issued.permissions_tokens
        };
        let at = list
            .iter()
            .position(|(holder, _)| holder == token)
            .ok_or_else(|| bad("the token was not issued by this plugin"))?;
        let (_, raw) = list.remove(at);
        let table = self.cell.lock();
        let f = if credential {
            op!(table, return_permissions_credential_token)
        } else {
            op!(table, return_permissions_token)
        };
        let mut err = blank_error();
        // SAFETY: the plugin's function with the struct it gave.
        let status = unsafe { f(table.user, &raw, &mut err) };
        check(status, &err)
    }

    fn endpoint_config(
        &mut self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
        writer: bool,
    ) -> SecurityResult<EndpointSecurityConfig> {
        let c = CEndpointRequest::new(request)?;
        let mut raw = ZeroDdsPluginEndpointConfig {
            topic: ZeroDdsPluginTopicConfig::default(),
            is_submessage_protected: false,
            is_payload_protected: false,
            is_key_protected: false,
            plugin_endpoint_attributes: 0,
            ac_endpoint_properties: core::ptr::null(),
            n_ac_endpoint_properties: 0,
            cipher_required_mask: 0,
            cipher_supported_mask: 0,
        };
        {
            let table = self.cell.lock();
            let f = if writer {
                op!(table, get_datawriter_security_config)
            } else {
                op!(table, get_datareader_security_config)
            };
            let mut err = blank_error();
            // SAFETY: the plugin's function with valid arguments.
            let status = unsafe { f(table.user, permissions.0, &c.raw, &mut raw, &mut err) };
            check(status, &err)?;
        }
        let config = endpoint_config_of(&raw)?;
        if writer {
            self.issued.writer_configs.push((config.clone(), raw));
        } else {
            self.issued.reader_configs.push((config.clone(), raw));
        }
        Ok(config)
    }

    fn give_back_endpoint_config(
        &mut self,
        config: &EndpointSecurityConfig,
        writer: bool,
    ) -> SecurityResult<()> {
        let list = if writer {
            &mut self.issued.writer_configs
        } else {
            &mut self.issued.reader_configs
        };
        let at = list
            .iter()
            .position(|(issued, _)| issued == config)
            .ok_or_else(|| bad("the config was not issued by this plugin"))?;
        let (_, raw) = list.remove(at);
        let table = self.cell.lock();
        let f = if writer {
            op!(table, return_datawriter_security_config)
        } else {
            op!(table, return_datareader_security_config)
        };
        let mut err = blank_error();
        // SAFETY: the plugin's function with the struct it gave.
        let status = unsafe { f(table.user, &raw, &mut err) };
        check(status, &err)
    }
}

impl AccessControlPlugin for CAccessControl {
    fn validate_local_permissions(
        &mut self,
        identity: IdentityHandle,
        participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<PermissionsHandle> {
        let c = CParticipantRequest::new(participant)?;
        let mut handle = 0u64;
        self.call(|t, err| {
            let f = op!(t, validate_local_permissions);
            // SAFETY: the plugin's function with valid arguments.
            Ok(unsafe { f(t.user, identity.0, &c.raw, &mut handle, err) })
        })?;
        Ok(PermissionsHandle(handle))
    }

    fn validate_remote_permissions(
        &mut self,
        local_identity: IdentityHandle,
        remote_identity: IdentityHandle,
        remote_permissions_token: &zerodds_security::token::PermissionsToken,
        remote_credential_token: &DataHolder,
    ) -> SecurityResult<PermissionsHandle> {
        let permissions = CDataHolder::new(remote_permissions_token)?;
        let credential = CDataHolder::new(remote_credential_token)?;
        let mut handle = 0u64;
        self.call(|t, err| {
            let f = op!(t, validate_remote_permissions);
            // SAFETY: the plugin's function with valid arguments.
            Ok(unsafe {
                f(
                    t.user,
                    local_identity.0,
                    remote_identity.0,
                    &permissions.raw,
                    &credential.raw,
                    &mut handle,
                    err,
                )
            })
        })?;
        Ok(PermissionsHandle(handle))
    }

    fn check_create_participant(
        &self,
        permissions: PermissionsHandle,
        participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decide_participant(
            permissions,
            participant,
            |t| t.check_create_participant,
            "check_create_participant",
        )
    }

    fn check_create_datawriter(
        &self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decide_endpoint(
            permissions,
            request,
            |t| t.check_create_datawriter,
            "check_create_datawriter",
        )
    }

    fn check_create_datareader(
        &self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decide_endpoint(
            permissions,
            request,
            |t| t.check_create_datareader,
            "check_create_datareader",
        )
    }

    fn check_create_topic(
        &self,
        permissions: PermissionsHandle,
        request: &TopicRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decide_topic(
            permissions,
            request,
            |t| t.check_create_topic,
            "check_create_topic",
        )
    }

    fn check_local_datawriter_register_instance(
        &self,
        permissions: PermissionsHandle,
        writer: [u8; 16],
        key: &[u8],
    ) -> SecurityResult<AccessDecision> {
        self.decide_instance(
            permissions,
            writer,
            key,
            |t| t.check_local_datawriter_register_instance,
            "check_local_datawriter_register_instance",
        )
    }

    fn check_local_datawriter_dispose_instance(
        &self,
        permissions: PermissionsHandle,
        writer: [u8; 16],
        key: &[u8],
    ) -> SecurityResult<AccessDecision> {
        self.decide_instance(
            permissions,
            writer,
            key,
            |t| t.check_local_datawriter_dispose_instance,
            "check_local_datawriter_dispose_instance",
        )
    }

    fn check_remote_participant(
        &self,
        permissions: PermissionsHandle,
        participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decide_participant(
            permissions,
            participant,
            |t| t.check_remote_participant,
            "check_remote_participant",
        )
    }

    fn check_remote_datawriter(
        &self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decide_endpoint(
            permissions,
            request,
            |t| t.check_remote_datawriter,
            "check_remote_datawriter",
        )
    }

    fn check_remote_datareader(
        &self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<RemoteReaderDecision> {
        let c = CEndpointRequest::new(request)?;
        let mut decision = 0i32;
        self.call(|t, err| {
            let f = op!(t, check_remote_datareader);
            // SAFETY: the plugin's function with valid arguments.
            Ok(unsafe { f(t.user, permissions.0, &c.raw, &mut decision, err) })
        })?;
        let raw = decision;
        match raw {
            0 => Ok(RemoteReaderDecision::Deny),
            1 => Ok(RemoteReaderDecision::Permit),
            2 => Ok(RemoteReaderDecision::RelayOnly),
            _ => Err(bad(
                "check_remote_datareader of the plugin gave a decision that is none of deny, permit, relay-only",
            )),
        }
    }

    fn check_remote_topic(
        &self,
        permissions: PermissionsHandle,
        request: &TopicRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decide_topic(
            permissions,
            request,
            |t| t.check_remote_topic,
            "check_remote_topic",
        )
    }

    fn check_local_datawriter_match(
        &self,
        writer_permissions: PermissionsHandle,
        reader_permissions: PermissionsHandle,
        publication: &EndpointRequest<'_>,
        subscription: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decide_match(
            writer_permissions,
            reader_permissions,
            publication,
            subscription,
            |t| t.check_local_datawriter_match,
            "check_local_datawriter_match",
        )
    }

    fn check_local_datareader_match(
        &self,
        writer_permissions: PermissionsHandle,
        reader_permissions: PermissionsHandle,
        subscription: &EndpointRequest<'_>,
        publication: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decide_match(
            writer_permissions,
            reader_permissions,
            subscription,
            publication,
            |t| t.check_local_datareader_match,
            "check_local_datareader_match",
        )
    }

    fn check_remote_datawriter_register_instance(
        &self,
        permissions: PermissionsHandle,
        reader: [u8; 16],
        publication: [u8; 16],
        key: &[u8],
    ) -> SecurityResult<AccessDecision> {
        self.decide_remote_instance(
            permissions,
            reader,
            publication,
            key,
            |t| t.check_remote_datawriter_register_instance,
            "check_remote_datawriter_register_instance",
        )
    }

    fn check_remote_datawriter_dispose_instance(
        &self,
        permissions: PermissionsHandle,
        reader: [u8; 16],
        publication: [u8; 16],
        key: &[u8],
    ) -> SecurityResult<AccessDecision> {
        self.decide_remote_instance(
            permissions,
            reader,
            publication,
            key,
            |t| t.check_remote_datawriter_dispose_instance,
            "check_remote_datawriter_dispose_instance",
        )
    }

    fn get_permissions_token(
        &mut self,
        handle: PermissionsHandle,
    ) -> SecurityResult<zerodds_security::token::PermissionsToken> {
        self.token(handle, false)
    }

    fn get_permissions_credential_token(
        &mut self,
        handle: PermissionsHandle,
    ) -> SecurityResult<zerodds_security::token::PermissionsCredentialToken> {
        self.token(handle, true)
    }

    fn set_listener(&mut self, listener: Arc<dyn AccessControlListener>) -> SecurityResult<()> {
        let mut listener = Box::new(listener);
        let table = Box::new(ZeroDdsPluginAccessControlListener {
            context: (&raw mut *listener).cast(),
            on_revoke_permissions: Some(on_revoke_trampoline),
        });
        let table_ptr: *const ZeroDdsPluginAccessControlListener = &*table;
        let state = ListenerState {
            _listener: listener,
            table,
        };
        let table = self.cell.lock();
        let f = op!(table, set_listener);
        let mut err = blank_error();
        // SAFETY: the plugin's function; the listener table lives in the cell
        // as long as the plugin.
        let status = unsafe { f(table.user, table_ptr, &mut err) };
        check(status, &err)?;
        // The earlier state, if any, is dropped when the plugin has the new
        // table (the call above returned).
        let _ = &state.table;
        *self
            .cell
            .keep
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Box::new(state));
        Ok(())
    }

    fn return_permissions_token(
        &mut self,
        token: zerodds_security::token::PermissionsToken,
    ) -> SecurityResult<()> {
        self.give_back_token(&token, false)
    }

    fn return_permissions_credential_token(
        &mut self,
        token: zerodds_security::token::PermissionsCredentialToken,
    ) -> SecurityResult<()> {
        self.give_back_token(&token, true)
    }

    fn get_participant_security_config(
        &mut self,
        permissions: PermissionsHandle,
    ) -> SecurityResult<ParticipantSecurityConfig> {
        let mut raw = ZeroDdsPluginParticipantConfig {
            allow_unauthenticated_participants: false,
            is_access_protected: false,
            is_rtps_axk_protected: false,
            is_rtps_psk_protected: false,
            is_discovery_protected: false,
            is_liveliness_protected: false,
            is_key_revision_enabled: false,
            plugin_participant_attributes: 0,
            ac_participant_properties: core::ptr::null(),
            n_ac_participant_properties: 0,
            algorithm_info: ZeroDdsPluginParticipantAlgorithmInfo::default(),
        };
        {
            let table = self.cell.lock();
            let f = op!(table, get_participant_security_config);
            let mut err = blank_error();
            // SAFETY: the plugin's function with valid arguments.
            let status = unsafe { f(table.user, permissions.0, &mut raw, &mut err) };
            check(status, &err)?;
        }
        let config = participant_config_of(&raw)?;
        self.issued.participant_configs.push((config.clone(), raw));
        Ok(config)
    }

    fn get_topic_security_config(
        &mut self,
        permissions: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<TopicSecurityConfig> {
        let topic = cstring(topic_name)?;
        let mut raw = ZeroDdsPluginTopicConfig::default();
        self.call(|t, err| {
            let f = op!(t, get_topic_security_config);
            // SAFETY: the plugin's function with valid arguments.
            Ok(unsafe { f(t.user, permissions.0, topic.as_ptr(), &mut raw, err) })
        })?;
        Ok(topic_config_of(raw))
    }

    fn get_datawriter_security_config(
        &mut self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<EndpointSecurityConfig> {
        self.endpoint_config(permissions, request, true)
    }

    fn get_datareader_security_config(
        &mut self,
        permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<EndpointSecurityConfig> {
        self.endpoint_config(permissions, request, false)
    }

    fn return_participant_security_config(
        &mut self,
        config: ParticipantSecurityConfig,
    ) -> SecurityResult<()> {
        let at = self
            .issued
            .participant_configs
            .iter()
            .position(|(issued, _)| *issued == config)
            .ok_or_else(|| bad("the config was not issued by this plugin"))?;
        let (_, raw) = self.issued.participant_configs.remove(at);
        let table = self.cell.lock();
        let f = op!(table, return_participant_security_config);
        let mut err = blank_error();
        // SAFETY: the plugin's function with the struct it gave.
        let status = unsafe { f(table.user, &raw, &mut err) };
        check(status, &err)
    }

    fn return_topic_security_config(&mut self, config: TopicSecurityConfig) -> SecurityResult<()> {
        let raw = topic_config_to(config);
        let table = self.cell.lock();
        let f = op!(table, return_topic_security_config);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe { f(table.user, &raw, &mut err) };
        check(status, &err)
    }

    fn return_datawriter_security_config(
        &mut self,
        config: EndpointSecurityConfig,
    ) -> SecurityResult<()> {
        self.give_back_endpoint_config(&config, true)
    }

    fn return_datareader_security_config(
        &mut self,
        config: EndpointSecurityConfig,
    ) -> SecurityResult<()> {
        self.give_back_endpoint_config(&config, false)
    }

    fn check_relay(
        &self,
        permissions: PermissionsHandle,
        role: RelayRole,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        let c = CEndpointRequest::new(request)?;
        let role = match role {
            RelayRole::Reader => 0,
            RelayRole::Writer => 1,
        };
        let mut permit = false;
        self.call(|t, err| {
            let f = op!(t, check_relay);
            // SAFETY: the plugin's function with valid arguments.
            Ok(unsafe { f(t.user, permissions.0, role, &c.raw, &mut permit, err) })
        })?;
        Ok(AccessDecision::from(permit))
    }

    fn plugin_class_id(&self) -> &str {
        &self.class_id
    }
}

// ============================================================================
// Logging
// ============================================================================

/// The listener the host gave a Logging plugin.
struct LoggerContext {
    listener: Arc<dyn LoggerListener>,
}

/// The function the Logging plugin calls for each message it logs.
///
/// # Safety
/// `context` is the pointer the runtime gave in `set_listener`; `participant`
/// is NULL or 16 octets; `category` and `message` are NULL or NUL-terminated.
unsafe extern "C" fn log_message_trampoline(
    context: *mut c_void,
    level: i32,
    participant: *const u8,
    category: *const c_char,
    message: *const c_char,
) -> i32 {
    if context.is_null() {
        return -1;
    }
    // SAFETY: `context` is the pointer `set_listener` made from a live
    // `Box<LoggerContext>`.
    let context = unsafe { &*context.cast::<LoggerContext>() };
    let Some(level) = LogLevel::from_spec_level(level) else {
        return -1;
    };
    let mut guid = [0u8; 16];
    if !participant.is_null() {
        // SAFETY: the plugin passes 16 octets.
        guid.copy_from_slice(unsafe { core::slice::from_raw_parts(participant, 16) });
    }
    let text = |p: *const c_char| {
        if p.is_null() {
            String::new()
        } else {
            // SAFETY: NUL-terminated by the plugin's contract.
            unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
        }
    };
    context
        .listener
        .log_message(level, guid, &text(category), &text(message))
}

/// The Logging plugin of an application in C, as the Rust plugin.
pub struct CLogging {
    cell: Arc<PluginCell<ZeroDdsPluginLogging>>,
    class_id: String,
    listener: Mutex<Option<(Box<LoggerContext>, Box<ZeroDdsPluginLoggerListener>)>>,
}

// SAFETY: the listener table holds a pointer to the listener next to it,
// which is `Send + Sync`; every call into the plugin goes through the lock of
// the cell.
unsafe impl Send for CLogging {}
// SAFETY: see above.
unsafe impl Sync for CLogging {}

impl core::fmt::Debug for CLogging {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CLogging")
            .field("class_id", &self.class_id)
            .finish_non_exhaustive()
    }
}

impl CLogging {
    /// Takes a copy of the plugin table.
    ///
    /// # Errors
    /// The `abi_version` is not [`ZERODDS_PLUGIN_ABI_VERSION`] or an operation
    /// is NULL.
    pub fn new(table: ZeroDdsPluginLogging) -> Result<Self, String> {
        if table.abi_version != ZERODDS_PLUGIN_ABI_VERSION {
            return Err(format!(
                "the Logging plugin has abi_version {}, the runtime {}",
                table.abi_version, ZERODDS_PLUGIN_ABI_VERSION
            ));
        }
        let present = [
            ("plugin_class_id", table.plugin_class_id.is_some()),
            ("set_log_options", table.set_log_options.is_some()),
            ("enable_logging", table.enable_logging.is_some()),
            ("set_listener", table.set_listener.is_some()),
            ("log", table.log.is_some()),
            ("destroy", table.destroy.is_some()),
        ];
        if let Some((name, _)) = present.iter().find(|(_, p)| !p) {
            return Err(format!("the Logging plugin has no {name}"));
        }
        let class_id = {
            let f = table.plugin_class_id.ok_or("plugin_class_id")?;
            // SAFETY: the plugin's own function.
            let text = unsafe { f(table.user) };
            string_of(text).map_err(|e| e.detail.into_owned())?
        };
        Ok(Self {
            cell: Arc::new(PluginCell::new(table)),
            class_id,
            listener: Mutex::new(None),
        })
    }
}

fn level_to_spec(level: LogLevel) -> i32 {
    level.to_spec_level()
}

impl LoggingPlugin for CLogging {
    fn set_log_options(&self, options: LogOptions) -> Result<(), SecurityError> {
        let file = options.log_file.as_deref().map(cstring).transpose()?;
        let raw = ZeroDdsPluginLogOptions {
            log_level: level_to_spec(options.log_level),
            log_file: file.as_ref().map_or(core::ptr::null(), |f| f.as_ptr()),
            distribute: options.distribute,
        };
        let table = self.cell.lock();
        let f = op!(table, set_log_options);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe { f(table.user, &raw, &mut err) };
        check(status, &err)
    }

    fn enable_logging(&self) -> Result<(), SecurityError> {
        let table = self.cell.lock();
        let f = op!(table, enable_logging);
        let mut err = blank_error();
        // SAFETY: the plugin's function with valid arguments.
        let status = unsafe { f(table.user, &mut err) };
        check(status, &err)
    }

    fn set_listener(&self, listener: Option<Arc<dyn LoggerListener>>) -> Result<(), SecurityError> {
        let mut slot = self
            .listener
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let new = listener.map(|listener| {
            let mut context = Box::new(LoggerContext { listener });
            let table = Box::new(ZeroDdsPluginLoggerListener {
                context: (&raw mut *context).cast(),
                log_message: Some(log_message_trampoline),
            });
            (context, table)
        });
        let table = self.cell.lock();
        let f = op!(table, set_listener);
        let mut err = blank_error();
        let pointer = new
            .as_ref()
            .map_or(core::ptr::null(), |(_, t)| &raw const **t);
        // SAFETY: the plugin's function; the listener table is kept in `slot`
        // until the next `set_listener` or the end of the adapter.
        let status = unsafe { f(table.user, pointer, &mut err) };
        check(status, &err)?;
        *slot = new;
        Ok(())
    }

    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str) {
        let (Ok(category), Ok(message)) = (cstring(category), cstring(message)) else {
            return;
        };
        let table = self.cell.lock();
        if let Some(f) = table.log {
            // SAFETY: the plugin's function with valid arguments.
            unsafe {
                f(
                    table.user,
                    level_to_spec(level),
                    participant.as_ptr(),
                    category.as_ptr(),
                    message.as_ptr(),
                );
            }
        }
    }

    fn plugin_class_id(&self) -> &str {
        &self.class_id
    }
}

// ============================================================================
// DataTagging
// ============================================================================

/// The DataTagging plugin of an application in C, as the Rust plugin.
pub struct CDataTagging {
    cell: Arc<PluginCell<ZeroDdsPluginDataTagging>>,
    class_id: String,
}

impl core::fmt::Debug for CDataTagging {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CDataTagging")
            .field("class_id", &self.class_id)
            .finish_non_exhaustive()
    }
}

impl CDataTagging {
    /// Takes a copy of the plugin table.
    ///
    /// # Errors
    /// The `abi_version` is not [`ZERODDS_PLUGIN_ABI_VERSION`] or an operation
    /// is NULL.
    pub fn new(table: ZeroDdsPluginDataTagging) -> Result<Self, String> {
        if table.abi_version != ZERODDS_PLUGIN_ABI_VERSION {
            return Err(format!(
                "the DataTagging plugin has abi_version {}, the runtime {}",
                table.abi_version, ZERODDS_PLUGIN_ABI_VERSION
            ));
        }
        let present = [
            ("plugin_class_id", table.plugin_class_id.is_some()),
            ("set_tags", table.set_tags.is_some()),
            ("get_tags", table.get_tags.is_some()),
            ("destroy", table.destroy.is_some()),
        ];
        if let Some((name, _)) = present.iter().find(|(_, p)| !p) {
            return Err(format!("the DataTagging plugin has no {name}"));
        }
        let class_id = {
            let f = table.plugin_class_id.ok_or("plugin_class_id")?;
            // SAFETY: the plugin's own function.
            let text = unsafe { f(table.user) };
            string_of(text).map_err(|e| e.detail.into_owned())?
        };
        Ok(Self {
            cell: Arc::new(PluginCell::new(table)),
            class_id,
        })
    }
}

impl DataTaggingPlugin for CDataTagging {
    fn set_tags(&mut self, endpoint_guid: [u8; 16], tags: Vec<DataTag>) {
        let strings: Vec<(CString, CString)> = tags
            .iter()
            .filter_map(|t| Some((cstring(&t.name).ok()?, cstring(&t.value).ok()?)))
            .collect();
        let raw: Vec<ZeroDdsPluginDataTag> = strings
            .iter()
            .map(|(n, v)| ZeroDdsPluginDataTag {
                name: n.as_ptr(),
                value: v.as_ptr(),
            })
            .collect();
        let table = self.cell.lock();
        if let Some(f) = table.set_tags {
            // SAFETY: the plugin's function with valid arguments.
            unsafe { f(table.user, endpoint_guid.as_ptr(), raw.as_ptr(), raw.len()) };
        }
    }

    fn get_tags(&self, endpoint_guid: [u8; 16]) -> Vec<DataTag> {
        let table = self.cell.lock();
        let Some(f) = table.get_tags else {
            return Vec::new();
        };
        let mut tags: *const ZeroDdsPluginDataTag = core::ptr::null();
        let mut n = 0usize;
        // SAFETY: the plugin's function with valid arguments.
        unsafe { f(table.user, endpoint_guid.as_ptr(), &mut tags, &mut n) };
        slice_of(tags, n)
            .iter()
            .filter_map(|t| {
                Some(DataTag {
                    name: string_of(t.name).ok()?,
                    value: string_of(t.value).ok()?,
                })
            })
            .collect()
    }

    fn plugin_class_id(&self) -> &str {
        &self.class_id
    }
}

mod security_plugins;
pub use security_plugins::*;

// ============================================================================
// What a security configuration holds
// ============================================================================

/// The plugins of an application that a security configuration holds.
#[derive(Debug, Default, Clone)]
pub struct ApplicationPlugins {
    /// The AccessControl plugin.
    pub access_control: Option<AccessControlHandle>,
    /// The Logging plugin.
    pub logging: Option<Arc<CLogging>>,
    /// The DataTagging plugin.
    pub data_tagging: Option<Arc<Mutex<CDataTagging>>>,
    /// The Authentication plugin.
    pub authentication: Option<AuthenticationHandle>,
    /// The Cryptographic plugin.
    pub cryptographic: Option<CryptographicHandle>,
}

/// The table behind `plugin`, copied.
///
/// # Safety
/// `plugin` is NULL or points to a valid `T`.
unsafe fn table_of<T: Copy>(plugin: *const c_void) -> Option<T> {
    if plugin.is_null() {
        return None;
    }
    // SAFETY: per the contract of the function.
    Some(unsafe { *plugin.cast::<T>() })
}

fn report(what: &str, error: &str) -> i32 {
    eprintln!("{what}: {error}");
    -2
}

/// Hands the AccessControl plugin `plugin` (a `ZeroDdsPluginAccessControl`)
/// to the security configuration `cfg`. The runtime copies the struct; its
/// `destroy` runs when the configuration and the runtimes made from it have
/// let go of it. Returns 0, or -1 for a NULL argument, -2 for a plugin the
/// runtime refuses (the reason is on stderr).
///
/// # Safety
/// `cfg` is from `zerodds_security_config_create`; `plugin` points to a
/// valid struct.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_security_set_access_control_plugin(
    cfg: *mut crate::security_ffi::ZeroDdsSecurityConfig,
    plugin: *const c_void,
) -> i32 {
    // SAFETY: per the contract of the function.
    let (Some(cfg), Some(table)) = (unsafe { cfg.as_mut() }, unsafe {
        table_of::<ZeroDdsPluginAccessControl>(plugin)
    }) else {
        return -1;
    };
    match AccessControlHandle::new(table) {
        Ok(handle) => {
            cfg.plugins.access_control = Some(handle);
            0
        }
        Err(e) => report("zerodds_security_set_access_control_plugin", &e),
    }
}

/// Hands the Logging plugin `plugin` (a `ZeroDdsPluginLogging`) to the
/// security configuration `cfg`; see
/// [`zerodds_security_set_access_control_plugin`].
///
/// # Safety
/// `cfg` is from `zerodds_security_config_create`; `plugin` points to a
/// valid struct.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_security_set_logging_plugin(
    cfg: *mut crate::security_ffi::ZeroDdsSecurityConfig,
    plugin: *const c_void,
) -> i32 {
    // SAFETY: per the contract of the function.
    let (Some(cfg), Some(table)) = (unsafe { cfg.as_mut() }, unsafe {
        table_of::<ZeroDdsPluginLogging>(plugin)
    }) else {
        return -1;
    };
    match CLogging::new(table) {
        Ok(logging) => {
            cfg.plugins.logging = Some(Arc::new(logging));
            0
        }
        Err(e) => report("zerodds_security_set_logging_plugin", &e),
    }
}

/// Hands the DataTagging plugin `plugin` (a `ZeroDdsPluginDataTagging`) to
/// the security configuration `cfg`; see
/// [`zerodds_security_set_access_control_plugin`].
///
/// # Safety
/// `cfg` is from `zerodds_security_config_create`; `plugin` points to a
/// valid struct.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_security_set_data_tagging_plugin(
    cfg: *mut crate::security_ffi::ZeroDdsSecurityConfig,
    plugin: *const c_void,
) -> i32 {
    // SAFETY: per the contract of the function.
    let (Some(cfg), Some(table)) = (unsafe { cfg.as_mut() }, unsafe {
        table_of::<ZeroDdsPluginDataTagging>(plugin)
    }) else {
        return -1;
    };
    match CDataTagging::new(table) {
        Ok(tagging) => {
            cfg.plugins.data_tagging = Some(Arc::new(Mutex::new(tagging)));
            0
        }
        Err(e) => report("zerodds_security_set_data_tagging_plugin", &e),
    }
}

/// Wraps the Logging plugin for `RuntimeConfig::security_logger`.
pub(crate) fn logging_for_runtime(logging: &Arc<CLogging>) -> Arc<dyn LoggingPlugin> {
    // The adapter is `LoggingPlugin` itself; the runtime shares it.
    Arc::clone(logging) as Arc<dyn LoggingPlugin>
}

/// Wraps the DataTagging plugin for `RuntimeConfig::security_data_tagging`.
pub(crate) fn data_tagging_for_runtime(
    tagging: &Arc<Mutex<CDataTagging>>,
) -> Arc<Mutex<dyn DataTaggingPlugin>> {
    Arc::clone(tagging) as Arc<Mutex<dyn DataTaggingPlugin>>
}
