// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-bridge-security`. Safety classification: **STANDARD**.
//!
//! Gemeinsamer Security-Layer für ZeroDDS Bridge-Daemons (ws / mqtt /
//! coap / amqp / grpc / corba).
//!
//! Spec: ZeroDDS Bridge-Spec 1.0 §7.1 (TLS), §7.2 (Auth-Modes), §7.3
//! (Topic-ACL).
//!
//! ## Schichten-Position
//!
//! Layer 5 (Bridges) — Substrat-Crate fuer alle sechs Bridge-Daemons.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`Acl`], [`AclEntry`], [`AclOp`] — Topic-ACL mit Wildcard- und
//!   Group-Matching (§7.3).
//! - [`AuthMode`], [`AuthSubject`], [`AuthError`] — Auth-Modes
//!   `none|bearer|jwt|mtls|sasl` (§7.2).
//! - [`RotatingTlsConfig`], [`build_client_tls_connector`],
//!   [`parse_server_name`], [`serve_tls_handshake`] — pro-Connection-
//!   TLS-Helpers (§7.1).
//! - [`SecurityConfig`], [`SecurityCtx`], [`SecurityError`],
//!   [`authenticate`], [`authorize`], [`build_ctx`],
//!   [`extract_mtls_subject`] — Aggregat-Ctx aus Auth + ACL + TLS.
//! - [`TlsConfigError`], [`load_server_config`] — `rustls`-
//!   ServerConfig-Builder mit PEM-Cert/Key-Loader (§7.1).
//!
//! ## Beispiel
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
