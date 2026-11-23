// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-dtls-openssl`. Safety classification: **STANDARD** (FFI
//! boundary to OpenSSL).
//!
//! DTLS 1.2 (RFC 6347) over UDP on OpenSSL 3.2 or later.
//!
//! RFC 7252 §9 asks a CoAP endpoint for three DTLS modes, and OpenSSL
//! serves all three: pre-shared keys (§9.1.3.1, RFC 4279), raw public keys
//! (§9.1.3.2, RFC 7250, OpenSSL 3.2) checked against names of RFC 6920
//! ([`ni`]), X.509 certificates (§9.1.3.3) checked against trust anchors
//! and the peer's name. The mandatory suites TLS_PSK_WITH_AES_128_CCM_8 and
//! TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8 are offered first; OpenSSL 3 rates
//! CCM-8 below its default security level, a security callback lets these
//! two pass ([`ffi`]).
//!
//! [`transport`] runs DTLS over one UDP socket per server, blocking, a
//! thread per session; [`config`] builds the OpenSSL contexts.
//!
//! The crate links the system's libssl (3.2 or later); the feature
//! `vendored` builds OpenSSL from source instead.

pub mod config;
pub mod ffi;
pub mod ni;
pub mod transport;

pub use config::{CertificateFiles, ClientConfig, ConfigError, ServerConfig};
pub use ni::{KeyName, NameError};
pub use transport::{PeerIdentity, Server, Session, Waker, connect};
