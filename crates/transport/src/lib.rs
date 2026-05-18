// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-transport`. Safety classification: **SAFE**.
//!
//! Transport-Trait + Locator-Re-Export + abstrakte send/receive-
//! Schnittstelle. Pure-Rust no_std + alloc, `forbid(unsafe_code)`.
//!
//! ## Spec
//!
//! - **DDSI-RTPS 2.5** §8.3.2 — Locator (re-exportiert aus
//!   `zerodds-rtps::wire_types::Locator`).
//! - ZeroDDS-eigenes [`Transport`]-Trait — abstrakte Schicht zwischen
//!   RTPS-State-Machines und konkreten Wire-Protokollen.
//!
//! ## Schichten-Position
//!
//! Layer 2 — Wire (Trait-Crate). Implementations: `zerodds-transport-udp`,
//! `zerodds-transport-tcp`, `zerodds-transport-shm`, `zerodds-transport-uds`,
//! `zerodds-transport-tsn`. Direkte Konsumenten: `zerodds-dcps`,
//! `zerodds-discovery`.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`Transport`] — Trait für send/receive-Operationen mit Locator-
//!   Adressierung.
//! - [`SendError`] / [`RecvError`] — typisierte Fehler.
//! - [`ReceivedDatagram`] — Empfangenes Datagramm + Source-Locator.
//! - `Locator` — re-exportiert aus `zerodds-rtps::wire_types`
//!   (Spec-Anker DDSI-RTPS §8.3.2).
//!
//! ## Architektur-Hinweis
//!
//! `Locator` lebt **bewusst** in `zerodds-rtps` (DDSI-RTPS-Spec definiert
//! das Wire-Format dort) und wird via `pub use` re-exportiert. Die
//! resultierende `transport → rtps` Crate-Dep ist ZeroDDS-deliberat
//! und stellt keinen Layer-Inversion-Bug dar — RTPS-Wire-Format-Types
//! gehören in die rtps-Crate, das Transport-Trait abstrahiert das
//! send/receive darüber.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

use core::fmt;

pub use zerodds_rtps::wire_types::Locator;

// Vec wird intern nicht mehr genutzt (ReceivedDatagram::data ist jetzt
// Arc<[u8]>). Tests im alloc-Modul referenzieren ihn ueber alloc::vec.

/// Fehler beim Senden ueber einen Transport.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SendError {
    /// Datagram zu gross fuer den Pfad-MTU.
    PayloadTooLarge {
        /// Tatsaechliche Datagram-Laenge.
        size: usize,
        /// Konfiguriertes Limit.
        limit: usize,
    },
    /// Locator kann von diesem Transport nicht bedient werden
    /// (z.B. UDPv6-Locator an einen UDPv4-only-Transport).
    UnsupportedLocator,
    /// I/O-Fehler bei der zugrundeliegenden Operation. Detail-Text
    /// transport-spezifisch.
    Io {
        /// Beschreibung.
        message: &'static str,
    },
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PayloadTooLarge { size, limit } => {
                write!(f, "transport payload too large: {size} > {limit}")
            }
            Self::UnsupportedLocator => f.write_str("transport: unsupported locator"),
            Self::Io { message } => write!(f, "transport I/O error: {message}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SendError {}

/// Fehler beim Empfangen.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecvError {
    /// Recv-Operation lief in Timeout.
    Timeout,
    /// Empfangs-Buffer war zu klein. Datagramm wurde getruncated oder
    /// verworfen — transport-spezifisch.
    BufferTooSmall {
        /// Datagram-Groesse, die nicht passte.
        datagram_size: usize,
        /// Buffer-Kapazitaet.
        buffer_size: usize,
    },
    /// I/O-Fehler.
    Io {
        /// Beschreibung.
        message: &'static str,
    },
}

impl fmt::Display for RecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout => f.write_str("transport recv timeout"),
            Self::BufferTooSmall {
                datagram_size,
                buffer_size,
            } => {
                write!(
                    f,
                    "recv buffer too small: datagram {datagram_size} > buffer {buffer_size}"
                )
            }
            Self::Io { message } => write!(f, "transport I/O error: {message}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RecvError {}

/// Empfangenes Datagram + Quell-Locator.
///
/// `data` ist ein refcounted `Arc<[u8]>` damit downstream-Consumer den
/// Buffer ohne Heap-Copy teilen koennen (Spec: zerodds-zero-copy-1.0 §6
/// Welle 3). Konstruktion via `Arc::from(vec.into_boxed_slice())` oder
/// `Arc::from(&slice[..])`.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedDatagram {
    /// Quell-Locator (z.B. UDP-Quelladresse).
    pub source: Locator,
    /// Datagram-Bytes (refcounted, zero-copy-shareable).
    pub data: alloc::sync::Arc<[u8]>,
}

/// Abstrakter Transport: sendet/empfaengt Datagramme zu/von Locatoren.
///
/// Implementations sind synchron und blocking. Async-Wrapper koennen
/// in einem hoeheren Layer aufgesetzt werden.
#[cfg(feature = "alloc")]
pub trait Transport {
    /// Sendet ein Datagram an den gegebenen Locator.
    ///
    /// # Errors
    /// [`SendError`].
    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError>;

    /// Empfaengt das naechste Datagram. Blocking; konkrete
    /// Implementations koennen Timeout via internem Setting konfigurieren.
    ///
    /// # Errors
    /// [`RecvError`].
    fn recv(&self) -> Result<ReceivedDatagram, RecvError>;

    /// Lokaler Locator (an dem dieser Transport empfangt).
    fn local_locator(&self) -> Locator;
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "alloc")]
    extern crate alloc;
    #[cfg(feature = "alloc")]
    use alloc::format;

    use super::*;

    #[cfg(feature = "alloc")]
    #[test]
    fn send_error_display_payload_too_large() {
        let e = SendError::PayloadTooLarge {
            size: 70_000,
            limit: 65_535,
        };
        let s = format!("{e}");
        assert!(s.contains("70000"));
        assert!(s.contains("65535"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn send_error_display_unsupported_locator() {
        let s = format!("{}", SendError::UnsupportedLocator);
        assert!(s.contains("unsupported"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn send_error_display_io() {
        let s = format!(
            "{}",
            SendError::Io {
                message: "ENETDOWN"
            }
        );
        assert!(s.contains("ENETDOWN"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn recv_error_display_timeout() {
        let s = format!("{}", RecvError::Timeout);
        assert!(s.contains("timeout"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn recv_error_display_buffer_too_small() {
        let s = format!(
            "{}",
            RecvError::BufferTooSmall {
                datagram_size: 1500,
                buffer_size: 1024
            }
        );
        assert!(s.contains("1500"));
        assert!(s.contains("1024"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn received_datagram_is_constructable() {
        let bytes: alloc::sync::Arc<[u8]> = alloc::sync::Arc::from([1u8, 2, 3].as_slice());
        let r = ReceivedDatagram {
            source: Locator::INVALID,
            data: bytes,
        };
        assert_eq!(&r.data[..], &[1u8, 2, 3]);
    }
}
