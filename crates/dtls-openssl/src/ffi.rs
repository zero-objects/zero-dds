// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The OpenSSL functions openssl-sys 0.9.118 does not bind: raw public keys
//! (RFC 7250, OpenSSL 3.2) and the security callback. The declarations
//! follow `openssl/ssl.h`, `openssl/tls1.h` and `openssl/x509_vfy.h` of
//! OpenSSL 3.5; every use is wrapped in a safe function below.

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_uchar, c_void};

use foreign_types::ForeignTypeRef;
use openssl::pkey::{PKeyRef, Public};
use openssl::ssl::{SslContextBuilder, SslRef};
use openssl::x509::X509StoreContextRef;
use openssl_sys::{EVP_PKEY, SSL, SSL_CIPHER, SSL_CTX, X509_STORE_CTX};

/// `TLSEXT_cert_type_x509` (RFC 7250 §3, `openssl/tls1.h`).
pub const CERT_TYPE_X509: u8 = 0;
/// `TLSEXT_cert_type_rpk`.
pub const CERT_TYPE_RPK: u8 = 2;

const SSL_SECOP_OTHER_TYPE: c_int = 0xffff_0000_u32 as c_int;
const SSL_SECOP_OTHER_CIPHER: c_int = 1 << 16;

type SecurityCallback = unsafe extern "C" fn(
    s: *const SSL,
    ctx: *const SSL_CTX,
    op: c_int,
    bits: c_int,
    nid: c_int,
    other: *mut c_void,
    ex: *mut c_void,
) -> c_int;

unsafe extern "C" {
    fn SSL_CTX_set1_client_cert_type(ctx: *mut SSL_CTX, val: *const c_uchar, len: usize) -> c_int;
    fn SSL_CTX_set1_server_cert_type(ctx: *mut SSL_CTX, val: *const c_uchar, len: usize) -> c_int;
    fn SSL_get0_peer_rpk(s: *const SSL) -> *mut EVP_PKEY;
    fn SSL_get_negotiated_client_cert_type(s: *const SSL) -> c_int;
    fn SSL_get_negotiated_server_cert_type(s: *const SSL) -> c_int;
    fn X509_STORE_CTX_get0_rpk(ctx: *const X509_STORE_CTX) -> *mut EVP_PKEY;
    fn SSL_CTX_set_security_callback(ctx: *mut SSL_CTX, cb: Option<SecurityCallback>);
    fn SSL_CTX_get_security_callback(ctx: *const SSL_CTX) -> Option<SecurityCallback>;
    fn SSL_CIPHER_get_id(c: *const SSL_CIPHER) -> u32;
    fn SSL_CTX_use_psk_identity_hint(ctx: *mut SSL_CTX, identity_hint: *const c_char) -> c_int;
}

/// The PSK identity hint a server sends (RFC 4279 §2).
///
/// # Errors
/// A hint with a NUL byte, or OpenSSL refuses it (longer than 128 octets).
pub fn set_psk_identity_hint(
    ctx: &mut SslContextBuilder,
    hint: &str,
) -> Result<(), openssl::error::ErrorStack> {
    let Ok(hint) = CString::new(hint) else {
        return Err(openssl::error::ErrorStack::get());
    };
    // SAFETY: a live SSL_CTX and a NUL-terminated string OpenSSL copies.
    if unsafe { SSL_CTX_use_psk_identity_hint(ctx.as_ptr(), hint.as_ptr()) } != 1 {
        return Err(openssl::error::ErrorStack::get());
    }
    Ok(())
}

/// Which certificate types a context offers or accepts for the client or
/// the server side (RFC 7250 §3: client_certificate_type and
/// server_certificate_type), most preferred first.
///
/// # Errors
/// OpenSSL refuses the list (an unknown type).
pub fn set_cert_types(
    ctx: &mut SslContextBuilder,
    client: Option<&[u8]>,
    server: Option<&[u8]>,
) -> Result<(), openssl::error::ErrorStack> {
    // SAFETY: `ctx.as_ptr()` is the live SSL_CTX the builder owns; OpenSSL
    // copies `val[..len]` (the `set1` convention) and keeps no pointer to it.
    unsafe {
        if let Some(types) = client
            && SSL_CTX_set1_client_cert_type(ctx.as_ptr(), types.as_ptr(), types.len()) != 1
        {
            return Err(openssl::error::ErrorStack::get());
        }
        if let Some(types) = server
            && SSL_CTX_set1_server_cert_type(ctx.as_ptr(), types.as_ptr(), types.len()) != 1
        {
            return Err(openssl::error::ErrorStack::get());
        }
    }
    Ok(())
}

/// The raw public key the peer presented, as DER SubjectPublicKeyInfo.
#[must_use]
pub fn peer_rpk_spki(ssl: &SslRef) -> Option<Vec<u8>> {
    // SAFETY: `ssl.as_ptr()` is a live SSL; SSL_get0_peer_rpk returns a
    // borrowed EVP_PKEY owned by the session (`get0`), or NULL, and the
    // reference does not outlive `ssl`.
    unsafe {
        let key = SSL_get0_peer_rpk(ssl.as_ptr());
        if key.is_null() {
            return None;
        }
        PKeyRef::<Public>::from_ptr(key).public_key_to_der().ok()
    }
}

/// The negotiated certificate types of the client and the server side
/// (RFC 7250 §4: x509 when the extension was not negotiated).
#[must_use]
pub fn negotiated_cert_types(ssl: &SslRef) -> (u8, u8) {
    // SAFETY: plain getters on a live SSL.
    let (client, server) = unsafe {
        (
            SSL_get_negotiated_client_cert_type(ssl.as_ptr()),
            SSL_get_negotiated_server_cert_type(ssl.as_ptr()),
        )
    };
    (
        u8::try_from(client).unwrap_or(CERT_TYPE_X509),
        u8::try_from(server).unwrap_or(CERT_TYPE_X509),
    )
}

/// In a verify callback: the raw public key being verified, as DER
/// SubjectPublicKeyInfo, when the peer sent one instead of a certificate.
#[must_use]
pub fn store_ctx_rpk_spki(ctx: &X509StoreContextRef) -> Option<Vec<u8>> {
    // SAFETY: `ctx.as_ptr()` is the live X509_STORE_CTX of the callback;
    // the returned EVP_PKEY is borrowed (`get0`) for its duration.
    unsafe {
        let key = X509_STORE_CTX_get0_rpk(ctx.as_ptr());
        if key.is_null() {
            return None;
        }
        PKeyRef::<Public>::from_ptr(key).public_key_to_der().ok()
    }
}

/// The cipher ids (`SSL_CIPHER_get_id`: 0x0300 and the IANA code point) of
/// the AES-128-CCM-8 suites RFC 7252 §9 makes mandatory.
const CCM8_IDS: [u32; 2] = [0x0300_c0a8, 0x0300_c0ae];

static DEFAULT_SECURITY: std::sync::OnceLock<Option<SecurityCallback>> = std::sync::OnceLock::new();

/// OpenSSL 3 rates the CCM-8 suites at 64 bits and so refuses them at every
/// security level above 0. This callback lets the two suites RFC 7252 §9
/// makes mandatory (TLS_PSK_WITH_AES_128_CCM_8, RFC 7252 §9.1.3.1;
/// TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8, §9.1.3.2, §9.1.3.3) pass and leaves
/// every other decision to OpenSSL's own callback at the context's level.
///
/// # Safety
///
/// Called by OpenSSL only, as the security callback of an SSL_CTX, with
/// the arguments of `ssl_security` (ssl/t1_lib.c): for the cipher
/// operations `other` points at an SSL_CIPHER, the other pointers are
/// handed on unread to OpenSSL's default callback.
unsafe extern "C" fn security(
    s: *const SSL,
    ctx: *const SSL_CTX,
    op: c_int,
    bits: c_int,
    nid: c_int,
    other: *mut c_void,
    ex: *mut c_void,
) -> c_int {
    if op & SSL_SECOP_OTHER_TYPE == SSL_SECOP_OTHER_CIPHER && !other.is_null() {
        // SAFETY: for the cipher operations `other` is the SSL_CIPHER
        // (`ssl_security` in ssl/t1_lib.c).
        let id = unsafe { SSL_CIPHER_get_id(other.cast::<SSL_CIPHER>()) };
        if CCM8_IDS.contains(&id) {
            return 1;
        }
    }
    match DEFAULT_SECURITY.get().copied().flatten() {
        // SAFETY: OpenSSL's own callback with the arguments it was called with.
        Some(default) => unsafe { default(s, ctx, op, bits, nid, other, ex) },
        None => 0,
    }
}

/// Installs the security callback of this module on `ctx`, keeping OpenSSL's default callback for
/// everything but the CCM-8 suites.
pub fn allow_ccm8(ctx: &mut SslContextBuilder) {
    // SAFETY: getter and setter on the live SSL_CTX of the builder; the
    // default callback is a static function of libssl, the same for every
    // context, and the new one a `'static` extern "C" fn.
    unsafe {
        let current = SSL_CTX_get_security_callback(ctx.as_ptr());
        let _ = DEFAULT_SECURITY.get_or_init(|| current);
        SSL_CTX_set_security_callback(ctx.as_ptr(), Some(security));
    }
}
