// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-transport-udp`. Safety classification: **SAFE** (std-only).
//!
//! UDP/IP PSM implementation of the `zerodds-transport::Transport` trait.
//!
//! ## Spec
//!
//! - **DDSI-RTPS 2.5** §9.6.1 — UDP/IP PSM wire mapping.
//! - **DDSI-RTPS 2.5** §8.3.2 — locator (re-export from `zerodds-transport`).
//!
//! ## Implemented (RC1)
//!
//! - UDPv4 unicast bind + send + recv via `std::net::UdpSocket`
//! - UDPv4 multicast: `bind_multicast_v4` with group join +
//!   `SO_REUSEADDR`/`SO_REUSEPORT` + `set_multicast_ttl_v4` (DDSI-RTPS
//!   §9.6.1.4 SPDP/SEDP discovery path)
//! - Read timeout configurable via `with_timeout`
//! - Bind retry loop for the CI EADDRINUSE race (3× backoff)
//! - Application-layer fragmentation in `zerodds-rtps` (DATA_FRAG/NACK_FRAG,
//!   WP 1.2) handles MTU — the transport itself sends datagrams
//!   atomically with the `MAX_DATAGRAM_SIZE` cap.
//!
//! ## Deliberately not in the crate
//!
//! - **UDPv6**: the locator wire format supports v6, but the bind API is
//!   v4-specific (`bind_v4`). v6 would require a parallel `bind_v6` +
//!   `Ipv6Addr` code path. An extension point — no layer break.
//! - **Async/non-blocking**: the sync architecture is a chosen style — DCPS
//!   uses its own tick scheduler, no Tokio stack.
//! - **Path-MTU discovery**: fragmentation runs at the RTPS layer (WP 1.2).

#![cfg_attr(not(feature = "std"), no_std)]
// `deny(unsafe_code)` as the crate default; local `#[allow(unsafe_code)]`
// islands only for the setsockopt FFI (SO_DONTROUTE on Unix) and the
// recvmmsg module (feature `recvmmsg-batch`). Every unsafe site has a
// SAFETY comment.
#![deny(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

/// Opt-2 (spec `zerodds-zero-copy-1.0` §9): Linux `recvmmsg` batch recv.
/// Only compiled with the `recvmmsg-batch` feature enabled + on Linux.
#[cfg(all(feature = "std", feature = "recvmmsg-batch", target_os = "linux"))]
pub mod recv_batch;
#[cfg(feature = "std")]
mod udp_transport;

/// #27: eligible local IPv4 interface enumeration for multi-locator discovery
/// announcement.
#[cfg(feature = "std")]
pub mod interfaces;

#[cfg(feature = "std")]
pub use interfaces::{EligibleInterface, eligible_ipv4_interfaces};
#[cfg(feature = "std")]
pub use udp_transport::{MAX_DATAGRAM_SIZE, UdpTransport, UdpTransportError};
