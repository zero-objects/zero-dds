// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE TLS-Wrapper fuer TCP (Spec §11.3 + §11.4).
//!
//! C6.2.D liefert hier nur ein **Skelett**, keine Live-TLS-Verbindung.
//! Begruendung: das Workspace hat zwar `rustls-pki-types` und
//! `rustls-webpki` (in `crates/security-pki/`, `crates/security-permissions/`),
//! aber keine `rustls`-Server-/Client-Engine als Dependency. Eine Voll-
//! integration zoege transitive Krypto-Provider mit (ring, aws-lc-rs)
//! und ist deployment-spezifisch (Embedded vs. Linux). Wir definieren
//! deshalb eine **Trait-Boundary**, die spaeter mit Rustls oder einer
//! Embedded-TLS-Lib (z.B. `embedded-tls`) gefuellt werden kann.
//!
//! Das echte Wire-Format (TLS-Records + Length-Prefix-Framing) ist
//! identisch zu §11.3 — TLS terminiert auf der Stream-Seite, der XRCE-
//! Codec sieht nur Plain-Bytes.
//!
//! API-Form folgt `transport_tcp.rs`, damit ein spaeterer Plug-in-
//! Schritt minimalen Aufruf-Site-Drift braucht.

use crate::error::XrceError;
use crate::submessages::Message;

/// Trait fuer TLS-Stream-Implementierungen. Voll-implementiert wird in
/// einer separaten Plug-In-Crate (z.B. `xrce-tls-rustls`).
pub trait XrceTlsStream {
    /// Sendet eine `Message` ueber den TLS-Stream.
    ///
    /// # Errors
    /// `XrceError`, wenn TLS-Send oder Encode fehlschlaegt.
    fn send_message(&mut self, msg: &Message) -> Result<(), XrceError>;

    /// Empfaengt eine `Message`.
    ///
    /// # Errors
    /// `XrceError`, wenn TLS-Recv oder Decode fehlschlaegt.
    fn recv_message(&mut self) -> Result<Message, XrceError>;

    /// Schliesst den Stream (Close-Notify).
    ///
    /// # Errors
    /// `XrceError`, wenn der Close-Notify nicht ausgehandelt werden kann.
    fn close(&mut self) -> Result<(), XrceError>;
}

/// Skelett fuer einen TLS-Client. Echte Implementierung wird ueber das
/// `tls`-Cargo-Feature aktiviert und benutzt eine externe TLS-Lib.
#[derive(Debug, Default)]
pub struct XrceTlsClient {
    /// Server-Name fuer SNI / Cert-Validation.
    pub server_name: alloc::string::String,
}

#[cfg(feature = "alloc")]
extern crate alloc;

impl XrceTlsClient {
    /// Konstruktor. Nimmt nur SNI-Server-Name. Die echte Verbindung
    /// passiert bei `connect`.
    #[must_use]
    pub fn new(server_name: alloc::string::String) -> Self {
        Self { server_name }
    }

    /// Verbindet sich (Stub). Liefert immer `Err(NotImplemented)`-aequivalent
    /// als `ValueOutOfRange`, bis eine TLS-Engine geplugged wird.
    ///
    /// # Errors
    /// `XrceError::ValueOutOfRange` mit Beschreibung. C6.2.D liefert nur
    /// das Skelett.
    pub fn connect(&self) -> Result<(), XrceError> {
        Err(XrceError::ValueOutOfRange {
            message: "tls connect: no engine plugged (C6.2.D skeleton)",
        })
    }
}

/// Skelett fuer einen TLS-Server.
#[derive(Debug, Default)]
pub struct XrceTlsServer;

impl XrceTlsServer {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Bindet einen TLS-Listener (Stub).
    ///
    /// # Errors
    /// `XrceError::ValueOutOfRange`, bis eine TLS-Engine geplugged wird.
    pub fn bind(&self) -> Result<(), XrceError> {
        Err(XrceError::ValueOutOfRange {
            message: "tls bind: no engine plugged (C6.2.D skeleton)",
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn tls_client_new_stores_server_name() {
        let c = XrceTlsClient::new("agent.example.invalid".into());
        assert_eq!(c.server_name, "agent.example.invalid");
    }

    #[test]
    fn tls_client_connect_returns_skeleton_error() {
        let c = XrceTlsClient::new("x".into());
        let res = c.connect();
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    #[test]
    fn tls_server_bind_returns_skeleton_error() {
        let s = XrceTlsServer::new();
        let res = s.bind();
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    #[test]
    fn tls_server_default_constructible() {
        let _ = XrceTlsServer;
    }
}
