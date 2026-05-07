// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE DTLS-Wrapper fuer UDP (Spec §11.5).
//!
//! C6.2.D liefert hier nur ein Trait-Skelett + eine Identity-Pass-Through-
//! Implementierung (`DummyDtls`). Begruendung:
//!
//! - DTLS-Stack-Wahl ist deployment-spezifisch (Embedded mit
//!   `embedded-tls`, Linux mit `webrtc-dtls` oder `tokio-dtls`).
//! - Eine Vollintegration wuerde transitive Krypto-Provider ziehen
//!   (ring, aws-lc-rs) und die `crates/xrce`-Crate von `no_std`-Pfad
//!   abkoppeln.
//! - Trait-Boundary jetzt + Plug-In spaeter ist die schlankere
//!   Architektur, analog zur Security-Stack-Trennung in
//!   `crates/security-runtime/`.
//!
//! Das DTLS-Layer sitzt unter dem XRCE-Codec auf dem UDP-Layer:
//!
//! ```text
//!   XRCE Message (Plaintext)
//!         |
//!         v
//!     DTLS-Layer  --[handshake]-> DTLS-Records
//!         |
//!         v
//!     UDP Datagram
//! ```
//!
//! Im Empfangs-Pfad invers.

extern crate alloc;
use alloc::vec::Vec;

/// Fehler-Klasse fuer DTLS-Operationen.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DtlsError {
    /// Handshake-Phase ist fehlgeschlagen.
    HandshakeFailed {
        /// Beschreibung.
        reason: &'static str,
    },
    /// Send-Operation fehlgeschlagen.
    SendFailed {
        /// Beschreibung.
        reason: &'static str,
    },
    /// Recv-Operation fehlgeschlagen.
    RecvFailed {
        /// Beschreibung.
        reason: &'static str,
    },
    /// Stream wurde geschlossen.
    Closed,
}

impl core::fmt::Display for DtlsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::HandshakeFailed { reason } => write!(f, "dtls handshake failed: {reason}"),
            Self::SendFailed { reason } => write!(f, "dtls send failed: {reason}"),
            Self::RecvFailed { reason } => write!(f, "dtls recv failed: {reason}"),
            Self::Closed => write!(f, "dtls stream closed"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DtlsError {}

/// Trait fuer DTLS-Layer-Implementierungen. Plugin-Stelle fuer eine
/// echte DTLS-Engine.
pub trait DtlsLayer {
    /// Fuehrt den DTLS-Handshake aus. Muss vor dem ersten `send`/`recv`
    /// erfolgreich gewesen sein.
    ///
    /// # Errors
    /// `HandshakeFailed`.
    fn handshake(&mut self) -> Result<(), DtlsError>;

    /// Verschluesselt+sendet `plaintext` ueber das DTLS-Layer.
    ///
    /// # Errors
    /// `SendFailed`, `Closed`.
    fn send(&mut self, plaintext: &[u8]) -> Result<(), DtlsError>;

    /// Empfaengt+entschluesselt eine DTLS-Record. Liefert den Plaintext.
    ///
    /// # Errors
    /// `RecvFailed`, `Closed`.
    fn recv(&mut self) -> Result<Vec<u8>, DtlsError>;

    /// Schliesst den Stream (DTLS close-notify).
    ///
    /// # Errors
    /// `SendFailed`.
    fn close(&mut self) -> Result<(), DtlsError>;

    /// `true`, wenn der Handshake abgeschlossen ist.
    fn is_handshake_complete(&self) -> bool;
}

/// Identity-Pass-Through-DTLS — fuer Tests und als Pre-Production-Stub.
///
/// Verschluesselt nichts, leitet Plaintext durch. Damit kann der
/// DTLS-Handshake/Send/Recv-Codepfad in Integration-Tests ohne echte
/// Krypto-Lib geuebt werden.
#[derive(Debug, Default)]
pub struct DummyDtls {
    handshake_done: bool,
    inbox: alloc::collections::VecDeque<Vec<u8>>,
    closed: bool,
}

impl DummyDtls {
    /// Frischer Pass-Through-Stub.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Schiebt einen Plaintext-Block in die Inbox, sodass der naechste
    /// `recv()` ihn liefert. Test-Hilfe.
    pub fn inject(&mut self, plaintext: Vec<u8>) {
        self.inbox.push_back(plaintext);
    }

    /// Anzahl Pakete in der Inbox.
    #[must_use]
    pub fn inbox_len(&self) -> usize {
        self.inbox.len()
    }
}

impl DtlsLayer for DummyDtls {
    fn handshake(&mut self) -> Result<(), DtlsError> {
        self.handshake_done = true;
        Ok(())
    }

    fn send(&mut self, plaintext: &[u8]) -> Result<(), DtlsError> {
        if self.closed {
            return Err(DtlsError::Closed);
        }
        if !self.handshake_done {
            return Err(DtlsError::SendFailed {
                reason: "handshake not complete",
            });
        }
        // Loopback: der gesendete Plaintext landet in der eigenen Inbox.
        self.inbox.push_back(plaintext.to_vec());
        Ok(())
    }

    fn recv(&mut self) -> Result<Vec<u8>, DtlsError> {
        if self.closed && self.inbox.is_empty() {
            return Err(DtlsError::Closed);
        }
        if !self.handshake_done {
            return Err(DtlsError::RecvFailed {
                reason: "handshake not complete",
            });
        }
        self.inbox.pop_front().ok_or(DtlsError::RecvFailed {
            reason: "inbox empty",
        })
    }

    fn close(&mut self) -> Result<(), DtlsError> {
        self.closed = true;
        Ok(())
    }

    fn is_handshake_complete(&self) -> bool {
        self.handshake_done
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn dummy_dtls_handshake_then_send_then_recv() {
        let mut d = DummyDtls::new();
        assert!(!d.is_handshake_complete());
        d.handshake().unwrap();
        assert!(d.is_handshake_complete());
        d.send(&[1, 2, 3, 4]).unwrap();
        let pt = d.recv().unwrap();
        assert_eq!(pt, alloc::vec![1, 2, 3, 4]);
    }

    #[test]
    fn dummy_dtls_send_before_handshake_fails() {
        let mut d = DummyDtls::new();
        let res = d.send(&[1, 2, 3]);
        assert!(matches!(
            res,
            Err(DtlsError::SendFailed {
                reason: "handshake not complete"
            })
        ));
    }

    #[test]
    fn dummy_dtls_recv_before_handshake_fails() {
        let mut d = DummyDtls::new();
        let res = d.recv();
        assert!(matches!(
            res,
            Err(DtlsError::RecvFailed {
                reason: "handshake not complete"
            })
        ));
    }

    #[test]
    fn dummy_dtls_close_returns_closed_on_subsequent_send() {
        let mut d = DummyDtls::new();
        d.handshake().unwrap();
        d.close().unwrap();
        let res = d.send(&[1]);
        assert!(matches!(res, Err(DtlsError::Closed)));
    }

    #[test]
    fn dummy_dtls_close_drains_inbox_then_returns_closed() {
        let mut d = DummyDtls::new();
        d.handshake().unwrap();
        d.send(&[1]).unwrap();
        d.close().unwrap();
        // Inbox-Drain weiter erlaubt.
        let pt = d.recv().unwrap();
        assert_eq!(pt, alloc::vec![1]);
        // danach Closed.
        let res = d.recv();
        assert!(matches!(res, Err(DtlsError::Closed)));
    }

    #[test]
    fn dummy_dtls_inject_makes_recv_yield_payload() {
        let mut d = DummyDtls::new();
        d.handshake().unwrap();
        d.inject(alloc::vec![9, 8, 7]);
        assert_eq!(d.inbox_len(), 1);
        assert_eq!(d.recv().unwrap(), alloc::vec![9, 8, 7]);
        assert_eq!(d.inbox_len(), 0);
    }

    #[test]
    fn dtls_error_display_formats_handshake() {
        let e = DtlsError::HandshakeFailed { reason: "bad cert" };
        let s = alloc::format!("{e}");
        assert!(s.contains("bad cert"));
    }

    #[test]
    fn dtls_error_display_formats_closed() {
        let s = alloc::format!("{}", DtlsError::Closed);
        assert!(s.contains("closed"));
    }

    #[test]
    fn dummy_dtls_default_is_constructible() {
        let _ = DummyDtls::default();
    }
}
