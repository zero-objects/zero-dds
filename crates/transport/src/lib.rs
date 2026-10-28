// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-transport`. Safety classification: **SAFE**.
//!
//! Transport trait + Locator re-export + abstract send/receive
//! interface. Pure-Rust no_std + alloc, `forbid(unsafe_code)`.
//!
//! ## Spec
//!
//! - **DDSI-RTPS 2.5** §8.3.2 — Locator (re-exported from
//!   `zerodds-rtps::wire_types::Locator`).
//! - ZeroDDS's own [`Transport`] trait — abstract layer between
//!   RTPS state machines and concrete wire protocols.
//!
//! ## Layer position
//!
//! Layer 2 — Wire (trait crate). Implementations: `zerodds-transport-udp`,
//! `zerodds-transport-tcp`, `zerodds-transport-shm`, `zerodds-transport-uds`,
//! `zerodds-transport-tsn`. Direct consumers: `zerodds-dcps`,
//! `zerodds-discovery`.
//!
//! ## Public API
//!
//! - [`Transport`] — trait for send/receive operations with Locator
//!   addressing.
//! - [`SendError`] / [`RecvError`] — typed errors.
//! - [`ReceivedDatagram`] — received datagram + source Locator.
//! - `Locator` — re-exported from `zerodds-rtps::wire_types`
//!   (spec anchor DDSI-RTPS §8.3.2).
//!
//! ## Architecture note
//!
//! `Locator` lives **deliberately** in `zerodds-rtps` (the DDSI-RTPS spec
//! defines the wire format there) and is re-exported via `pub use`. The
//! resulting `transport → rtps` crate dependency is deliberate in ZeroDDS
//! and is not a layer-inversion bug — RTPS wire-format types
//! belong in the rtps crate; the transport trait abstracts the
//! send/receive on top of it.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

use core::fmt;

pub use zerodds_rtps::wire_types::Locator;

// Vec is no longer used internally (ReceivedDatagram::data is now
// Arc<[u8]>). Tests in the alloc module reference it via alloc::vec.

/// Error when sending over a transport.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SendError {
    /// Datagram too large for the path MTU.
    PayloadTooLarge {
        /// Actual datagram length.
        size: usize,
        /// Configured limit.
        limit: usize,
    },
    /// Locator cannot be served by this transport
    /// (e.g. a UDPv6 locator to a UDPv4-only transport).
    UnsupportedLocator,
    /// I/O error in the underlying operation. The detail text is
    /// transport-specific.
    Io {
        /// Description.
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

/// Error when receiving.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecvError {
    /// Recv operation timed out.
    Timeout,
    /// Receive buffer was too small. The datagram was truncated or
    /// dropped — transport-specific.
    BufferTooSmall {
        /// Datagram size that did not fit.
        datagram_size: usize,
        /// Buffer capacity.
        buffer_size: usize,
    },
    /// I/O error.
    Io {
        /// Description.
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

/// Received datagram + source locator.
///
/// `data` is a refcounted `Arc<[u8]>` so that downstream consumers can
/// share the buffer without a heap copy (spec: zerodds-zero-copy-1.0 §6
/// wave 3). Construct via `Arc::from(vec.into_boxed_slice())` or
/// `Arc::from(&slice[..])`.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedDatagram {
    /// Source locator (e.g. the UDP source address).
    pub source: Locator,
    /// Datagram bytes (refcounted, zero-copy shareable).
    pub data: alloc::sync::Arc<[u8]>,
}

/// What [`Transport::max_message_size`] answers by default: 1472 octets,
/// 1500 of an Ethernet payload less the IPv4 header (20) and the UDP header
/// (8).
pub const DEFAULT_MAX_MESSAGE_SIZE: usize = 1472;

/// Abstract transport: sends/receives datagrams to/from locators.
///
/// Implementations are synchronous and blocking. Async wrappers can
/// be layered on top.
#[cfg(feature = "alloc")]
pub trait Transport {
    /// Sends a datagram to the given locator.
    ///
    /// # Errors
    /// [`SendError`].
    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError>;

    /// Sends a datagram with the TRANSPORT_PRIORITY of the writer it is
    /// from (DDS 1.4 §2.2.3.15, "a hint to the infrastructure"). `0` is no
    /// priority and the same as [`Self::send`]; a transport that has no
    /// way to carry one sends as [`Self::send`] does.
    ///
    /// # Errors
    /// [`SendError`].
    fn send_with_priority(
        &self,
        dest: &Locator,
        data: &[u8],
        priority: i32,
    ) -> Result<(), SendError> {
        let _ = priority;
        self.send(dest, data)
    }

    /// The largest RTPS message, in octets, this transport sends in one
    /// datagram. DDSI-RTPS 2.5 §8.4.14.1: "the transport with the smallest
    /// maximum message size determines the fragment size", over all the
    /// transports available to a writer. The default is
    /// [`DEFAULT_MAX_MESSAGE_SIZE`], what an Ethernet frame carries in a UDP
    /// datagram; a transport with another limit says so.
    fn max_message_size(&self) -> usize {
        DEFAULT_MAX_MESSAGE_SIZE
    }

    /// Receives the next datagram. Blocking; concrete
    /// implementations can configure a timeout via an internal setting.
    ///
    /// # Errors
    /// [`RecvError`].
    fn recv(&self) -> Result<ReceivedDatagram, RecvError>;

    /// Local locator (where this transport receives).
    fn local_locator(&self) -> Locator;

    /// All local locators this transport receives on. Defaults to the single
    /// [`Self::local_locator`]; a multiplexing transport (e.g. a layered
    /// SHM + UDP transport) overrides this to advertise every leg's locator
    /// so peers can reach it over any of them.
    fn local_locators(&self) -> alloc::vec::Vec<Locator> {
        alloc::vec![self.local_locator()]
    }

    /// The peer behind `dest` is gone: a transport that keeps state per
    /// destination (the UDP transport's connected socket per target) drops
    /// it. Nothing by default.
    fn forget_destination(&self, _dest: &Locator) {}

    /// The owner of this transport stops using it: what the transport
    /// runs ends, and what it holds open closes. A `recv` that waits
    /// returns, and a later `send` or `recv` may fail. Without it a
    /// receive loop that waits until the transport's own stop (a TCP
    /// connection reader, a per-peer SHM thread) kept a shutdown's join
    /// waiting. Nothing by default: a socket's read timeout ends its wait.
    fn close(&self) {}

    /// Receives up to `max` datagrams, appending them to `out`, and returns
    /// how many were appended. Blocking for the first one, exactly like
    /// [`Self::recv`]; whatever else has already arrived comes with it.
    ///
    /// The default appends the single [`Self::recv`] result, so a transport
    /// that has nothing to batch needs no implementation and behaves as
    /// before. A transport whose kernel interface can hand over several
    /// datagrams per syscall overrides this — that is the whole point of the
    /// method: a caller holding a `dyn Transport` cannot reach an inherent
    /// batch method, so without it the batch path is unreachable from the
    /// DCPS receive loop no matter which features are enabled.
    ///
    /// `out` is **not** cleared, so a caller can keep its capacity across
    /// iterations. A return of `0` means the wait ended with nothing (only
    /// possible for an implementation that maps a timeout to zero rather than
    /// to [`RecvError::Timeout`]).
    ///
    /// # Errors
    /// [`RecvError`], on the same conditions as [`Self::recv`].
    fn recv_batch(
        &self,
        out: &mut alloc::vec::Vec<ReceivedDatagram>,
        max: usize,
    ) -> Result<usize, RecvError> {
        if max == 0 {
            return Ok(0);
        }
        out.push(self.recv()?);
        Ok(1)
    }
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
