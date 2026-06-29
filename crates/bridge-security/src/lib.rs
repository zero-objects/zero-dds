// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-bridge-security`. Safety classification: **STANDARD**.
//!
//! Shared security layer for ZeroDDS bridge daemons (ws / mqtt / coap /
//! amqp / grpc / corba).
//!
//! Spec: ZeroDDS Bridge Spec 1.0 §7.1 (TLS), §7.2 (auth modes), §7.3
//! (topic ACL).
//!
//! ## Layer position
//!
//! Layer 5 (Bridges) — substrate crate for all six bridge daemons.
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! - [`Acl`], [`AclEntry`], [`AclOp`] — topic ACL with wildcard and
//!   group matching (§7.3).
//! - [`AuthMode`], [`AuthSubject`], [`AuthError`] — auth modes
//!   `none|bearer|jwt|mtls|sasl` (§7.2).
//! - [`RotatingTlsConfig`], [`build_client_tls_connector`],
//!   [`parse_server_name`], [`serve_tls_handshake`] — per-connection
//!   TLS helpers (§7.1).
//! - [`SecurityConfig`], [`SecurityCtx`], [`SecurityError`],
//!   [`authenticate`], [`authorize`], [`build_ctx`],
//!   [`extract_mtls_subject`] — aggregate ctx of auth + ACL + TLS.
//! - [`TlsConfigError`], [`load_server_config`] — `rustls`
//!   ServerConfig builder with PEM cert/key loader (§7.1).
//!
//! ## Example
//!
//! ```rust,no_run
//! use zerodds_bridge_security::{Acl, AclOp, AuthSubject};
//!
//! let subj = AuthSubject::new("alice").with_group("publishers");
//! let acl = Acl::allow_all();
//! let _allowed = acl.check(&subj, AclOp::Write, "/topics/trade");
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

// Crypto primitive backend: `ring` (default) or `aws-lc-rs` (FIPS, via `aws-lc`).
// Swaps the JWT-RS256 verify primitives (`backend::`) and, in lockstep, the
// rustls TLS provider (`rustls::crypto::ring` ↔ `rustls::crypto::aws_lc_rs`).
#[cfg(feature = "aws-lc")]
pub(crate) use aws_lc_rs as backend;
#[cfg(all(feature = "ring-backend", not(feature = "aws-lc")))]
pub(crate) use ring as backend;
#[cfg(not(any(feature = "ring-backend", feature = "aws-lc")))]
compile_error!("enable exactly one crypto backend: `ring-backend` (default) or `aws-lc`");

/// The rustls [`CryptoProvider`](rustls::crypto::CryptoProvider) for the
/// selected backend — `ring` by default, `aws_lc_rs` under the `aws-lc` (FIPS)
/// feature. Pass it to `*_with_provider` builders; the implicit `builder()`
/// path is ambiguous when both providers are linked.
#[cfg(not(feature = "aws-lc"))]
pub fn tls_provider() -> rustls::crypto::CryptoProvider {
    rustls::crypto::ring::default_provider()
}
/// See [`tls_provider`] (ring variant).
#[cfg(feature = "aws-lc")]
pub fn tls_provider() -> rustls::crypto::CryptoProvider {
    rustls::crypto::aws_lc_rs::default_provider()
}

pub mod acl;
pub mod auth;
pub mod connection;
pub mod ctx;
pub mod tls;

pub use acl::{Acl, AclEntry, AclOp};
pub use auth::{AuthError, AuthMode, AuthSubject};
pub use connection::{
    RotatingTlsConfig, build_client_tls_connector, parse_server_name, serve_tls_handshake,
};
pub use ctx::{
    SecurityConfig, SecurityCtx, SecurityError, authenticate, authorize, build_ctx,
    extract_mtls_subject,
};
pub use tls::{TlsConfigError, load_server_config};

// Dep-2 (Spec `zerodds-zero-copy-1.0` §Dep-Audit): re-exports of the
// canonical rustls types, so that bridge crates can switch their own
// `use rustls::*` imports to `use zerodds_bridge_security::rustls::*`
// and drop the direct `rustls` dep from their Cargo.toml. Prevents
// version drift between rustls versions that individual bridges might
// later upgrade manually.
//
// Bridges that only need ServerConfig/ClientConfig/cert/key wire types
// can, instead of direct rustls deps:
//
// ```rust,ignore
// use zerodds_bridge_security::{rustls, rustls_pki_types, rustls_pemfile};
// let cfg: rustls::ServerConfig = ...;
// ```
//
// Re-exports are considered stable API of the bridge-security crate;
// they follow the same version pinning as rustls in
// [`workspace.dependencies`].

/// Re-export of the `rustls` crate for bridges.
pub use rustls;
/// Re-export of the `rustls_pemfile` crate for the PEM loader.
pub use rustls_pemfile;
/// Re-export of the `rustls_pki_types` crate for cert/key wire types.
pub use rustls_pki_types;
